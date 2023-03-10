use ripple_sdk::{
    api::{
        apps::{AppMethod, AppRequest, AppResponse},
        firebolt::fb_lifecycle_management::LifecycleManagementRequest,
    },
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::{ExtnMessage, ExtnPayload, ExtnPayloadProvider},
    },
    log::error,
    tokio::sync::{mpsc::Sender, oneshot},
    utils::error::RippleError,
};

use crate::service::extn::ripple_client::RippleClient;

/// Processor to service incoming Lifecycle Requests from launcher extension.
#[derive(Debug)]
pub struct LifecycleManagementProcessor {
    client: RippleClient,
    streamer: DefaultExtnStreamer,
}

impl LifecycleManagementProcessor {
    pub fn new(client: RippleClient) -> LifecycleManagementProcessor {
        LifecycleManagementProcessor {
            client,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for LifecycleManagementProcessor {
    type STATE = RippleClient;
    type VALUE = LifecycleManagementRequest;
    fn get_state(&self) -> Self::STATE {
        self.client.clone()
    }

    fn sender(&self) -> Sender<ExtnMessage> {
        self.streamer.sender()
    }

    fn receiver(&mut self) -> ripple_sdk::tokio::sync::mpsc::Receiver<ExtnMessage> {
        self.streamer.receiver()
    }
}

#[async_trait]
impl ExtnRequestProcessor for LifecycleManagementProcessor {
    fn get_client(&self) -> ripple_sdk::extn::client::extn_client::ExtnClient {
        self.client.get_extn_client()
    }

    async fn process_request(state: Self::STATE, msg: ExtnMessage, request: Self::VALUE) -> bool {
        // Safety handler
        if !msg.requestor.is_launcher_channel() {
            return Self::handle_error(state.get_extn_client(), msg, RippleError::InvalidAccess)
                .await;
        }

        let (resp_tx, resp_rx) = oneshot::channel::<AppResponse>();
        let method = match request {
            LifecycleManagementRequest::Session(s) => AppMethod::BrowserSession(s.session),
            LifecycleManagementRequest::SetState(s) => AppMethod::SetState(s.app_id, s.state),
        };
        if let Err(e) = state.send_app_request(AppRequest::new(method, resp_tx)) {
            error!("Sending to App manager {:?}", e);
            return Self::handle_error(state.get_extn_client(), msg, e).await;
        }
        let resp = resp_rx.await;
        if let Ok(app_response) = resp {
            if let ExtnPayload::Response(payload) = app_response.get_extn_payload() {
                return Self::respond(state.get_extn_client(), msg, payload)
                    .await
                    .is_ok();
            }
        }

        true
    }
}
