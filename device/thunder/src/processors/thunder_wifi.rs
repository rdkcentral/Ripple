// If not stated otherwise in this file or this component's license file the
// following copyright and licenses apply:
//
// Copyright 2023 RDK Management
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use thunder_ripple_sdk::{client::thunder_plugin::ThunderPlugin::Wifi, ripple_sdk::{extn::{extn_client_message::{ExtnPayload, ExtnPayloadProvider}}, self, api::device::{device_wifi::WifiRequest, device_operator::DeviceUnsubscribeRequest}}};
use serde::{Serialize, Deserialize};
use serde_json::json;
use thunder_ripple_sdk::ripple_sdk::{
    api::{device::{
        device_info_request::DeviceInfoRequest,
        device_operator::{DeviceCallRequest, DeviceOperator, DeviceChannelParams, DeviceSubscribeRequest, DeviceResponseMessage}, device_wifi::{WifiSecurityMode, AccessPoint, AccessPointList},
    }, wifi::WifiResponse},
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::{ExtnMessage, ExtnResponse},
    },
    log::{error, info},
    serde_json,
    utils::error::RippleError, tokio,
};
use thunder_ripple_sdk::{
    client::thunder_plugin::ThunderPlugin,
    ripple_sdk::{extn::client::extn_client::ExtnClient, tokio::sync::mpsc},
    thunder_state::ThunderState,
};


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
        info!("fasil {}",response.message);
        let response = match response.message["success"].as_bool() {
            Some(v) => {
                info!("fasil inside function");
                let result_ssid =  ThunderWifiRequestProcessor::wait_for_thunder_ssids(
                    state.clone(),
                    req.clone(),
                ).await;
                info!("fasil result_ssid {:?}",result_ssid);

                WifiResponse::WifiScanListResponse(result_ssid)
            },
             None => WifiResponse::Error(RippleError::InvalidOutput),
         };

        info!("fasil {:?}",response);

        Self::respond(state.get_client(), 
        req,
        if let ExtnPayload::Response(r) = response.get_extn_payload() { r} 
                    else {
                            ExtnResponse::Error(ripple_sdk::utils::error::RippleError::ProcessorError)
                        }
                )
                .await
                .is_ok()            
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

    let Handle = tokio::spawn(async move {
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
        
            unsub_client
            .unsubscribe(DeviceUnsubscribeRequest { module: Wifi.callsign_and_version(), event_name: "onAvailableSSIDs".into() })
            .await;
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

    fn receiver(&mut self) -> mpsc::Receiver<ExtnMessage> {
        self.streamer.receiver()
    }

    fn sender(&self) -> mpsc::Sender<ExtnMessage> {
        self.streamer.sender()
    }
}

#[async_trait]
impl ExtnRequestProcessor for ThunderWifiRequestProcessor {
    fn get_client(&self) -> ExtnClient {
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