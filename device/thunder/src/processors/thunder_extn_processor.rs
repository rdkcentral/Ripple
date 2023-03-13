use ripple_sdk::{
    api::device::{
        device_operator::{DeviceChannelRequest, DeviceOperator},
        device_request::BaseDeviceRequest,
    },
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_capability::{ExtnCapability, ExtnClass},
        extn_client_message::{ExtnMessage, ExtnResponse},
        ffi::ffi_device::DeviceExtn,
    },
    log::error,
};
use serde_json::Value;

use crate::thunder_state::ThunderState;

#[derive(Debug, Clone)]
pub struct ThunderExtnState {
    state: ThunderState,
    extn: DeviceExtn,
}

pub struct ThunderExtnProcessor {
    state: ThunderExtnState,
    streamer: DefaultExtnStreamer,
}

impl ThunderExtnProcessor {
    pub fn new(state: ThunderState, extn: DeviceExtn) -> ThunderExtnProcessor {
        ThunderExtnProcessor {
            state: ThunderExtnState { state, extn },
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for ThunderExtnProcessor {
    type STATE = ThunderExtnState;
    type VALUE = BaseDeviceRequest;

    fn get_state(&self) -> Self::STATE {
        self.state.clone()
    }

    fn capability(&self) -> ExtnCapability {
        ExtnCapability::new_channel(ExtnClass::Device, self.state.clone().extn.service)
    }

    fn receiver(&mut self) -> ripple_sdk::tokio::sync::mpsc::Receiver<ExtnMessage> {
        self.streamer.receiver()
    }

    fn sender(&self) -> ripple_sdk::tokio::sync::mpsc::Sender<ExtnMessage> {
        self.streamer.sender()
    }
}

#[async_trait]
impl ExtnRequestProcessor for ThunderExtnProcessor {
    fn get_client(&self) -> ripple_sdk::extn::client::extn_client::ExtnClient {
        self.state.state.get_client()
    }

    async fn process_request(
        state: ThunderExtnState,
        msg: ExtnMessage,
        extracted_message: BaseDeviceRequest,
    ) -> bool {
        let extracted_c = extracted_message.clone();
        let request = if extracted_c.params.is_none() {
            Value::Null
        } else {
            extracted_c.params.unwrap()
        };
        let thunder_request: DeviceChannelRequest = (state.extn.get_request)(request).into();
        let client = state.state.get_thunder_client();

        match thunder_request {
            DeviceChannelRequest::Call(c) => {
                let response = client.call(c).await;
                if let Ok(r) = (state.extn.process)(response.message) {
                    if let Err(_e) = state
                        .state
                        .get_client()
                        .send_message(msg.get_response(ExtnResponse::Value(r)).unwrap())
                        .await
                    {
                        error!("Sending back response for extn.processor ");
                        return false;
                    }
                }
            }
            _ => {
                return Self::handle_error(
                    state.state.get_client(),
                    msg,
                    ripple_sdk::utils::error::RippleError::InvalidInput,
                )
                .await
            }
        }
        true
    }
}
