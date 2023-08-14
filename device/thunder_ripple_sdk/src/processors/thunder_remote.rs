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
    client::{
        thunder_client::ThunderClient,
        thunder_plugin::ThunderPlugin::{self, RemoteControl},
    },
    ripple_sdk::{
        api::{
            accessory::RemoteAccessoryResponse,
            device::{
                device_accessory::{
                    AccessoryDeviceListResponse, AccessoryDeviceResponse, AccessoryListRequest,
                    AccessoryPairRequest, AccessoryProtocol, AccessoryProtocolListType,
                    AccessoryType, RemoteAccessoryRequest,
                },
                device_operator::{
                    DeviceCallRequest, DeviceChannelParams, DeviceOperator, DeviceResponseMessage,
                    DeviceSubscribeRequest, DeviceUnsubscribeRequest,
                },
            },
        },
        async_trait::async_trait,
        extn::{
            client::{
                extn_client::ExtnClient,
                extn_processor::{
                    DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
                },
            },
            extn_client_message::{ExtnMessage, ExtnPayload, ExtnPayloadProvider, ExtnResponse},
        },
        log::{info, warn},
        serde_json::{self},
        tokio,
        tokio::sync::mpsc,
        utils::error::RippleError,
    },
    thunder_state::ThunderState,
};
use serde::{Deserialize, Serialize};
use tokio::time::{sleep, Duration};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteStatus {
    net_type: u32,
    pairing_state: String,
    remote_data: Vec<RemoteData>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteData {
    make: String,
    model: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemotePairRequest {
    net_type: u32,
    timeout: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteListRequest {
    net_type: u32,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_camel_case_types, non_snake_case)]
struct RemotePairError {
    code: u32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteStatusEvent {
    status: RemoteStatus,
}

impl RemoteStatusEvent {
    pub fn get_list_response(self: Box<Self>) -> AccessoryDeviceListResponse {
        let mut remote_list = Vec::new();
        let (new_self, opt_protocol) = self.get_protocol();
        let protocol = opt_protocol.unwrap();
        let remote_data_list = new_self.status.remote_data;
        for i in remote_data_list {
            remote_list.push(RemoteStatusEvent::get_accessory_device_response(
                i,
                protocol.clone(),
            ))
        }
        AccessoryDeviceListResponse { list: remote_list }
    }

    pub fn get_protocol(self: Box<Self>) -> (Box<Self>, Option<AccessoryProtocol>) {
        let status = self.status.net_type;
        (
            self,
            match status {
                0 => Some(AccessoryProtocol::RF4CE),
                1 => Some(AccessoryProtocol::BluetoothLE),
                _ => None,
            },
        )
    }

    pub fn get_accessory_device_response(
        remote_data: RemoteData,
        protocol: AccessoryProtocol,
    ) -> AccessoryDeviceResponse {
        AccessoryDeviceResponse {
            _type: AccessoryType::Remote,
            make: remote_data.make,
            model: remote_data.model,
            protocol,
        }
    }
}

pub struct ThunderRemoteAccessoryRequestProcessor {
    state: ThunderState,
    streamer: DefaultExtnStreamer,
}

impl ThunderRemoteAccessoryRequestProcessor {
    pub fn new(state: ThunderState) -> ThunderRemoteAccessoryRequestProcessor {
        ThunderRemoteAccessoryRequestProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }

    async fn pair(
        state: ThunderState,
        pair_request: AccessoryPairRequest,
        req: ExtnMessage,
    ) -> bool {
        // let pair_request = pair_request_opt.unwrap();
        let pair_request = pair_request.clone();
        let protocol = pair_request.clone().protocol;
        let remote_pair_request = RemotePairRequest {
            net_type: ThunderRemoteAccessoryRequestProcessor::get_net_type(protocol.clone()),
            timeout: pair_request.timeout,
        };
        let request_method: String = ThunderPlugin::RemoteControl.method("startPairing");

        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: request_method,
                params: Some(DeviceChannelParams::Json(
                    ripple_sdk::serde_json::to_string(&remote_pair_request).unwrap(),
                )),
            })
            .await;
        let response = match response.message["success"].as_bool() {
            Some(_v) => {
                let result = ThunderRemoteAccessoryRequestProcessor::wait_for_remote_pair(
                    state.clone(),
                    pair_request,
                    req.clone(),
                )
                .await;
                info!("remote accessory pair result {:?}", result);
                result
            }
            None => RemoteAccessoryResponse::Error(RippleError::InvalidOutput),
        };

        Self::respond(
            state.get_client(),
            req,
            if let ExtnPayload::Response(r) = response.get_extn_payload() {
                r
            } else {
                ExtnResponse::Error(RippleError::ProcessorError)
            },
        )
        .await
        .is_ok()
    }

    async fn wait_for_remote_pair(
        state: ThunderState,
        pair_request_opt: AccessoryPairRequest,
        _req: ExtnMessage,
    ) -> RemoteAccessoryResponse {
        let (tx, mut rx) = mpsc::channel::<RemoteAccessoryResponse>(32);
        let client = state.get_thunder_client();
        let unsub_client = client.clone();
        let protocol = pair_request_opt.protocol;

        let (sub_tx, mut sub_rx) = mpsc::channel::<DeviceResponseMessage>(32);
        client
            .clone()
            .subscribe(
                DeviceSubscribeRequest {
                    module: RemoteControl.callsign_and_version(),
                    event_name: "onStatus".into(),
                    params: None,
                    sub_id: None,
                },
                sub_tx,
            )
            .await;
        info!("subscribed to remote onStatus events");

        let _handle = tokio::spawn(async move {
                let sleep = sleep(Duration::from_secs(pair_request_opt.timeout));
                tokio::pin!(sleep);
                loop {
                    tokio::select! {
                        Some(m) = sub_rx.recv() => {
                let remote_status_event: RemoteStatusEvent =
                        serde_json::from_value(m.message).unwrap();
                    let pairing_status = remote_status_event.status.pairing_state.clone();
                    match pairing_status.as_str() {
                        "CONFIGURATION_COMPLETE" => {
                            info!("successfully paired");
                            let success_accessory_response: AccessoryDeviceResponse =
                            if !remote_status_event.status.remote_data.is_empty() {
                                let remote = remote_status_event.status.remote_data.get(0).unwrap();

                                ThunderRemoteAccessoryRequestProcessor::get_accessory_response(
                                        protocol,
                                        remote.make.clone(),
                                        remote.model.clone(),
                                    )
                            } else {
                                warn!("No Remote info");

                                ThunderRemoteAccessoryRequestProcessor::get_accessory_response(
                                        protocol,
                                        "".into(),
                                        "".into(),
                                    )
                            };
                            info!("{:?}", success_accessory_response);
                            tx.send(RemoteAccessoryResponse::AccessoryPairResponse(success_accessory_response)).await.unwrap();
                            ThunderRemoteAccessoryRequestProcessor::unsubscribe_pairing(unsub_client).await;
                            break;
                        }
                        "FAILED" => {
                            tx.send(RemoteAccessoryResponse::Error(RippleError::InvalidOutput)).await.unwrap();
                            ThunderRemoteAccessoryRequestProcessor::unsubscribe_pairing(unsub_client).await;
                            break;
                        }
                        _ => {
                            info!("Remote Pairing Status {}", pairing_status)
                        }
                    }
                }
                () = &mut sleep => {
                    let error_string = RemoteAccessoryResponse::String("Timed out while waiting for response".into());
                    tx.send(error_string).await.unwrap();
                    break;
                },
            }
        }
        })
        .await;

        rx.recv().await.unwrap()
    }

    pub async fn list(
        state: ThunderState,
        list_request_opt: Option<AccessoryListRequest>,
        req: ExtnMessage,
    ) -> bool {
        let list_request = list_request_opt.unwrap_or_default();
        let protocol = list_request
            .protocol
            .unwrap_or(AccessoryProtocolListType::All);
        let remote_pair_request = RemoteListRequest {
            net_type: ThunderRemoteAccessoryRequestProcessor::get_net_type_list(protocol.clone()),
        };
        let request_method = RemoteControl.method("getNetStatus");

        let list_response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: request_method,
                params: Some(DeviceChannelParams::Json(
                    serde_json::to_string(&remote_pair_request).unwrap(),
                )),
            })
            .await;
        info!("{}", list_response.message);
        let remote_status_event: RemoteStatusEvent =
            serde_json::from_value(list_response.message).unwrap();
        let remote_event = Box::new(remote_status_event);
        let success_response =
            RemoteAccessoryResponse::RemoteAccessoryListResponse(remote_event.get_list_response());

        Self::respond(
            state.get_client(),
            req,
            if let ExtnPayload::Response(r) = success_response.get_extn_payload() {
                r
            } else {
                ExtnResponse::Error(RippleError::ProcessorError)
            },
        )
        .await
        .is_ok()
    }

    pub fn get_net_type(protocol: AccessoryProtocol) -> u32 {
        match protocol {
            AccessoryProtocol::BluetoothLE => 1,
            AccessoryProtocol::RF4CE => 0,
        }
    }

    pub fn get_net_type_list(protocol: AccessoryProtocolListType) -> u32 {
        match protocol {
            AccessoryProtocolListType::BluetoothLE => 1,
            AccessoryProtocolListType::RF4CE | AccessoryProtocolListType::All => 0,
        }
    }

    pub fn get_accessory_response(
        protocol: AccessoryProtocol,
        make: String,
        model: String,
    ) -> AccessoryDeviceResponse {
        AccessoryDeviceResponse {
            _type: AccessoryType::Remote,
            make,
            model,
            protocol,
        }
    }

    pub async fn unsubscribe_pairing(client: ThunderClient) {
        // unsubscribing pairing
        client
            .unsubscribe(DeviceUnsubscribeRequest {
                module: RemoteControl.callsign_and_version(),
                event_name: "onStatus".into(),
            })
            .await;
    }
}

impl ExtnStreamProcessor for ThunderRemoteAccessoryRequestProcessor {
    type STATE = ThunderState;
    type VALUE = RemoteAccessoryRequest;

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
impl ExtnRequestProcessor for ThunderRemoteAccessoryRequestProcessor {
    fn get_client(&self) -> ExtnClient {
        self.state.get_client()
    }

    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        match extracted_message {
            RemoteAccessoryRequest::Pair(pair_params) => {
                Self::pair(state.clone(), pair_params, msg).await
            }
            RemoteAccessoryRequest::List(list_params) => {
                Self::list(state.clone(), Some(list_params), msg).await
            }
        }
    }
}
