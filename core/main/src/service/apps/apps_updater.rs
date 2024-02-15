use jsonrpsee::core::async_trait;
use ripple_sdk::{
    api::{
        app_catalog::{self, AppsUpdate},
        device::device_apps::{AppMetadata, AppsRequest},
    },
    extn::{
        client::{
            extn_client::ExtnClient,
            extn_processor::{DefaultExtnStreamer, ExtnEventProcessor, ExtnStreamProcessor},
        },
        extn_client_message::{ExtnMessage, ExtnPayload, ExtnResponse},
    },
    tokio::sync::mpsc::{Receiver, Sender},
};

#[cfg(not(test))]
use ripple_sdk::log::{debug, error};

#[cfg(test)]
use std::{println as debug, println as error};

use ripple_sdk::extn::client::extn_processor::ExtnStreamer;

#[derive(Clone)]
pub struct AppsUpdaterState {
    client: ExtnClient,
    ignore_list: Vec<String>,
    uninstalls_enabled: bool,
}

pub struct AppsUpdater {
    state: AppsUpdaterState,
    streamer: DefaultExtnStreamer,
}

impl AppsUpdater {
    pub fn new(
        client: ExtnClient,
        ignore_list: Vec<String>,
        uninstalls_enabled: bool,
    ) -> AppsUpdater {
        AppsUpdater {
            state: AppsUpdaterState {
                client,
                ignore_list,
                uninstalls_enabled,
            },
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for AppsUpdater {
    type VALUE = AppsUpdate;
    type STATE = AppsUpdaterState;

    fn get_state(&self) -> Self::STATE {
        self.state.clone()
    }

    fn sender(&self) -> Sender<ExtnMessage> {
        self.streamer.sender()
    }

    fn receiver(&mut self) -> Receiver<ExtnMessage> {
        self.streamer.receiver()
    }
}

#[async_trait]
impl ExtnEventProcessor for AppsUpdater {
    async fn process_event(
        state: Self::STATE,
        _msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> Option<bool> {
        update(
            state.client.clone(),
            extracted_message,
            state.ignore_list,
            state.uninstalls_enabled,
        )
        .await;
        None
    }
}

pub async fn update(
    mut client: ExtnClient,
    apps_update: AppsUpdate,
    ignore_list: Vec<String>,
    uninstalls_enabled: bool,
) {
    debug!("update: apps_update={:?}", apps_update);

    let resp = client.request(AppsRequest::GetApps(None)).await;
    if let Err(e) = resp {
        error!("update: Could not retrieve app list: e={:?}", e);
        return;
    }

    let installed_apps = match resp.unwrap().payload {
        ExtnPayload::Response(response) => match response {
            ExtnResponse::InstalledApps(apps) => apps,
            _ => {
                error!("update: Unexpected response");
                return;
            }
        },
        _ => {
            error!("update: Unexpected payload");
            return;
        }
    };

    if uninstalls_enabled {
        // Uninstall removed apps

        for installed_app in installed_apps.clone() {
            if ignore_list.contains(&installed_app.id) {
                // Skip ignored apps
                continue;
            }

            if !apps_update
                .apps
                .clone()
                .into_iter()
                .any(|app| app.id.eq(&installed_app.id))
            {
                debug!("update: Existing application is no longer in the updated catalog, uninstalling: id={}, version={}", installed_app.id, installed_app.version);

                let resp = client
                    .request(AppsRequest::UninstallApp(installed_app))
                    .await;

                if let Err(e) = resp {
                    error!("update: Could not uninstall app: e={:?}", e);
                }
            }
        }
    }

    // Install new/updated apps

    debug!(
        "update: apps_update={:?}, installed_apps={:?}",
        apps_update,
        installed_apps.clone()
    );

    let mut app_list: Vec<app_catalog::AppMetadata> = apps_update
        .apps
        .into_iter()
        .filter(|a| {
            !installed_apps
                .iter()
                .any(|ia| ia.id.eq(&a.id) && ia.version.eq(&a.version))
        })
        .collect();
    app_list.sort_by_key(|app| app.install_priority);
    for app in app_list {
        debug!("update: Application is not currently installed, installing: title={}, id={}, version={}", app.title, app.id, app.version);
        let metadata = AppMetadata::new(app.id, app.title, app.version, app.uri, app.data);
        let resp = client.request(AppsRequest::InstallApp(metadata)).await;

        if let Err(e) = resp {
            error!("update: Could not install app: e={:?}", e);
        }
    }
}

#[cfg(test)]
pub mod tests {

    use ripple_sdk::{
        api::{
            app_catalog::{AppMetadata, AppsUpdate},
            device::device_apps::{AppMetadata as DeviceAppMetadata, AppsRequest, InstalledApp},
        },
        async_channel::Receiver,
        extn::{
            client::extn_client::ExtnClient,
            extn_client_message::{ExtnMessage, ExtnPayload, ExtnRequest, ExtnResponse},
            ffi::ffi_message::CExtnMessage,
            mock_extension_client::MockExtnClient,
        },
        framework::ripple_contract::RippleContract,
        tokio::{self, spawn, task::JoinHandle},
    };
    use serde_json::Value;

    use super::update;

    struct AppBuilder {
        meta: AppMetadata,
    }

    impl AppBuilder {
        pub fn new(id: &str) -> AppBuilder {
            AppBuilder {
                meta: AppMetadata {
                    id: String::from(id),
                    title: String::from(id),
                    version: String::from("1.0"),
                    uri: String::from("http://localhost/app.pkg"),
                    data: None,
                    install_priority: 9999,
                },
            }
        }

        pub fn version(mut self, v: &str) -> AppBuilder {
            self.meta.version = String::from(v);
            self
        }

        pub fn priority(mut self, p: i32) -> AppBuilder {
            self.meta.install_priority = p;
            self
        }

        pub fn meta(self) -> AppMetadata {
            self.meta
        }

        pub fn installed(self) -> InstalledApp {
            InstalledApp {
                id: self.meta.id,
                version: self.meta.version,
            }
        }
    }

    struct MockPackageManager {
        pre_installed_apps: Vec<InstalledApp>,
        new_installed_apps: Vec<DeviceAppMetadata>,
        new_uninstalled_apps: Vec<InstalledApp>,
    }

    impl MockPackageManager {
        pub fn start(
            pre_installed_apps: Vec<InstalledApp>,
            mut client: ExtnClient,
            pm_r: Receiver<CExtnMessage>,
        ) -> JoinHandle<MockPackageManager> {
            let mut pm = MockPackageManager {
                pre_installed_apps,
                new_installed_apps: vec![],
                new_uninstalled_apps: vec![],
            };
            spawn(async move {
                while let Ok(m) = pm_r.recv().await {
                    let msg: ExtnMessage = m.try_into().unwrap();
                    if let ExtnPayload::Request(ExtnRequest::Extn(_)) = &msg.payload {
                        // using this payload to signal shutdown of mockPM
                        return pm;
                    }
                    let apps_req = msg.payload.extract::<AppsRequest>().unwrap();
                    match apps_req {
                        AppsRequest::GetApps(_) => {
                            let response =
                                ExtnResponse::InstalledApps(pm.pre_installed_apps.clone());
                            client
                                .send_message(msg.get_response(response).unwrap())
                                .await
                                .unwrap();
                        }
                        AppsRequest::InstallApp(app) => {
                            pm.new_installed_apps.push(app);
                            client.send_message(msg.ack()).await.unwrap();
                        }
                        AppsRequest::UninstallApp(app) => {
                            pm.new_uninstalled_apps.push(app);
                            client.send_message(msg.ack()).await.unwrap();
                        }
                        _ => panic!(),
                    }
                }
                pm
            })
        }

        /// Shuts down the mock extension and returns
        /// a MockPackageManager which can be used
        /// to validate new app installs and uninstalls
        pub async fn shutdown(
            client: &mut ExtnClient,
            pm_ftr: JoinHandle<MockPackageManager>,
        ) -> MockPackageManager {
            client
                .send_message(MockExtnClient::req(
                    RippleContract::Internal,
                    ExtnRequest::Extn(Value::Null),
                ))
                .await
                .ok();
            pm_ftr.await.unwrap()
        }
    }

    /// No apps installed, AppUpdate has one app
    #[tokio::test]
    pub async fn test_update_install_one_app() {
        let (mut client, pm_r) = MockExtnClient::main_and_extn(vec![String::from("apps")]);
        let pm_ftr = MockPackageManager::start(vec![], client.clone(), pm_r);
        let apps_update = AppsUpdate {
            apps: vec![AppBuilder::new("firecert").meta()],
        };

        update(client.clone(), apps_update, vec![], true).await;
        let pm = MockPackageManager::shutdown(&mut client, pm_ftr).await;
        let apps: Vec<String> = pm.new_installed_apps.iter().map(|x| x.id.clone()).collect();
        assert_eq!(apps.len(), 1);
        assert!(apps.contains(&String::from("firecert")));
    }

    /// AppUpdate has app that is already installed
    #[tokio::test]
    pub async fn test_update_no_changes() {
        let (mut client, pm_r) = MockExtnClient::main_and_extn(vec![String::from("apps")]);
        let pm_ftr = MockPackageManager::start(
            vec![AppBuilder::new("firecert").installed()],
            client.clone(),
            pm_r,
        );
        let apps_update = AppsUpdate {
            apps: vec![AppBuilder::new("firecert").meta()],
        };

        update(client.clone(), apps_update, vec![], true).await;

        let pm = MockPackageManager::shutdown(&mut client, pm_ftr).await;
        assert_eq!(pm.new_installed_apps.len(), 0);
        assert_eq!(pm.new_uninstalled_apps.len(), 0);
    }

    /// Install many apps of different priority
    #[tokio::test]
    pub async fn test_update_install_in_priority_order() {
        let (mut client, pm_r) = MockExtnClient::main_and_extn(vec![String::from("apps")]);
        let pm_ftr = MockPackageManager::start(vec![], client.clone(), pm_r);
        let apps_update = AppsUpdate {
            apps: vec![
                AppBuilder::new("no_priority_app").meta(),
                AppBuilder::new("high_priority_app").priority(1).meta(),
                AppBuilder::new("low_priority_app").priority(200).meta(),
                AppBuilder::new("medium_priority_app").priority(50).meta(),
            ],
        };

        update(client.clone(), apps_update, vec![], true).await;
        let pm = MockPackageManager::shutdown(&mut client, pm_ftr).await;
        let apps: Vec<&str> = pm
            .new_installed_apps
            .iter()
            .map(|x| x.id.as_str())
            .collect();
        let exp_order = vec![
            "high_priority_app",
            "medium_priority_app",
            "low_priority_app",
            "no_priority_app",
        ];
        assert_eq!(apps, exp_order);
    }

    /// app already installed, but update has a new version
    #[tokio::test]
    pub async fn test_update_update_one_app() {
        let (mut client, pm_r) = MockExtnClient::main_and_extn(vec![String::from("apps")]);
        let pm_ftr = MockPackageManager::start(
            vec![AppBuilder::new("firecert").version("1.0").installed()],
            client.clone(),
            pm_r,
        );
        let apps_update = AppsUpdate {
            apps: vec![AppBuilder::new("firecert").version("2.0").meta()],
        };

        update(client.clone(), apps_update, vec![], true).await;
        let pm = MockPackageManager::shutdown(&mut client, pm_ftr).await;
        assert_eq!(pm.new_installed_apps.len(), 1);
        let installed = pm
            .new_installed_apps
            .iter()
            .find(|a| a.id == "firecert")
            .unwrap();
        assert_eq!(installed.version, "2.0");
        assert_eq!(pm.new_uninstalled_apps.len(), 0);
    }

    /// app already installed, but update has app missing and uninstalls are enabled
    #[tokio::test]
    pub async fn test_update_unistall_one_with_enabled() {
        let (mut client, pm_r) = MockExtnClient::main_and_extn(vec![String::from("apps")]);
        let pm_ftr = MockPackageManager::start(
            vec![AppBuilder::new("firecert").installed()],
            client.clone(),
            pm_r,
        );
        let apps_update = AppsUpdate { apps: vec![] };

        update(client.clone(), apps_update, vec![], true).await;
        let pm = MockPackageManager::shutdown(&mut client, pm_ftr).await;
        assert_eq!(pm.new_installed_apps.len(), 0);
        assert_eq!(pm.new_uninstalled_apps.len(), 1);
        assert!(pm.new_uninstalled_apps.iter().any(|a| a.id == "firecert"));
    }

    /// app already installed, but update has app missing and uninstalls are disabled
    #[tokio::test]
    pub async fn test_update_unistall_one_with_disabled() {
        let (mut client, pm_r) = MockExtnClient::main_and_extn(vec![String::from("apps")]);
        let pm_ftr = MockPackageManager::start(
            vec![AppBuilder::new("firecert").installed()],
            client.clone(),
            pm_r,
        );
        let apps_update = AppsUpdate { apps: vec![] };

        update(client.clone(), apps_update, vec![], false).await;
        let pm = MockPackageManager::shutdown(&mut client, pm_ftr).await;
        assert_eq!(pm.new_installed_apps.len(), 0);
        assert_eq!(pm.new_uninstalled_apps.len(), 0);
    }

    /// Install one new app, Uninstall one old app
    #[tokio::test]
    pub async fn test_update_install_one_app_and_uninstall_one() {
        let (mut client, pm_r) = MockExtnClient::main_and_extn(vec![String::from("apps")]);
        let pm_ftr = MockPackageManager::start(
            vec![AppBuilder::new("to_uninstall").installed()],
            client.clone(),
            pm_r,
        );
        let apps_update = AppsUpdate {
            apps: vec![AppBuilder::new("firecert").meta()],
        };

        update(client.clone(), apps_update, vec![], true).await;
        let pm = MockPackageManager::shutdown(&mut client, pm_ftr).await;
        assert_eq!(pm.new_installed_apps.len(), 1);
        assert!(pm.new_installed_apps.iter().any(|a| a.id == "firecert"));
        assert_eq!(pm.new_uninstalled_apps.len(), 1);
        assert!(pm
            .new_uninstalled_apps
            .iter()
            .any(|a| a.id == "to_uninstall"));
    }
}
