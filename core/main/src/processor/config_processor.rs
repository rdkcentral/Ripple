use ripple_sdk::{
    api::config::Config,
    async_trait::async_trait,
    extn::{
        client::extn_processor::{ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer},
        extn_client_message::{ExtnMessage, ExtnResponse},
    },
    log::error,
    tokio::sync::mpsc::{Receiver as MReceiver, Sender as MSender},
};

use crate::state::platform_state::PlatformState;

/// Supports processing of [Config] request from extensions and also
/// internal services.
#[derive(Debug)]
pub struct ConfigRequestProcessor {
    state: PlatformState,
    streamer: ExtnStreamer,
}

impl ConfigRequestProcessor {
    pub fn new(state: PlatformState) -> ConfigRequestProcessor {
        ConfigRequestProcessor {
            state,
            streamer: ExtnStreamer::new(),
        }
    }
}

#[async_trait]
impl ExtnStreamProcessor for ConfigRequestProcessor {
    type S = PlatformState;
    type V = Config;
    fn get_state(&self) -> Self::S {
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
impl ExtnRequestProcessor for ConfigRequestProcessor {
    async fn process_error(
        _state: Self::S,
        _msg: ExtnMessage,
        _error: ripple_sdk::utils::error::RippleError,
    ) -> Option<bool> {
        error!("Invalid config request");
        None
    }

    async fn process_request(
        state: Self::S,
        msg: ExtnMessage,
        extracted_message: Self::V,
    ) -> Option<bool> {
        let device_manifest = state.get_device_manifest();
        let config_request = extracted_message;
        let response = match config_request {
            Config::PlatformParameters => {
                ExtnResponse::Value(device_manifest.configuration.platform_parameters.clone())
            }
            _ => ExtnResponse::Error(ripple_sdk::utils::error::RippleError::InvalidInput),
        };
        if let Ok(response_ext_message) = msg.get_response(response) {
            if let Err(e) = state.respond(response_ext_message).await {
                error!("Error sending config response back {:?}", e);
            }
        } else {
            error!("Not a valid request object");
        }
        None
    }
}
