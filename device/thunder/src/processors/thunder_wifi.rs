use crate::{client::thunder_plugin::ThunderPlugin, thunder_state::ThunderState};
use crate::processors::thunder_wifi::ThunderPlugin::Wifi;
use jsonrpsee::{types::{response, request}, tracing::{Span, info_span}};
use ripple_sdk::tokio::runtime::Handle;
use ripple_sdk::{
    api::device::{
        device_wifi::{WifiRequest,AccessPoint,AccessPointList,WifiSecurityMode},
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


pub fn wifi_security_mode_to_u32(v: WifiSecurityMode) -> u32 {
    match v {
        WifiSecurityMode::Wep64 => 1,
        WifiSecurityMode::Wep128 => 2,
        WifiSecurityMode::WpaPskTkip => 3,
        WifiSecurityMode::WpaPskAes => 4,
        WifiSecurityMode::Wpa2PskTkip => 5,
        WifiSecurityMode::Wpa2PskAes => 6,
        WifiSecurityMode::WpaEnterpriseTkip => 7,
        WifiSecurityMode::WpaEnterpriseAes => 8,
        WifiSecurityMode::Wpa2EnterpriseTkip => 9,
        WifiSecurityMode::Wpa2EnterpriseAes => 10,
        WifiSecurityMode::Wpa2Psk => 11,
        WifiSecurityMode::Wpa2Enterprise => 12,
        WifiSecurityMode::Wpa3PskAes => 13,
        WifiSecurityMode::Wpa3Sae => 14,
        WifiSecurityMode::None => 0,
    }
}

pub fn wifi_security_mode_from_u32(v: u32) -> WifiSecurityMode {
    match v {
        1 => WifiSecurityMode::Wep64,
        2 => WifiSecurityMode::Wep128,
        3 => WifiSecurityMode::WpaPskTkip,
        4 => WifiSecurityMode::WpaPskAes,
        5 => WifiSecurityMode::Wpa2PskTkip,
        6 => WifiSecurityMode::Wpa2PskAes,
        7 => WifiSecurityMode::WpaEnterpriseTkip,
        8 => WifiSecurityMode::WpaEnterpriseAes,
        9 => WifiSecurityMode::Wpa2EnterpriseTkip,
        10 => WifiSecurityMode::Wpa2EnterpriseAes,
        11 => WifiSecurityMode::Wpa2Psk,
        12 => WifiSecurityMode::Wpa2Enterprise,
        13 => WifiSecurityMode::Wpa3PskAes,
        14 => WifiSecurityMode::Wpa3Sae,
        0 | _ => WifiSecurityMode::None,
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThunderSSID {
    ssid: String,
    security: u32,
    signal_strength: i32,
    frequency: f32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SSIDEventResponse {
    more_data: bool,
    ssids: Vec<ThunderSSID>,
}

impl ThunderSSID {
    fn to_access_point(self: Box<Self>) -> AccessPoint {
        AccessPoint {
            ssid: self.ssid.clone(),
            security_mode: wifi_security_mode_from_u32(self.security),
            signal_strength: self.signal_strength,
            frequency: self.frequency,
        }
    }

    fn to_frequency(s: String) -> f32 {
        match s.parse::<f32>() {
            Ok(v) => v,
            Err(_e) => {
                error!("invalid frequency {}", _e);
                0f32
            }
        }
    }
}


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
        info!("fasil {}",response.message.clone());

             if let response = response.message["success"].as_bool() {
                info!("fasil inside function");

                let result_ssid =  ThunderWifiRequestProcessor::wait_for_thunder_ssids(
                    state.clone(),
                    req.clone(),
                ).await;

                info!("fasil result_ssid {:?}",result_ssid);
                ExtnResponse::WifiScanListResponse(result_ssid);
             }

             true
/*
        let response = match response.message["success"].as_bool() {
            Some(val) => {
            let result_ssid =  ThunderWifiRequestProcessor::wait_for_thunder_ssids(
                    state.clone(),
                    req.clone(),
                ).await;

            info!("fasil {}",response.message);

                let ap_resp = AccessPointList {
                    list: vec![
                             AccessPoint { ssid: (String::from("Test-1")), security_mode: (ripple_sdk::api::device::device_wifi::WifiSecurityMode::None), signal_strength: (0), frequency: (0.0)},
                             AccessPoint { ssid: (String::from("Test-2")), security_mode: (ripple_sdk::api::device::device_wifi::WifiSecurityMode::None), signal_strength: (0), frequency: (0.0)}
                         ] };            
            info!("thread res {:?}",result_ssid);
            ExtnResponse::WifiScanListResponse(ap_resp)
                    }
            None => ExtnResponse::Error(RippleError::InvalidOutput),
        };
 */       
//        info!("fasil {:?}",response);
//        Self::respond(state.get_client(), req, response)
//            .await
//            .is_ok()            
    }

    async fn wait_for_thunder_ssids( state: ThunderState, req: ExtnMessage) -> AccessPointList {      

            let (tx, mut rx) = mpsc::channel::<AccessPointList>(32);

            info!("subscribing to wifi ssid scan thunder events");        
            let client = state.get_thunder_client();
            let (sub_tx, mut sub_rx) = mpsc::channel::<DeviceResponseMessage>(32);
            let unsub_client = client.clone();
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
                   // spawn a thread that handles all scan events, handle the success and error events

    let Handle =    tokio::spawn(async move {
            if let Some(m) = sub_rx.recv().await {
                let mut list = Vec::new();
                let ssid_response: SSIDEventResponse = serde_json::from_value(m.message).unwrap();
                let mut dedup = Vec::new();
                for ssid in ssid_response.ssids {
                    let check_ssid = ssid.ssid.clone();
                    if !dedup.contains(&check_ssid) {
                        list.push(Box::new(ssid).to_access_point());
                        dedup.push(check_ssid);
                    }
                }

                list.sort_by(|a, b| b.signal_strength.cmp(&a.signal_strength));
//                let ap_list = AccessPointList{ list: list };
//                info!("ap_list {:#?}",ap_list);
                let access_point_list = AccessPointList { list: list };
                info!("ap_list {:#?}",access_point_list);
                // Send the access point list to the main thread
                tx.send(access_point_list).await.unwrap();
            }
        }).await;

/*
    // Receive and print the access point list sent from the Tokio task
    if let Some(access_point_list) = rx.recv().await{
        info!("outside thread");
        info!("{:?}", access_point_list);        
    };
*/
/*
    {
        for access_point in access_point_list.list {
            info!("outside thread");
            info!("{:?}", access_point);
        }
    }
*/
    // Receive the access point list sent from the Tokio task
    let access_point_list = rx.recv().await.unwrap();
    info!("outside thread");
    info!("{:?}", access_point_list);

    access_point_list

//         let ap_resp = AccessPointList {
//             list: vec![
//                      AccessPoint { ssid: (String::from("Test-1")), security_mode: (ripple_sdk::api::device::device_wifi::WifiSecurityMode::None), signal_strength: (0), frequency: (0.0)},
//                      AccessPoint { ssid: (String::from("Test-2")), security_mode: (ripple_sdk::api::device::device_wifi::WifiSecurityMode::None), signal_strength: (0), frequency: (0.0)}
//                  ] };            
//     ExtnResponse::WifiScanListResponse(ap_resp.clone());

// ap_resp
    
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
