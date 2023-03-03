use ripple_sdk::{
    api::firebolt::{
        fb_discovery::LaunchRequest, fb_lifecycle_management::LifecycleManagementEventRequest,
    },
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnEventProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::ExtnMessage,
    },
    log::error,
    tokio::sync::{mpsc::Receiver as MReceiver, mpsc::Sender as MSender},
};

use crate::{launcher_state::LauncherState, manager::app_launcher::AppLauncher};

#[derive(Debug)]
pub struct LauncherLifecycleEventProcessor {
    state: LauncherState,
    streamer: DefaultExtnStreamer,
}

/// Event processor used for cases where a certain Extension Capability is required to be ready.
/// Bootstrap uses the [WaitForStatusReadyEventProcessor] to await during Device Connnection before starting the gateway.
impl LauncherLifecycleEventProcessor {
    pub fn new(state: LauncherState) -> LauncherLifecycleEventProcessor {
        LauncherLifecycleEventProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for LauncherLifecycleEventProcessor {
    type VALUE = LifecycleManagementEventRequest;
    type STATE = LauncherState;

    fn get_state(&self) -> Self::STATE {
        self.state.clone()
    }

    fn sender(&self) -> MSender<ExtnMessage> {
        self.streamer.sender()
    }

    fn receiver(&mut self) -> MReceiver<ExtnMessage> {
        self.streamer.receiver()
    }
}

#[async_trait]
impl ExtnEventProcessor for LauncherLifecycleEventProcessor {
    async fn process_event(
        state: Self::STATE,
        _msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> Option<bool> {
        let result = match extracted_message {
            LifecycleManagementEventRequest::Ready(ready) => {
                AppLauncher::ready(state.clone(), &ready.parameters.app_id).await
            }
            LifecycleManagementEventRequest::Close(close) => {
                AppLauncher::close(
                    state.clone(),
                    &close.parameters.app_id,
                    close.parameters.reason,
                )
                .await
            }
            LifecycleManagementEventRequest::Finished(finished) => {
                AppLauncher::destroy(state.clone(), &finished.parameters.app_id).await
            }
            LifecycleManagementEventRequest::Launch(launch) => {
                AppLauncher::launch(
                    state.clone(),
                    LaunchRequest {
                        app_id: launch.parameters.app_id,
                        intent: launch.parameters.intent,
                    },
                )
                .await
            }
        };
        if let Err(e) = result {
            error!("Error during lifecycle management processing {:?}", e)
        }
        None
    }
}
