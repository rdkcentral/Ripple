// Copyright 2023 Comcast Cable Communications Management, LLC
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
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::{
    client::thunder_plugin::ThunderPlugin,
    ripple_sdk::{
        api::{
            device::{
                device_operator::{
                    DeviceCallRequest, DeviceChannelParams, DeviceOperator, DeviceResponseMessage,
                    DeviceSubscribeRequest,
                },
                device_wifi::{AccessPoint, AccessPointList, AccessPointRequest, WifiSecurityMode},
            },
            wifi::WifiResponse,
        },
        async_trait::async_trait,
        extn::{
            client::extn_client::ExtnClient,
            client::extn_processor::{
                DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
            },
            extn_client_message::{ExtnMessage, ExtnResponse},
        },
        log::{debug, info},
        serde_json, tokio,
        tokio::sync::mpsc,
    },
    thunder_state::ThunderState,
};
use crate::{
    client::thunder_plugin::ThunderPlugin::Wifi,
    ripple_sdk::{
        self,
        api::device::{device_operator::DeviceUnsubscribeRequest, device_wifi::WifiRequest},
        extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider},
    },
};
use serde::{Deserialize, Serialize};
use tokio::time::{self, timeout, Duration};

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
        _ => WifiSecurityMode::None,
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConnectedSSIDResult {
    ssid: String,
    security: String,
    signal_strength: String,
    frequency: String,
}

impl ConnectedSSIDResult {
    fn to_access_point(&self) -> AccessPoint {
        AccessPoint {
            ssid: self.ssid.clone(),
            security_mode: wifi_security_mode_from_u32(
                self.security.parse::<u32>().unwrap_or_default(),
            ),
            signal_strength: self.signal_strength.parse::<i32>().unwrap_or_default(),
            frequency: self.frequency.parse::<f32>().unwrap_or_default(),
        }
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
    #[serde(default)]
    more_data: bool,
    ssids: Vec<ThunderSSID>,
}

impl ThunderSSID {
    fn to_access_point(&self) -> AccessPoint {
        AccessPoint {
            ssid: self.ssid.clone(),
            security_mode: wifi_security_mode_from_u32(self.security),
            signal_strength: self.signal_strength,
            frequency: self.frequency,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ThunderWifiConnectRequest {
    pub ssid: String,
    pub passphrase: String,
    pub security_mode: u32,
}

impl ThunderWifiConnectRequest {
    fn from_access_point_request(access_point_request: AccessPointRequest) -> Self {
        Self {
            ssid: access_point_request.ssid.clone(),
            passphrase: access_point_request.passphrase.clone(),
            security_mode: wifi_security_mode_to_u32(access_point_request.security),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_camel_case_types, non_snake_case)]
struct WifiStateChanged {
    state: u32,
    isLNF: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_camel_case_types, non_snake_case)]
struct WifiConnectError {
    code: u32,
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

    ///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
    /// WIFI SCAN ///
    ////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

    async fn scan(state: ThunderState, req: ExtnMessage, timeout: u64) -> bool {
        let start_scan: String = ThunderPlugin::Wifi.method("startScan");
        let request: ThunderWifiScanRequest = ThunderWifiScanRequest { incremental: false };
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: start_scan,
                params: Some(DeviceChannelParams::Json(
                    serde_json::to_string(&request).unwrap(),
                )),
            })
            .await;
        let response = match response.message["success"].as_bool() {
            Some(_v) => {
                let result_ssid = ThunderWifiRequestProcessor::wait_for_thunder_ssids(
                    state.clone(),
                    req.clone(),
                    timeout,
                )
                .await;
                info!("wifi scan result {:?}", result_ssid);
                WifiResponse::WifiScanListResponse(result_ssid)
            }
            None => WifiResponse::CustomError("UNKNOWN ERROR".into()),
        };

        Self::respond(
            state.get_client(),
            req,
            if let ExtnPayload::Response(r) = response.get_extn_payload() {
                r
            } else {
                ExtnResponse::Error(ripple_sdk::utils::error::RippleError::ProcessorError)
            },
        )
        .await
        .is_ok()
    }

    async fn wait_for_thunder_ssids(
        state: ThunderState,
        _req: ExtnMessage,
        timeout_value: u64,
    ) -> AccessPointList {
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
            // <pca>
            //.await;
            .await
            .ok();
        // </pca>
        // spawn a thread that handles all scan events, handle the success and error events
        tokio::spawn(async move {
            if let Ok(Some(m)) = timeout(Duration::from_secs(timeout_value), sub_rx.recv()).await {
                let mut list = Vec::new();
                let ssid_response: SSIDEventResponse = match serde_json::from_value(m.message) {
                    Ok(response) => response,
                    Err(e) => {
                        let access_point_list = AccessPointList { list: Vec::new() };
                        info!(
                            "Failed to parse SSIDEventResponse: {}. Access point list: {:#?}",
                            e, access_point_list
                        );
                        tx.send(access_point_list).await.unwrap();
                        return;
                    }
                };

                let mut dedup = Vec::new();
                for ssid in ssid_response.ssids {
                    let check_ssid = ssid.ssid.clone();
                    if !dedup.contains(&check_ssid) {
                        list.push(ssid.to_access_point());
                        dedup.push(check_ssid);
                    }
                }

                list.sort_by(|a, b| b.signal_strength.cmp(&a.signal_strength));
                let access_point_list = AccessPointList { list };
                info!("access point list {:#?}", access_point_list);
                // Send the access point list to the main thread
                tx.send(access_point_list).await.unwrap();
                info!("unsubscribing to wifi ssid scan thunder events");
            } else {
                let access_point_list = AccessPointList { list: Vec::new() };
                info!("access point list {:#?}", access_point_list);
                // Send the access point list to the main thread
                tx.send(access_point_list).await.unwrap();
            }

            unsub_client
                .unsubscribe(DeviceUnsubscribeRequest {
                    module: Wifi.callsign_and_version(),
                    event_name: "onAvailableSSIDs".into(),
                })
                .await;
        });

        // Receive the access point list sent from the Tokio task
        rx.recv().await.unwrap()
    }

    ///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
    /// WIFI CONNECT ///
    ////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

    async fn connect(
        state: ThunderState,
        req: ExtnMessage,
        access_point_request: AccessPointRequest,
    ) -> bool {
        info!("starting wifi connect");
        let start_scan: String = ThunderPlugin::Wifi.method("connect");
        let request = ThunderWifiConnectRequest::from_access_point_request(access_point_request);
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: start_scan,
                params: Some(DeviceChannelParams::Json(
                    serde_json::to_string(&request).unwrap(),
                )),
            })
            .await;
        let response = match response.message["success"].as_bool() {
            Some(_v) => {
                info!("wifi connect success");
                let result_ssid =
                    ThunderWifiRequestProcessor::wait_for_wifi_connect(state.clone(), req.clone())
                        .await;
                info!("wifi connect response :{:?}", result_ssid);
                result_ssid
            }
            None => WifiResponse::CustomError("UNKNOWN ERROR".into()),
        };

        Self::respond(
            state.get_client(),
            req,
            if let ExtnPayload::Response(r) = response.get_extn_payload() {
                r
            } else {
                ExtnResponse::Error(ripple_sdk::utils::error::RippleError::ProcessorError)
            },
        )
        .await
        .is_ok()
    }

    async fn wait_for_wifi_connect(state: ThunderState, _req: ExtnMessage) -> WifiResponse {
        let (tx, mut rx) = mpsc::channel::<WifiResponse>(32);
        let client = state.get_thunder_client();
        let unsub_client = client.clone();

        let (sub_tx, mut sub_rx) = mpsc::channel::<DeviceResponseMessage>(32);
        client
            .clone()
            .subscribe(
                DeviceSubscribeRequest {
                    module: Wifi.callsign_and_version(),
                    event_name: "onWIFIStateChanged".into(),
                    params: None,
                    sub_id: None,
                },
                sub_tx,
            )
            // <pca>
            //.await;
            .await
            .ok();
        // </pca>
        info!("subscribed to onWIFIStateChanged events");

        let (err_tx, mut err_rx) = mpsc::channel::<DeviceResponseMessage>(32);
        client
            .clone()
            .subscribe(
                DeviceSubscribeRequest {
                    module: Wifi.callsign_and_version(),
                    event_name: "onError".into(),
                    params: None,
                    sub_id: None,
                },
                err_tx,
            )
            // <pca>
            //.await;
            .await
            .ok();
        // </pca>
        info!("subscribed to wifi onError events");

        let _handle = tokio::spawn(async move {
            let sleep = time::sleep(Duration::from_secs(60));
            tokio::pin!(sleep);
    loop {
        tokio::select! {
            Some(m) = err_rx.recv() => {
                let error_code_response: WifiConnectError = serde_json::from_value(m.message).unwrap();
                debug!("{:?}",error_code_response);
                let error_string = match error_code_response.code {
                    0 => WifiResponse::CustomError("SSID_CHANGED".into()),
                    1 => WifiResponse::CustomError("CONNECTION_LOST".into()),
                    2 => WifiResponse::CustomError("CONNECTION_FAILED".into()),
                    3 => WifiResponse::CustomError("CONNECTION_INTERRUPTED".into()),
                    4 => WifiResponse::CustomError("INVALID_CREDENTIALS".into()),
                    5 => WifiResponse::CustomError("NO_SSID".into()),
                    _ => WifiResponse::CustomError("UNKNOWN ERROR".into()),
                };
                info!("error code response: {:?} ",error_string);
                tx.send(error_string).await.unwrap();
                break;
            }
            Some(m) = sub_rx.recv() => {
                let wifi_state_response: WifiStateChanged = serde_json::from_value(m.message).unwrap();
                info!("Wifi statechanged: {}", wifi_state_response.state);
                match wifi_state_response.state {
                    5 => {
                        let resp =
                            ThunderWifiRequestProcessor::get_connected_ssid(state.clone())
                                .await;
                        info!("{:?}", resp);
                        // Send the access point list to the main thread
                        tx.send(WifiResponse::WifiConnectSuccessResponse(resp)).await.unwrap();
                        break;
                    }
                    6 => {
                        let error_string = WifiResponse::CustomError("Unknown error...".into());
                        tx.send(error_string).await.unwrap();
                        break;
                    }
                    _ => {}
                }
            }
            () = &mut sleep => {
                let error_string = WifiResponse::CustomError("Timed out while waiting for response".into());
                tx.send(error_string).await.unwrap();
                break;
            },
        }
    }

    // unsubscribing wifi events
    unsub_client
        .unsubscribe(DeviceUnsubscribeRequest {
            module: Wifi.callsign_and_version(),
            event_name: "onWIFIStateChanged".into(),
        })
        .await;
    info!("Unsubscribing to onWIFIStateChanged events");

    unsub_client
    .unsubscribe(DeviceUnsubscribeRequest {
        module: Wifi.callsign_and_version(),
        event_name: "onError".into(),
    })
    .await;
info!("Unsubscribing to onError events");

})
.await;

        if let Some(msg) = rx.recv().await {
            match msg {
                WifiResponse::CustomError(s) => {
                    info!("Received string: {}", s);
                    WifiResponse::CustomError(s)
                    // handle string response here
                }
                WifiResponse::WifiConnectSuccessResponse(ap_list) => {
                    info!("Received access point list: {:?}", ap_list);
                    WifiResponse::WifiConnectSuccessResponse(ap_list)
                    // handle access point list response here
                }
                _ => {
                    info!("Received unknown message type");
                    // handle unknown message type here
                    WifiResponse::CustomError("out Received unknown message type".into())
                }
            }
        } else {
            WifiResponse::CustomError("Unknown Error".into())
        }
    }

    async fn get_connected_ssid(state: ThunderState) -> AccessPoint {
        let start_scan: String = ThunderPlugin::Wifi.method("getConnectedSSID");
        let request: ThunderWifiScanRequest = ThunderWifiScanRequest { incremental: false };
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: start_scan,
                params: Some(DeviceChannelParams::Json(
                    serde_json::to_string(&request).unwrap(),
                )),
            })
            .await;
        let get_connected_ssid_response: ConnectedSSIDResult =
            serde_json::from_value(response.message).unwrap();
        info!(
            "connected ssid response : {:?}",
            get_connected_ssid_response
        );

        get_connected_ssid_response.to_access_point()
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
            WifiRequest::Scan(timeout) => Self::scan(state.clone(), msg, timeout).await,
            WifiRequest::Connect(access_point) => {
                Self::connect(state.clone(), msg, access_point).await
            }
        }
    }
}
