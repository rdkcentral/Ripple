use jsonrpsee::core::async_trait;
use ripple_sdk::{
    api::{
        app_catalog::AppsUpdate,
        device::device_apps::{AppMetadata, AppsRequest},
    },
    extn::{
        client::{
            extn_client::ExtnClient,
            extn_processor::{DefaultExtnStreamer, ExtnEventProcessor, ExtnStreamProcessor},
        },
        extn_client_message::{ExtnMessage, ExtnPayload, ExtnResponse},
    },
    log::{debug, error},
    tokio::sync::mpsc::{Receiver, Sender},
};

use ripple_sdk::extn::client::extn_processor::ExtnStreamer;

#[derive(Clone)]
pub struct AppsUpdaterState {
    client: ExtnClient,
    ignore_list: Vec<String>,
}

pub struct AppsUpdater {
    state: AppsUpdaterState,
    streamer: DefaultExtnStreamer,
}

impl AppsUpdater {
    pub fn new(client: ExtnClient, ignore_list: Vec<String>) -> AppsUpdater {
        AppsUpdater {
            state: AppsUpdaterState {
                client,
                ignore_list,
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
        update(state.client.clone(), extracted_message, state.ignore_list).await;
        None
    }
}

pub async fn update(mut client: ExtnClient, apps_update: AppsUpdate, ignore_list: Vec<String>) {
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

    // Uninstall removed apps

    for installed_app in installed_apps.clone() {
        if ignore_list.contains(&installed_app.id) {
            // Skip ignored apps
            continue;
        }

        if apps_update
            .apps
            .clone()
            .into_iter()
            .find(|app| app.id.eq(&installed_app.id))
            .is_none()
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

    // Install new/updated apps

    debug!(
        "update: apps_update={:?}, installed_apps={:?}",
        apps_update,
        installed_apps.clone()
    );

    for app in apps_update.apps {
        let installed_ids = installed_apps.clone();
        if installed_ids
            .into_iter()
            .find(|installed_app| {
                installed_app.id.eq(&app.id) && installed_app.version.eq(&app.version)
            })
            .is_none()
        {
            debug!("update: Application is not currently installed, installing: title={}, id={}, version={}", app.title, app.id, app.version);
            let metadata = AppMetadata::new(app.id, app.title, app.version, app.uri, app.data);
            let resp = client.request(AppsRequest::InstallApp(metadata)).await;

            if let Err(e) = resp {
                error!("update: Could not install app: e={:?}", e);
            }
        }
    }
}
