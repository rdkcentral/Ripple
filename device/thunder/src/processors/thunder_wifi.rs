use crate::{client::thunder_plugin::ThunderPlugin, thunder_state::ThunderState};
use jsonrpsee::types::{response, request};
use ripple_sdk::{
    api::device::{
        device_wifi::WifiRequest,
        device_operator::{DeviceCallRequest, DeviceOperator, DeviceChannelParams},
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
use serde::{Deserialize,Serialize};

#[derive(Debug)]
pub struct ThunderWifiRequestProcessor {
    state: ThunderState,
    streamer: DefaultExtnStreamer,
}

#[derive(Debug, Serialize, Deserialize)]
struct WifiRequestHeader {
    callsign: String,
    client: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ThunderWifiScanRequest {
    pub incremental: bool,
}

impl ThunderWifiRequestProcessor {
    pub fn new(state: ThunderState) -> ThunderWifiRequestProcessor {
        ThunderWifiRequestProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }

    async fn scan(state: ThunderState, req: ExtnMessage) -> bool {
        let start_scan: String = ThunderPlugin::Wifi.method("startScan");
        let request: ThunderWifiScanRequest = ThunderWifiScanRequest { incremental: false };
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest { 
                method: start_scan, 
                params: Some(DeviceChannelParams::Json(serde_json::to_string(&request).unwrap(),
            )),
             })
             .await;
        info!("fasil {}",response.message);
        let response = match response.message["success"].as_bool() {
            Some(v) => ExtnResponse::String(v.to_string()),
            None => ExtnResponse::Error(RippleError::InvalidOutput),
        };
        
        info!("fasil {:?}",response);
        Self::respond(state.get_client(), req, response)
            .await
            .is_ok()            
    }

}

impl ExtnStreamProcessor for ThunderWifiRequestProcessor {
    type STATE = ThunderState;
    type VALUE = WifiRequest;

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
            WifiRequest::Scan => Self::scan(state.clone(), msg).await,
            _ => false,
        }
    }
}
