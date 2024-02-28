use std::sync::{Arc, RwLock};

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

// #[derive(Debug, Clone)]
// pub enum PendingSessionRequest {
//     PendingSession(String),
// }

// impl ExtnPayloadProvider for PendingSessionRequest {
//     fn get_extn_payload(&self) -> ExtnPayload {
//         ExtnPayload::Request(ExtnRequest::PendingSession(self.clone()))
//     }

//     fn get_from_payload(payload: ExtnPayload) -> Option<PendingSessionRequest> {
//         if let ExtnPayload::Request(ExtnRequest::PendingSession(r)) = payload {
//             return Some(r);
//         }

//         None
//     }

//     fn contract() -> RippleContract {
//         RippleContract::LifecycleManagement
//     }
// }

#[derive(Debug, Clone)]
pub struct PendingSessionState {
    pub platform_state: PlatformState,
    pub pending_sessions: Arc<RwLock<Vec<String>>>,
}

impl PendingSessionState {
    pub fn new(platform_state: PlatformState) -> PendingSessionState {
        PendingSessionState {
            platform_state,
            pending_sessions: Arc::new(RwLock::new(Vec::default())),
        }
    }
}

pub struct PendingSessionEventProcessor {
    state: PendingSessionState,
    streamer: DefaultExtnStreamer,
}

impl PendingSessionEventProcessor {
    pub fn new(state: PendingSessionState) -> PendingSessionEventProcessor {
        PendingSessionEventProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for PendingSessionEventProcessor {
    type STATE = PendingSessionState;
    type VALUE = AppsUpdate;
    fn get_state(&self) -> Self::STATE {
        self.state.clone()
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
            println!(
                "PendingSessionEventProcessor::process_event: Got AppsUpdate::InstallComplete"
            );
            //let pending_sessions = state.pending_sessions.read().unwrap();
            //if pending_sessions.contains(&operation.id) {
            let has_pending_session = {
                state
                    .pending_sessions
                    .read()
                    .unwrap()
                    .contains(&operation.id)
            };
            if has_pending_session {
                if operation.success {
                    // Send request to create session and send lifecyclemanagement.OnSessionTransitionComplete
                    match PermissionHandler::fetch_permission_for_app_session(
                        &state.platform_state,
                        &operation.id,
                    )
                    .await
                    {
                        Ok(()) => {
                            DelegatedLauncherHandler::emit_completed(
                                &state.platform_state,
                                &operation.id,
                            )
                            .await
                        }
                        Err(e) => {
                            println!(
                            "PendingSessionEventProcessor::process_event: Failed to fetch permissions: app_id={}, e={:?}", operation.id, e);
                            DelegatedLauncherHandler::emit_cancelled(
                                &state.platform_state,
                                &operation.id,
                            )
                            .await;
                        }
                    }
                } else {
                    error!("PendingSessionEventProcessor::process_event: Installation failed: app_id={}", operation.id);
                    DelegatedLauncherHandler::emit_cancelled(&state.platform_state, &operation.id)
                        .await;
                }
                //pending_sessions.retain(|id| !id.eq(&operation.id));
                {
                    state
                        .pending_sessions
                        .write()
                        .unwrap()
                        .retain(|id| !id.eq(&operation.id));
                }
            }
        }
        None
    }
}
