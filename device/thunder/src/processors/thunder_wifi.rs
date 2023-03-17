use crate::{client::thunder_plugin::ThunderPlugin, thunder_state::ThunderState};
use ripple_sdk::{
    api::device::{
        device_wifi::WifiRequest,
        device_info_request::DeviceInfoRequest,
        device_operator::{DeviceCallRequest, DeviceOperator},
    },
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::{ExtnMessage, ExtnResponse},
    },
    log::{error, info},
    utils::error::RippleError,
};
use serde_json::json;

#[derive(Debug)]
pub struct ThunderWifiRequestProcessor {
    state: ThunderState,
    streamer: DefaultExtnStreamer,
}

impl ThunderWifiRequestProcessor {
    pub fn new(state: ThunderState) -> ThunderWifiRequestProcessor {
        ThunderWifiRequestProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }

/*
    async fn make(state: ThunderState, req: ExtnMessage) -> bool {
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::System.method("getDeviceInfo"),
                params: None,
            })
            .await;
        info!("{}", response.message);
        let response = match response.message["make"].as_str() {
            Some(v) => ExtnResponse::String(v.to_string()),
            None => ExtnResponse::Error(RippleError::InvalidOutput),
        };

        Self::respond(state.get_client(), req, response)
            .await
            .is_ok()
    }
*/

}

impl ExtnStreamProcessor for ThunderWifiRequestProcessor {
    type STATE = ThunderState;
    type VALUE = DeviceInfoRequest;

    fn get_state(&self) -> Self::STATE {
        self.state.clone()
    }

    fn receiver(&mut self) -> ripple_sdk::tokio::sync::mpsc::Receiver<ExtnMessage> {
        self.streamer.receiver()
    }

    fn sender(&self) -> ripple_sdk::tokio::sync::mpsc::Sender<ExtnMessage> {
        self.streamer.sender()
    }
}

#[async_trait]
impl ExtnRequestProcessor for ThunderWifiRequestProcessor {
    fn get_client(&self) -> ripple_sdk::extn::client::extn_client::ExtnClient {
        self.state.get_client()
    }

    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        match extracted_message {
//            WifiRequest::Scan => Self::
//            DeviceInfoRequest::Make => Self::make(state.clone(), msg).await,
//            DeviceInfoRequest::Model => Self::model(state.clone(), msg).await,
//            DeviceInfoRequest::AvailableMemory => Self::available_memory(state.clone(), msg).await,
            _ => false,
        }
    }
}
