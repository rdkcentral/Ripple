use ripple_sdk::{
    api::config::{Config, ConfigResponse},
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
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
    streamer: DefaultExtnStreamer,
}

impl ConfigRequestProcessor {
    pub fn new(state: PlatformState) -> ConfigRequestProcessor {
        ConfigRequestProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for ConfigRequestProcessor {
    type STATE = PlatformState;
    type VALUE = Config;
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
impl ExtnRequestProcessor for ConfigRequestProcessor {
    async fn process_error(
        _state: Self::STATE,
        _msg: ExtnMessage,
        _error: ripple_sdk::utils::error::RippleError,
    ) -> Option<bool> {
        error!("Invalid config request");
        None
    }

    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> Option<bool> {
        let device_manifest = state.get_device_manifest();
        let config_request = extracted_message;
        let response = match config_request {
            Config::PlatformParameters => {
                ExtnResponse::Value(device_manifest.configuration.platform_parameters.clone())
            }
            Config::AllDefaultApps => ExtnResponse::Config(ConfigResponse::AllApps(
                state.app_library_state.get_all_apps(),
            )),
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
