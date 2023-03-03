use crate::{client::thunder_plugin::ThunderPlugin, thunder_state::ThunderState};
use ripple_sdk::{
    api::device::{
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
pub struct ThunderDeviceInfoRequestProcessor {
    state: ThunderState,
    streamer: DefaultExtnStreamer,
}

impl ThunderDeviceInfoRequestProcessor {
    pub fn new(state: ThunderState) -> ThunderDeviceInfoRequestProcessor {
        ThunderDeviceInfoRequestProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }

    pub async fn make(state: ThunderState, req: ExtnMessage) {
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

        if let Err(_e) = Self::respond(state.get_client(), req, response).await {
            error!("Sending back response for device.make ");
        }
    }

    async fn model(state: ThunderState, req: ExtnMessage) {
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::System.method("getSystemVersions"),
                params: None,
            })
            .await;
        info!("{}", response.message);
        let response = match response.message["stbVersion"].as_str() {
            Some(v) => {
                let split_string: Vec<&str> = v.split("_").collect();
                ExtnResponse::String(String::from(split_string[0]))
            }
            None => ExtnResponse::Error(RippleError::InvalidOutput),
        };
        if let Err(_e) = Self::respond(state.get_client(), req, response).await {
            error!("Sending back response for device.model ");
        }
    }

    async fn available_memory(state: ThunderState, req: ExtnMessage) {
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::RDKShell.method("getSystemMemory"),
                params: None,
            })
            .await;
        info!("{}", response.message);
        if response.message.get("success").is_some()
            && response.message["success"].as_bool().unwrap() == true
        {
            if let Some(v) = response.message["freeRam"].as_u64() {
                if let Err(_e) =
                    Self::respond(state.get_client(), req, ExtnResponse::Value(json!(v))).await
                {
                    error!("Sending back response for device.model ");
                }
            }
            return;
        }
        error!("{}", response.message);
        Self::process_error(state.clone(), req, RippleError::ProcessorError).await;
    }
}

impl ExtnStreamProcessor for ThunderDeviceInfoRequestProcessor {
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
impl ExtnRequestProcessor for ThunderDeviceInfoRequestProcessor {
    async fn process_error(
        _state: Self::STATE,
        _msg: ExtnMessage,
        _error: ripple_sdk::utils::error::RippleError,
    ) -> Option<bool> {
        error!("Invalid message");
        None
    }

    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> Option<bool> {
        match extracted_message {
            DeviceInfoRequest::Make => Self::make(state.clone(), msg).await,
            DeviceInfoRequest::Model => Self::model(state.clone(), msg).await,
            DeviceInfoRequest::AvailableMemory => Self::available_memory(state.clone(), msg).await,
            _ => {}
        }
        None
    }
}
