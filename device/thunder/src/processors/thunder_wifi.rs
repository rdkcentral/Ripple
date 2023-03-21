use crate::{client::thunder_plugin::ThunderPlugin, thunder_state::ThunderState};
use crate::processors::thunder_wifi::ThunderPlugin::Wifi;
use jsonrpsee::{types::{response, request}, tracing::{Span, info_span}};
use ripple_sdk::{
    api::device::{
        device_wifi::WifiRequest,
        device_operator::{DeviceCallRequest, DeviceOperator, DeviceChannelParams, DeviceResponseMessage, DeviceSubscribeRequest, DeviceUnsubscribeRequest},
    },
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::{ExtnMessage, ExtnResponse},
    },
    log::{error, info},
    utils::error::RippleError, tokio::{sync::mpsc, self},
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

        async fn wait_for_thunder_ssids( state: ThunderState, req: ExtnMessage) {
                
            info!("subscribing to wifi ssid scan thunder events");

        
                let client = state.get_thunder_client();
        
        //        let (s, mut r) = tokio::sync::mpsc::channel::<ThunderResponseMessage>(32);
                let (sub_tx, mut sub_rx) = mpsc::channel::<DeviceResponseMessage>(32);
        
                //let (resp_tx, resp_rx) = mpsc::channel::<DeviceResponseMessage>();
        
        //        let unsub_client = client.clone();
                client
                    .clone()
                    .subscribe(
                        DeviceSubscribeRequest {
                            module: Wifi.callsign_and_version(),
                            event_name: "onAvailableSSIDs".into(),
                            params: None,
                            sub_id: None,
                        },
                        sub_tx,
                    )
                    .await;
                
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

        if true {
            ThunderWifiRequestProcessor::wait_for_thunder_ssids(
                state.clone(),
                msg.clone(),
            )
            .await;
        } else {
            error!("Wifi scan failure")
        };


        match extracted_message {
            WifiRequest::Scan => Self::scan(state.clone(), msg).await,
            _ => false,
        }
    }
}
