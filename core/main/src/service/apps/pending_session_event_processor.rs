use ripple_sdk::extn::client::extn_processor::ExtnStreamer;
use ripple_sdk::{
    api::app_catalog::AppsUpdate,
    async_trait::async_trait,
    extn::{
        client::extn_processor::{DefaultExtnStreamer, ExtnEventProcessor, ExtnStreamProcessor},
        extn_client_message::ExtnMessage,
    },
    log::error,
    tokio::sync::mpsc::Sender,
};

use crate::{
    service::apps::delegated_launcher_handler::DelegatedLauncherHandler,
    state::{cap::permitted_state::PermissionHandler, platform_state::PlatformState},
};

pub struct PendingSessionEventProcessor {
    platform_state: PlatformState,
    streamer: DefaultExtnStreamer,
}

impl PendingSessionEventProcessor {
    pub fn new(platform_state: PlatformState) -> PendingSessionEventProcessor {
        PendingSessionEventProcessor {
            platform_state,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for PendingSessionEventProcessor {
    type STATE = PlatformState;
    type VALUE = AppsUpdate;
    fn get_state(&self) -> Self::STATE {
        self.platform_state.clone()
    }

    fn sender(&self) -> Sender<ExtnMessage> {
        self.streamer.sender()
    }

    fn receiver(&mut self) -> ripple_sdk::tokio::sync::mpsc::Receiver<ExtnMessage> {
        self.streamer.receiver()
    }
}

#[async_trait]
impl ExtnEventProcessor for PendingSessionEventProcessor {
    async fn process_event(
        state: Self::STATE,
        _msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> Option<bool> {
        if let AppsUpdate::InstallComplete(operation) = extracted_message.clone() {
            if let Some(pending_session_info) =
                state.session_state.get_pending_session_info(&operation.id)
            {
                if operation.success {
                    // Send request to create session and send lifecyclemanagement.OnSessionTransitionComplete
                    match PermissionHandler::fetch_permission_for_app_session(&state, &operation.id)
                        .await
                    {
                        Ok(()) => {
                            if let Some(info) = pending_session_info {
                                DelegatedLauncherHandler::check_grants_then_load_or_activate(
                                    &state, info,
                                )
                                .await;
                            }
                            DelegatedLauncherHandler::emit_completed(&state, &operation.id).await;
                        }
                        Err(e) => {
                            error!(
                            "PendingSessionEventProcessor::process_event: Failed to fetch permissions: app_id={}, e={:?}", operation.id, e);
                            DelegatedLauncherHandler::emit_cancelled(&state, &operation.id).await;
                        }
                    }
                } else {
                    error!("PendingSessionEventProcessor::process_event: Installation failed: app_id={}", operation.id);
                    DelegatedLauncherHandler::emit_cancelled(&state, &operation.id).await;
                }
            }
        }
        None
    }
}
