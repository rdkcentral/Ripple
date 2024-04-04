use jsonrpsee::core::async_trait;
use ripple_sdk::{
    api::{
        app_catalog::{self, AppOperationComplete, AppsCatalogUpdate, AppsUpdate},
        device::{
            device_apps::{AppsRequest, DeviceAppMetadata, InstalledApp},
            device_info_request::DeviceInfo,
        },
        manifest::remote_feature::RemoteFeature,
    },
    extn::{
        client::{
            extn_client::ExtnClient,
            extn_processor::{DefaultExtnStreamer, ExtnEventProcessor, ExtnStreamProcessor},
        },
        extn_client_message::{ExtnMessage, ExtnPayload, ExtnResponse},
    },
    log::info,
    tokio::sync::mpsc::{Receiver, Sender},
};

#[cfg(not(test))]
use ripple_sdk::log::{debug, error};
use serde::{Deserialize, Serialize};

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};
#[cfg(test)]
use std::{println as debug, println as error};

use ripple_sdk::extn::client::extn_processor::ExtnStreamer;

use crate::state::platform_state::PlatformState;

#[derive(Clone)]
pub struct AppsUpdaterState {
    client: ExtnClient,
    ignore_list: Vec<String>,
    uninstalls_enabled: bool,
    sideload_support: bool,
    failed_app_installs: Arc<RwLock<Vec<FailedAppInstall>>>,
    failed_app_installs_persist_path: PathBuf,
    pending_installs: Arc<RwLock<Vec<AppInstall>>>,
}

const FAILED_INSTALLS_FILE_NAME: &str = "failed_installs.json";

impl AppsUpdaterState {
    pub fn load_failed_app_installs(persistence_path: &Path) -> Vec<FailedAppInstall> {
        let file_path = persistence_path.join(FAILED_INSTALLS_FILE_NAME);

        let file_res = fs::OpenOptions::new().read(true).open(file_path);
        if file_res.is_err() {
            error!(
                "Failed to load failed app installs records from persistent file, err={:?}",
                file_res.err()
            );
            return vec![];
        }

        let failed = serde_json::from_reader::<_, Vec<FailedAppInstall>>(&file_res.unwrap());
        match failed {
            Ok(f) => f,
            Err(e) => {
                error!(
                    "Failed to load failed app installs records from persistent file, err={:?}",
                    e
                );
                vec![]
            }
        }
    }

    pub fn persist_failed_installs(&self) -> bool {
        let failed_installs = self.failed_app_installs.read().unwrap();
        let path = std::path::Path::new(&self.failed_app_installs_persist_path)
            .join(FAILED_INSTALLS_FILE_NAME);
        match fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
        {
            Ok(file) => serde_json::to_writer_pretty(
                &file,
                &serde_json::to_value(failed_installs.clone()).unwrap(),
            )
            .is_ok(),
            Err(err) => {
                error!(
                    "unable to create file {}: {:?}",
                    FAILED_INSTALLS_FILE_NAME, err
                );
                false
            }
        }
    }

    pub fn success_install(&self, app_id: String) {
        let did_change = {
            let mut failed = self.failed_app_installs.write().unwrap();
            let len_before = failed.len();
            failed.retain(|a| a.app_id != app_id);
            failed.len() != len_before
        };
        if did_change {
            self.persist_failed_installs();
        }
    }

    pub fn fail_install(&self, install: FailedAppInstall) {
        {
            let mut failed = self.failed_app_installs.write().unwrap();
            if let Some(exists_pos) = failed.iter().position(|a| a.app_id == install.app_id) {
                if let Some(exists) = failed.get_mut(exists_pos) {
                    exists.failed_install_version = install.failed_install_version.clone();
                }
            } else {
                failed.push(install);
            }
        }
        self.persist_failed_installs();
    }

    pub fn remove_pending(&mut self, app_id: &String, version: &String) -> Option<AppInstall> {
        let mut pi = self.pending_installs.write().unwrap();

        pi.iter()
            .position(|a| a.app_id == *app_id && a.version == *version)
            .map(|pos| pi.remove(pos))
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FailedAppInstall {
    pub app_id: String,
    pub failed_install_version: String,
    pub last_good_version: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppInstall {
    pub app_id: String,
    pub version: String,
    pub previous_version: Option<String>,
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
        sideload_support: bool,
        failed_app_installs: Vec<FailedAppInstall>,
        failed_app_installs_persist_path: PathBuf,
    ) -> AppsUpdater {
        AppsUpdater {
            state: AppsUpdaterState {
                client,
                ignore_list,
                uninstalls_enabled,
                sideload_support,
                failed_app_installs: Arc::new(RwLock::new(failed_app_installs)),
                failed_app_installs_persist_path,
                pending_installs: Arc::new(RwLock::new(vec![])),
            },
            streamer: DefaultExtnStreamer::new(),
        }
    }

    pub async fn init(state: &PlatformState) -> AppsUpdater {
        let ignore_list = vec![state.get_device_manifest().applications.defaults.main];
        let mut extn_client = state.get_client().get_extn_client();
        let feats = state.get_device_manifest().get_features();
        let sideload_support = DeviceInfo::is_debug(&mut extn_client).await;
        let uninstalls_enabled =
            RemoteFeature::flag(&mut extn_client, feats.catalog_uninstalls_enabled).await;
        info!(
            "Catalog manager uninstalls_enabled={} sideload_support={}",
            uninstalls_enabled, sideload_support
        );
        let persist_path =
            Path::new(&state.get_device_manifest().configuration.saved_dir).join("apps");
        let failed = AppsUpdaterState::load_failed_app_installs(&persist_path);
        AppsUpdater::new(
            extn_client,
            ignore_list,
            uninstalls_enabled,
            sideload_support,
            failed,
            persist_path,
        )
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
        _: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> Option<bool> {
        match extracted_message {
            AppsUpdate::AppsCatalogUpdate(apps_catalog_update) => {
                update(state, apps_catalog_update).await;
            }
            AppsUpdate::InstallComplete(op) => {
                install_complete(state, op);
            }
            AppsUpdate::UninstallComplete(_) => {}
        }
        None
    }
}

/// Checks if the given installed app is
/// the expected app to be installed
/// If it is the same version in the old catalog
/// or if it is the last_good_version before a failed install
fn is_expected_install(
    old_catalog: &Option<Vec<app_catalog::AppMetadata>>,
    failed_installs: &Arc<RwLock<Vec<FailedAppInstall>>>,
    installed_app: &InstalledApp,
) -> Option<bool> {
    let in_old_catalog = old_catalog.as_ref().map(|apps| {
        apps.iter()
            .any(|a| a.id == installed_app.id && a.version == installed_app.version)
    });
    if in_old_catalog.is_some() && in_old_catalog.unwrap() {
        return in_old_catalog;
    }
    // If not in the old catalog, check if it failed to install
    let fis = failed_installs.read().unwrap();
    let failed_install = fis.iter().find(|a| a.app_id == installed_app.id);
    match failed_install {
        Some(fi) => match &fi.last_good_version {
            Some(lgv) => {
                debug!(
                    "Found that {} failed its last install, comparing last good version ({}) with installed version ({})",
                    installed_app.id, lgv, installed_app.version
                );
                Some(*lgv == installed_app.version)
            }
            None => {
                debug!(
                    "Found that {} failed its last install but has never successfully installed from catalog",
                    installed_app.id
                );
                None
            }
        },
        None => in_old_catalog,
    }
}

pub async fn update(mut state: AppsUpdaterState, apps_catalog_update: AppsCatalogUpdate) {
    let resp = state
        .client
        .request(AppsRequest::GetInstalledApps(None))
        .await;
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

    debug!(
        "update: apps_update={:?}, installed_apps={:?}",
        apps_catalog_update,
        installed_apps.clone()
    );

    if state.uninstalls_enabled {
        // Uninstall removed apps

        for installed_app in installed_apps.clone() {
            if state.ignore_list.contains(&installed_app.id) {
                // Skip ignored apps
                continue;
            }

            if !apps_catalog_update
                .new_catalog
                .clone()
                .into_iter()
                .any(|app| app.id.eq(&installed_app.id))
            {
                if state.sideload_support {
                    let expected = is_expected_install(
                        &apps_catalog_update.old_catalog,
                        &state.failed_app_installs,
                        &installed_app,
                    );
                    if expected.is_some() && !expected.unwrap() {
                        debug!(
                            "Skipping uninstalling sideloaded app {}-{}",
                            installed_app.id, installed_app.version
                        );
                        continue;
                    }
                }
                debug!("update: Existing application is no longer in the updated catalog, uninstalling: id={}, version={}", installed_app.id, installed_app.version);

                let resp = state
                    .client
                    .request(AppsRequest::UninstallApp(installed_app))
                    .await;

                if let Err(e) = resp {
                    error!("update: Could not uninstall app: e={:?}", e);
                }
            }
        }
    }

    // Install new/updated apps

    let mut app_list: Vec<app_catalog::AppMetadata> = apps_catalog_update
        .new_catalog
        .into_iter()
        .filter(|a| {
            !installed_apps
                .iter()
                .any(|ia| ia.id.eq(&a.id) && ia.version.eq(&a.version))
        })
        .filter(|a| {
            if !state.sideload_support {
                return true;
            }
            let ia_opt = installed_apps.iter().find(|iacheck| iacheck.id == a.id);
            if ia_opt.is_none() {
                return true;
            }
            let ia = ia_opt.unwrap();

            let expected = is_expected_install(
                &apps_catalog_update.old_catalog,
                &state.failed_app_installs,
                ia,
            );
            if expected.is_some() && !expected.unwrap() {
                debug!("Skipping updating sideloaded app {}-{}", ia.id, ia.version);
                return false;
            }
            true
        })
        .collect();
    app_list.sort_by_key(|app| app.install_priority);
    for app in app_list {
        let v = app.version.clone();
        debug!("update: Application is not currently installed, installing: title={}, id={}, version={}", app.title, app.id, v);
        let metadata = DeviceAppMetadata::new(app.id.clone(), app.title, v, app.uri, app.data);

        let previous_version = match &apps_catalog_update.old_catalog {
            Some(c) => c.iter().find(|a| a.id == app.id).map(|a| a.version.clone()),
            None => None,
        };
        state.pending_installs.write().unwrap().push(AppInstall {
            app_id: app.id.clone(),
            version: app.version.clone(),
            previous_version: previous_version.clone(),
        });
        let resp = state
            .client
            .request_and_flatten(AppsRequest::InstallApp(metadata))
            .await;

        if resp.is_err() {
            error!("update: Could not install app: e={:?}", resp.err().unwrap());
            state.fail_install(FailedAppInstall {
                app_id: app.id,
                failed_install_version: app.version,
                last_good_version: previous_version,
            });
        }
    }
}

fn install_complete(mut state: AppsUpdaterState, op: AppOperationComplete) {
    let pending = state.remove_pending(&op.id, &op.version);
    if op.success {
        state.success_install(op.id);
    } else {
        let last_good_version: Option<String> = match pending {
            Some(p) => p.previous_version,
            None => None,
        };
        state.fail_install(FailedAppInstall {
            app_id: op.id,
            failed_install_version: op.version,
            last_good_version,
        })
    }
}

#[cfg(test)]
pub mod tests {

    use std::{
        path::Path,
        sync::{Arc, RwLock},
        time::Duration,
    };

    use super::{AppsUpdater, AppsUpdaterState};
    use crate::{service::extn::ripple_client::RippleClient, state::platform_state::PlatformState};
    use ripple_sdk::{
        api::{
            app_catalog::{AppMetadata, AppOperationComplete, AppsCatalogUpdate, AppsUpdate},
            device::device_apps::{AppsRequest, DeviceAppMetadata, InstalledApp},
        },
        create_processor,
        extn::{
            client::{
                extn_client::ExtnClient,
                extn_processor::{DefaultExtnStreamer, ExtnStreamer},
            },
            extn_client_message::{ExtnMessage, ExtnResponse},
            mock_extension_client::{MockExtnClient, MockExtnRequest},
        },
        tokio::{self, spawn, sync::mpsc, task::JoinHandle, time::sleep},
    };
    use ripple_sdk::{
        api::{
            config::Config,
            device::device_info_request::{DeviceInfoRequest, DeviceResponse, PlatformBuildInfo},
            manifest::{
                device_manifest::{DeviceManifest, RippleConfiguration, RippleFeatures},
                extn_manifest::ExtnManifest,
                remote_feature::FeatureFlag,
            },
        },
        utils::error::RippleError,
    };
    use rstest::rstest;
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    use super::update;

    use ripple_sdk::extn::client::extn_processor::ExtnStreamProcessor;

    fn test_state(client: ExtnClient) -> AppsUpdaterState {
        AppsUpdaterState {
            client,
            ignore_list: vec![],
            uninstalls_enabled: true,
            sideload_support: false,
            failed_app_installs: Arc::new(RwLock::new(vec![])),
            failed_app_installs_persist_path: Path::new("/tmp").to_path_buf(),
            pending_installs: Arc::new(RwLock::new(vec![])),
        }
    }

    const TEST_APP_ID: &str = "firecert";

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

    enum FailedInstall {
        Sync,
        Async,
    }

    #[derive(Clone, Serialize, Deserialize)]
    enum PMControl {
        SetInstalledVersion(String),
    }

    struct MockPackageManager {
        pre_installed_apps: Vec<InstalledApp>,
        new_installed_apps: Vec<DeviceAppMetadata>,
        new_uninstalled_apps: Vec<InstalledApp>,
    }

    impl MockPackageManager {
        pub fn start(
            pre_installed_apps: Vec<InstalledApp>,
            client: ExtnClient,
            pm_r: mpsc::Receiver<MockExtnRequest<AppsRequest>>,
        ) -> JoinHandle<MockPackageManager> {
            Self::start_with_install_fails(pre_installed_apps, vec![], client, pm_r)
        }

        pub fn start_with_install_fails(
            pre_installed_apps: Vec<InstalledApp>,
            mut failed_installs: Vec<FailedInstall>,
            mut client: ExtnClient,
            mut pm_r: mpsc::Receiver<MockExtnRequest<AppsRequest>>,
        ) -> JoinHandle<MockPackageManager> {
            let mut pm = MockPackageManager {
                pre_installed_apps,
                new_installed_apps: vec![],
                new_uninstalled_apps: vec![],
            };
            spawn(async move {
                while let Some(msg) = pm_r.recv().await {
                    println!("Got msg {:?}", msg);
                    if let MockExtnRequest::Shutdown = msg {
                        return pm;
                    }
                    if let MockExtnRequest::ControlMessage(v) = &msg {
                        let cm = serde_json::from_value::<PMControl>(v.clone());
                        if let Ok(PMControl::SetInstalledVersion(v)) = cm {
                            if let Some(ia) = pm.pre_installed_apps.first_mut() {
                                ia.version = v.clone();
                            }
                            continue;
                        }
                    }
                    let (msg, apps_req) = msg.as_msg().unwrap();
                    match apps_req {
                        AppsRequest::GetInstalledApps(_) => {
                            let response =
                                ExtnResponse::InstalledApps(pm.pre_installed_apps.clone());
                            MockExtnClient::respond_with_payload(&mut client, msg, response).await;
                        }
                        AppsRequest::InstallApp(app) => {
                            pm.new_installed_apps.push(app.clone());
                            if let Some(fail) = failed_installs.pop() {
                                match fail {
                                    FailedInstall::Sync => {
                                        MockExtnClient::respond_with_payload(
                                            &mut client,
                                            msg,
                                            ExtnResponse::Error(RippleError::InvalidOutput),
                                        )
                                        .await;
                                    }
                                    FailedInstall::Async => {
                                        client.send_message(msg.ack()).await.unwrap();
                                        client
                                            .event(AppsUpdate::InstallComplete(
                                                AppOperationComplete {
                                                    id: app.id,
                                                    version: app.version,
                                                    success: false,
                                                },
                                            ))
                                            .ok();
                                    }
                                }
                            } else {
                                client.send_message(msg.ack()).await.unwrap();
                                client
                                    .event(AppsUpdate::InstallComplete(AppOperationComplete {
                                        id: app.id,
                                        version: app.version,
                                        success: true,
                                    }))
                                    .ok();
                            }
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
    }

    create_processor!(MockPMProcessor, AppsRequest);

    /// No apps installed, AppUpdate has one app
    #[tokio::test]
    pub async fn test_update_install_one_app() {
        let (client, mock_extn, extn_rx) = MockPMProcessor::mock_extn_client();
        let pm_ftr = MockPackageManager::start(vec![], client.clone(), extn_rx);
        let apps_update = AppsCatalogUpdate {
            old_catalog: None,
            new_catalog: vec![AppBuilder::new(TEST_APP_ID).meta()],
        };
        update(test_state(client.clone()), apps_update).await;
        mock_extn.shutdown().await;
        let pm = pm_ftr.await.unwrap();
        let apps: Vec<String> = pm.new_installed_apps.iter().map(|x| x.id.clone()).collect();
        assert_eq!(apps.len(), 1);
        assert!(apps.contains(&String::from(TEST_APP_ID)));
    }

    /// AppUpdate has app that is already installed
    #[tokio::test]
    pub async fn test_update_no_changes() {
        let (client, mock_extn, extn_rx) = MockPMProcessor::mock_extn_client();
        let pm_ftr = MockPackageManager::start(
            vec![AppBuilder::new(TEST_APP_ID).installed()],
            client.clone(),
            extn_rx,
        );
        let apps_update = AppsCatalogUpdate {
            old_catalog: None,
            new_catalog: vec![AppBuilder::new(TEST_APP_ID).meta()],
        };
        update(test_state(client.clone()), apps_update).await;

        mock_extn.shutdown().await;
        let pm = pm_ftr.await.unwrap();
        assert_eq!(pm.new_installed_apps.len(), 0);
        assert_eq!(pm.new_uninstalled_apps.len(), 0);
    }

    /// Install many apps of different priority
    #[tokio::test]
    pub async fn test_update_install_in_priority_order() {
        let (client, mock_extn, extn_rx) = MockPMProcessor::mock_extn_client();
        let pm_ftr = MockPackageManager::start(vec![], client.clone(), extn_rx);
        let apps_update = AppsCatalogUpdate {
            old_catalog: None,
            new_catalog: vec![
                AppBuilder::new("no_priority_app").meta(),
                AppBuilder::new("high_priority_app").priority(1).meta(),
                AppBuilder::new("low_priority_app").priority(200).meta(),
                AppBuilder::new("medium_priority_app").priority(50).meta(),
            ],
        };
        update(test_state(client.clone()), apps_update).await;
        mock_extn.shutdown().await;
        let pm = pm_ftr.await.unwrap();
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
        let (client, mock_extn, extn_rx) = MockPMProcessor::mock_extn_client();
        let pm_ftr = MockPackageManager::start(
            vec![AppBuilder::new(TEST_APP_ID).version("1.0").installed()],
            client.clone(),
            extn_rx,
        );
        let apps_update = AppsCatalogUpdate {
            old_catalog: None,
            new_catalog: vec![AppBuilder::new(TEST_APP_ID).version("2.0").meta()],
        };

        update(test_state(client.clone()), apps_update).await;
        mock_extn.shutdown().await;
        let pm = pm_ftr.await.unwrap();
        assert_eq!(pm.new_installed_apps.len(), 1);
        let installed = pm
            .new_installed_apps
            .iter()
            .find(|a| a.id == TEST_APP_ID)
            .unwrap();
        assert_eq!(installed.version, "2.0");
        assert_eq!(pm.new_uninstalled_apps.len(), 0);
    }

    /// app already installed, but update has app missing and uninstalls are enabled
    #[tokio::test]
    pub async fn test_update_unistall_one_with_enabled() {
        let (client, mock_extn, extn_rx) = MockPMProcessor::mock_extn_client();
        let pm_ftr = MockPackageManager::start(
            vec![AppBuilder::new(TEST_APP_ID).installed()],
            client.clone(),
            extn_rx,
        );
        let apps_update = AppsCatalogUpdate {
            old_catalog: None,
            new_catalog: vec![],
        };

        update(test_state(client.clone()), apps_update).await;
        mock_extn.shutdown().await;
        let pm = pm_ftr.await.unwrap();
        assert_eq!(pm.new_installed_apps.len(), 0);
        assert_eq!(pm.new_uninstalled_apps.len(), 1);
        assert!(pm.new_uninstalled_apps.iter().any(|a| a.id == TEST_APP_ID));
    }

    /// app already installed, but update has app missing and uninstalls are disabled
    #[tokio::test]
    pub async fn test_update_unistall_one_with_disabled() {
        let (client, mock_extn, extn_rx) = MockPMProcessor::mock_extn_client();
        let pm_ftr = MockPackageManager::start(
            vec![AppBuilder::new(TEST_APP_ID).installed()],
            client.clone(),
            extn_rx,
        );
        let apps_update = AppsCatalogUpdate {
            old_catalog: None,
            new_catalog: vec![],
        };

        update(
            AppsUpdaterState {
                uninstalls_enabled: false,
                ..test_state(client.clone())
            },
            apps_update,
        )
        .await;
        mock_extn.shutdown().await;
        let pm = pm_ftr.await.unwrap();
        assert_eq!(pm.new_installed_apps.len(), 0);
        assert_eq!(pm.new_uninstalled_apps.len(), 0);
    }

    /// Install one new app, Uninstall one old app
    #[tokio::test]
    pub async fn test_update_install_one_app_and_uninstall_one() {
        let (client, mock_extn, extn_rx) = MockPMProcessor::mock_extn_client();
        let pm_ftr = MockPackageManager::start(
            vec![AppBuilder::new("to_uninstall").installed()],
            client.clone(),
            extn_rx,
        );
        let apps_update = AppsCatalogUpdate {
            old_catalog: None,
            new_catalog: vec![AppBuilder::new(TEST_APP_ID).meta()],
        };

        update(test_state(client.clone()), apps_update).await;
        mock_extn.shutdown().await;
        let pm = pm_ftr.await.unwrap();
        assert_eq!(pm.new_installed_apps.len(), 1);
        assert!(pm.new_installed_apps.iter().any(|a| a.id == TEST_APP_ID));
        assert_eq!(pm.new_uninstalled_apps.len(), 1);
        assert!(pm
            .new_uninstalled_apps
            .iter()
            .any(|a| a.id == "to_uninstall"));
    }

    fn cat_update(previous_version: &str, next_version: Option<&str>) -> AppsCatalogUpdate {
        AppsCatalogUpdate {
            old_catalog: Some(vec![AppBuilder::new(TEST_APP_ID)
                .version(previous_version)
                .meta()]),
            new_catalog: match next_version {
                Some(v) => vec![AppBuilder::new(TEST_APP_ID).version(v).meta()],
                None => vec![],
            },
        }
    }

    pub async fn test_update_and_assert_counts(
        sideload_support: bool,
        installed_version: &str,
        previous_version: &str,
        next_version: Option<&str>,
        exp_installs: usize,
        exp_uninstalls: usize,
    ) {
        let (client, mock_extn, extn_rx) = MockPMProcessor::mock_extn_client();
        let pm_ftr = MockPackageManager::start(
            vec![AppBuilder::new(TEST_APP_ID)
                .version(installed_version)
                .installed()],
            client.clone(),
            extn_rx,
        );
        let apps_update = cat_update(previous_version, next_version);
        update(
            AppsUpdaterState {
                sideload_support,
                ..test_state(client.clone())
            },
            apps_update,
        )
        .await;
        mock_extn.shutdown().await;
        let pm = pm_ftr.await.unwrap();
        assert_eq!(pm.new_installed_apps.len(), exp_installs);
        assert_eq!(pm.new_uninstalled_apps.len(), exp_uninstalls);
    }

    /// App catalog has a new version for an app, but the current version is sideloaded
    #[tokio::test]
    pub async fn test_do_not_update_side_loaded_apps() {
        test_update_and_assert_counts(true, "1.2", "1.0", Some("1.1"), 0, 0).await;
    }

    /// App catalog has a new version for an app, but the current version is sideloaded but sideload_support=false (Prod device)
    #[tokio::test]
    pub async fn test_do_update_side_loaded_apps_when_no_support() {
        test_update_and_assert_counts(false, "1.2", "1.0", Some("1.1"), 1, 0).await;
    }

    /// This is a second update with a new version of an app. Side load supported, but app is not side loaded
    #[tokio::test]
    pub async fn test_normal_update_with_last_update() {
        test_update_and_assert_counts(true, "1.0", "1.0", Some("1.1"), 1, 0).await;
    }

    /// App catalog is empty, one app is installed and is side loaded
    #[tokio::test]
    pub async fn test_do_not_uninstall_side_loaded_apps() {
        test_update_and_assert_counts(true, "1.1", "1.0", None, 0, 0).await;
    }

    /// App catalog is empty, one app is installed and is side loaded but sideload_support=false (prod device)
    #[tokio::test]
    pub async fn test_do_uninstall_side_loaded_apps_when_no_support() {
        test_update_and_assert_counts(false, "1.1", "1.0", None, 0, 1).await;
    }

    /// Second update, App catalog is empty, one app is installed and is NOT sideloaded
    #[tokio::test]
    pub async fn test_normal_uninstall_with_last_update() {
        test_update_and_assert_counts(true, "1.0", "1.0", None, 0, 1).await;
    }

    /// Installing an app fails (sync) during an update, then during a second update it should still update
    /// App should not be seen as sideloaded
    #[tokio::test]
    pub async fn test_update_with_failure_sync() {
        test_update_with_failure(FailedInstall::Sync).await;
    }

    /// Installing an app fails (async) during an update, then during a second update it should still update
    /// App should not be seen as sideloaded
    #[tokio::test]
    pub async fn test_update_with_failure_async() {
        test_update_with_failure(FailedInstall::Async).await;
    }

    async fn test_update_with_failure(failure_type: FailedInstall) {
        let (mut client, mock_extn, extn_rx) = MockPMProcessor::mock_extn_client();
        let pm_ftr = MockPackageManager::start_with_install_fails(
            vec![AppBuilder::new(TEST_APP_ID).version("1.0").installed()],
            vec![failure_type],
            client.clone(),
            extn_rx,
        );

        let state = AppsUpdaterState {
            sideload_support: true,
            ..test_state(client.clone())
        };

        let apps_updater = AppsUpdater {
            state: state.clone(),
            streamer: DefaultExtnStreamer::new(),
        };
        client.add_event_processor(apps_updater);

        let first_apps_update = cat_update("1.0", Some("1.1"));
        update(state.clone(), first_apps_update).await; // first one has install failed

        sleep(Duration::from_secs(1)).await; // wait some time for the async install completion event to come in
        let second_apps_update = cat_update("1.1", Some("1.1"));
        update(state, second_apps_update).await;
        mock_extn.shutdown().await;
        let pm = pm_ftr.await.unwrap();
        // should be 2 install attempts
        assert_eq!(pm.new_installed_apps.len(), 2);
        assert_eq!(pm.new_uninstalled_apps.len(), 0);
    }

    /// Update1: install fails
    /// Update2: install succeeds
    /// App side loaded
    /// Update3: should skip install
    /// This validate we clear the failure status of an app after a successful install
    #[tokio::test]
    pub async fn test_update_with_failure_then_success_then_sideload() {
        let (mut client, mock_extn, extn_rx) = MockPMProcessor::mock_extn_client();

        let state = AppsUpdaterState {
            sideload_support: true,
            ..test_state(client.clone())
        };

        let apps_updater = AppsUpdater {
            state: state.clone(),
            streamer: DefaultExtnStreamer::new(),
        };
        client.add_event_processor(apps_updater);

        let pm_ftr = MockPackageManager::start_with_install_fails(
            vec![AppBuilder::new(TEST_APP_ID).version("1.0").installed()],
            vec![FailedInstall::Sync],
            client.clone(),
            extn_rx,
        );

        let first_apps_update = cat_update("1.0", Some("1.1"));
        update(state.clone(), first_apps_update).await; // first one has install failed
        let second_apps_update = cat_update("1.1", Some("1.1"));
        update(state.clone(), second_apps_update).await;
        // sideload back to 1.0
        mock_extn
            .send_control(json!(PMControl::SetInstalledVersion(String::from("1.0"))))
            .await;

        let third_apps_update = cat_update("1.1", Some("1.2"));
        update(state, third_apps_update).await;
        // should be 2 install attempts (3rd should be skipped)
        mock_extn.shutdown().await;
        let pm = pm_ftr.await.unwrap();
        assert_eq!(pm.new_installed_apps.len(), 2);
        assert_eq!(pm.new_uninstalled_apps.len(), 0);
    }

    create_processor!(MockDeviceInfo, DeviceInfoRequest);
    create_processor!(MockConfig, Config);

    #[rstest(
        device_debug,
        enabled_default,
        rfc_value,
        exp_uninstall_enabled,
        exp_sideload_support,
        case(true, true, Some(true), true, true),
        case(true, false, Some(true), true, true),
        case(true, true, Some(false), false, true),
        case(true, true, None, true, true),
        case(true, false, None, false, true),
        case(false, true, None, true, false),
        case(false, false, None, false, false)
    )]
    #[tokio::test]
    pub async fn test_init_apps_updater(
        device_debug: bool,
        enabled_default: bool,
        rfc_value: Option<bool>,
        exp_uninstall_enabled: bool,
        exp_sideload_support: bool,
    ) {
        let mut client = MockExtnClient::main();
        let (mock_di_extn, mut di_extn_rx) = MockDeviceInfo::add(&mut client);
        let (mock_cfg_extn, mut cfg_extn_rx) = MockConfig::add(&mut client);
        MockExtnClient::start(client.clone());
        let mut client_for_di = client.clone();
        let mut client_for_cfg = client.clone();
        // Set the RippleFeatures on the device manifest for the test
        let dev_man = DeviceManifest {
            configuration: RippleConfiguration {
                features: RippleFeatures {
                    catalog_uninstalls_enabled: FeatureFlag {
                        default: enabled_default,
                        remote_key: Some(String::from("enable_uninstalls")),
                    },
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };
        let state = PlatformState::new(
            ExtnManifest::default(),
            dev_man,
            RippleClient::test_client(client.clone()),
            vec![],
            None,
        );
        spawn(async move {
            while let Some(msg) = di_extn_rx.recv().await {
                if let MockExtnRequest::Shutdown = msg {
                    return;
                }
                if let MockExtnRequest::Message(m, DeviceInfoRequest::PlatformBuildInfo) = msg {
                    let response = DeviceResponse::PlatformBuildInfo(PlatformBuildInfo {
                        debug: device_debug,
                        ..Default::default()
                    });
                    MockExtnClient::respond_with_payload(&mut client_for_di, m, response).await;
                }
            }
        });
        spawn(async move {
            while let Some(msg) = cfg_extn_rx.recv().await {
                if let MockExtnRequest::Shutdown = msg {
                    return;
                }
                if let MockExtnRequest::Message(m, Config::RFC(_)) = msg {
                    let resp = ExtnResponse::Value(serde_json::json!(rfc_value));
                    MockExtnClient::respond_with_payload(&mut client_for_cfg, m, resp).await;
                }
            }
        });
        let updater = AppsUpdater::init(&state).await;
        let state = updater.get_state();
        mock_di_extn.shutdown().await;
        mock_cfg_extn.shutdown().await;
        assert_eq!(state.uninstalls_enabled, exp_uninstall_enabled);
        assert_eq!(state.sideload_support, exp_sideload_support);
    }
}
