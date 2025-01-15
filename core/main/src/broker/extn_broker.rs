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
use super::endpoint_broker::{
    BrokerCallback, BrokerCleaner, BrokerConnectRequest, BrokerOutput, BrokerRequest, BrokerSender,
    BrokerSubMap, EndpointBroker, EndpointBrokerState,
};
use crate::broker::broker_utils::BrokerUtils;
use crate::broker::workflow_broker::SubBrokerErr;
use crate::state::platform_state::PlatformState;
use ripple_sdk::api::gateway::rpc_gateway_api::JsonRpcApiError;

use ripple_sdk::async_channel::unbounded;
use ripple_sdk::extn::extn_client_message::{ExtnMessage, ExtnPayload, ExtnRequest};
use ripple_sdk::extn::ffi::ffi_message::CExtnMessage;
use ripple_sdk::framework::ripple_contract::RippleContract;
use ripple_sdk::{
    api::gateway::rpc_gateway_api::JsonRpcApiResponse,
    extn::client::extn_client::ExtnClient,
    extn::extn_id::{ExtnClassId, ExtnId, ExtnProviderRequest},
    log::{debug, error, info, trace},
    tokio::{
        self,
        sync::{mpsc, Mutex},
    },
    utils::error::RippleError,
};
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    vec,
};
use tokio_tungstenite::tungstenite::http::method;

#[derive(Clone)]
pub struct ExtnBroker {
    sender: BrokerSender,
}

impl ExtnBroker {
    pub fn start(
        ps: Option<PlatformState>,
        callback: BrokerCallback,
        endpoint_broker: EndpointBrokerState,
    ) -> BrokerSender {
        println!("**** extn_broker: start");
        let (tx, mut rx) = mpsc::channel::<BrokerRequest>(10);

        tokio::spawn(async move {
            println!("**** extn_broker: start: spawned");
            loop {
                match rx.recv().await {
                    Some(broker_request) => {
                        println!(
                            "**** extn_broker: start: Received message {:?}",
                            broker_request
                        );
                        trace!("Received message {:?}", broker_request);
                        let rpc_request = broker_request.rpc.clone();
                        println!("**** extn_broker: start: Received request: broker_request: rpc request: {:?}", rpc_request);
                        let mut extn_msg =
                            endpoint_broker.get_extn_message(rpc_request.ctx.call_id, true);
                        println!("**** extn_broker: start: Received request:broker_request: extn_request_map: extn_msg:{:?}", extn_msg);
                        let rule = broker_request.rule;
                        println!(
                            "**** extn_broker: start: Received request:broker_request: rule: {:?}",
                            rule
                        );

                        // id: ripple:channel:gateway:badger => comes from rule.endpoint
                        let alias = rule.alias;
                        println!("**** extn_broker: start: Received request:broker_request: alias:  {:?}", alias);

                        // fine tune: based on the endpoint, get the id
                        let id = if alias == "ripple:channel:gateway:badger" {
                            ExtnId::new_extn(ExtnClassId::Gateway, "badger".into())
                        } else {
                            ExtnId::new_extn(ExtnClassId::Gateway, alias.clone())
                        };

                        // method: badger.info => comes from rpc_request.method
                        let method = rpc_request.method.clone();
                        println!("**** extn_broker: start: Received request:broker_request: method:  {:?}", method);

                        println!("**** extn_broker: start: Received request:broker_request: rpc_request: {:?}", rpc_request);
                        let req = ExtnProviderRequest {
                            value: serde_json::to_value(rpc_request.clone()).unwrap(),
                            id: id.clone(),
                        };
                        println!("**** extn_broker: start: Received request:broker_request: extn_provider_request: {:?}", req);

                        let mut extn_client = if let Some(platform_state) = &ps {
                            platform_state.get_client().get_extn_client()
                        } else {
                            return;
                        };

                        //                         **** extn_sender: ExtnSender::send_request: msg: CExtnMessage { id: "4e98e642-7b55-4af2-be0e-c539f57709dc",
                        // requestor: "ripple:extn:jsonrpsee:badger", target: "\"device_info\"",
                        // target_id: "", payload: "{\"Request\":{\"Device\":{\"DeviceInfo\":\"GetTimezoneWithOffset\"}}}",
                        // callback: Some(Sender { .. }), ts: 1734392600618 }
                        // **** extn_client: initialize: c_message: CExtnMessage { id: "4e98e642-7b55-4af2-be0e-c539f57709dc", requestor: "ripple:extn:jsonrpsee:badger", target: "\"device_info\"", target_id: "", payload: "{\"Request\":{\"Device\":{\"DeviceInfo\":\"GetTimezoneWithOffset\"}}}", callback: Some(Sender { .. }), ts: 1734392600618 }
                        // **** extn_client: initialize: message: ExtnMessage { id: "4e98e642-7b55-4af2-be0e-c539f57709dc", requestor: ExtnId { _type: Extn, class: Jsonrpsee, service: "badger" }, target: DeviceInfo, target_id: None, payload: Request(Device(DeviceInfo(GetTimezoneWithOffset))), callback: Some(Sender { .. }), ts: Some(1734392600618) }

                        let s = extn_client.get_extn_sender_with_extn_id(&alias);
                        println!("**** extn_broker: start: sender from extn_client: {:?}", s);

                        // Remove *Revert back to the original -  no extn message becaue it's rpcrequest
                        // let extn_msg = endpoint_broker.get_extn_message(rpc_request.ctx.call_id, true);
                        // println!("**** extn_broker: start: Received request:broker_request: extn_request_map: extn_msg:{:?}", extn_msg);

                        if let Some(sender) = s {
                            println!("**** extn_broker: start: sender available");

                            // <pca> 2
                            let (callback_tx, callback_rx) = unbounded();
                            // </pca>

                            let msg = ExtnMessage {
                                id: rpc_request.ctx.call_id.to_string(),
                                requestor: ExtnId::get_main_target("main".into()),
                                target: RippleContract::JsonRpsee,
                                target_id: Some(id),
                                payload: ExtnPayload::Request(ExtnRequest::Rpc(
                                    rpc_request.clone(),
                                )),
                                // <pca> 2
                                //callback: None,
                                callback: Some(callback_tx),
                                // </pca>
                                ts: None,
                            };

                            //let result = extn_client.send_message(msg).await;

                            // tokio::spawn(async move {

                            // fix channel request: not working
                            let response = sender.try_send(msg.into());
                            println!("**** extn_broker: start: channel response ={:?}", response);
                            println!("*** _DEBUG: extn_broker: send response ={:?}", response);
                            let resp = callback_rx.recv().await;
                            println!("*** _DEBUG: extn_broker: callback resp={:?}", resp);
                            // });

                            // TBD - ExtnPayloadProvider ?
                            // let p = payload.get_extn_payload();
                            // let c_request = p.into();
                            // let msg = CExtnMessage {
                            //     requestor: self.id.to_string(),
                            //     callback,
                            //     payload: c_request,
                            //     id,
                            //     target: payload.get_contract().into(),
                            //     target_id: "".to_owned(),
                            //     ts: Utc::now().timestamp_millis(),
                            // };
                            // let msg = ExtnMessage {
                            //     callback: callback.clone(),
                            //     id: uuid::Uuid::new_v4().to_string(),
                            //     requestor: alias.clone(),
                            //     target:
                            //     payload,
                            //     target_id: "".to_owned(),
                            //     ts: Utc::now().timestamp_millis(),
                            // };
                            // if let Err(e) = sender.try_send(msg) {
                            //     println!("**** extn_broker: start: send() error for message in other sender {}", e);
                            //     error!("send() error for message in other sender {}", e);
                            //     return Err(RippleError::SendFailure);
                            // }
                        } else {
                            println!("**** extn_broker: start: No sender available");
                            error!("No sender available");
                        }

                        // if let Some(sender) = s {
                        //     trace!("Sending message on the other sender");
                        //     if let Err(e) = sender.try_send(msg) {
                        //         error!("send() error for message in other sender {}", e);
                        //         return Err(RippleError::SendFailure);
                        //     }
                        //     Ok(())
                        // }

                        // BrokerRequest { rpc: RpcRequest { method: "badger.info", params_json: "[{\"app_id\":\"comcast.test.firecert\",\"call_id\":1,\"cid\":\"98ddd60d-7fe5-4706-b427-7add3cda88c4\",\"gateway_secure\":false,\"method\":\"badger.info\",\"protocol\":\"JsonRpc\",\"request_id\":\"d75167d7-9108-4bac-83e0-0bc12598db03\",\"session_id\":\"b9a5ee49-bb32-44b5-b321-ab5596843f21\"},{}]",
                        // ctx: CallContext { session_id: "b9a5ee49-bb32-44b5-b321-ab5596843f21", request_id: "d75167d7-9108-4bac-83e0-0bc12598db03", app_id: "comcast.test.firecert",
                        // call_id: 1, protocol: JsonRpc, method: "badger.info", cid: Some("98ddd60d-7fe5-4706-b427-7add3cda88c4"),
                        // gateway_secure: false } }, rule: Rule { alias: "ripple:channel:gateway:badger",
                        // transform: RuleTransform { request: None, response: None, event: None, event_decorator_method: None },
                        // filter: None, event_handler: None, endpoint: Some("extn"), sources: None }, subscription_processed: None,
                        // workflow_callback: None }

                        // BrokerRequest { rpc: RpcRequest { method: "voiceguidance.enabled",
                        // params_json: "[{\"app_id\":\"epg\",\"call_id\":1000,\"cid\":\"14c15171-230c-4e3d-a54d-ff177ccf26de\",\"gateway_secure\":false,\"method\":\"accessibility.voiceGuidance\",\"protocol\":\"JsonRpc\",\"request_id\":\"61aedc0b-5bb1-44d0-8689-2cd3f43f6b09\",\"session_id\":\"ec01b1a6-e0d7-4ef2-9e6e-9d1daea56384\"},{}]",
                        // ctx: CallContext { session_id: "ec01b1a6-e0d7-4ef2-9e6e-9d1daea56384", request_id: "61aedc0b-5bb1-44d0-8689-2cd3f43f6b09",
                        // app_id: "epg", call_id: 8, protocol: JsonRpc, method: "accessibility.voiceGuidance", cid: Some("14c15171-230c-4e3d-a54d-ff177ccf26de"),
                        // gateway_secure: false } }, rule: Rule { alias: "org.rdk.TextToSpeech.isttsenabled",
                        // transform: RuleTransform { request: None, response: Some(".result.isenabled"), event: None,
                        // event_decorator_method: None }, filter: None, event_handler: None, endpoint: None, sources: None }, subscription_processed: None,
                        // workflow_callback: Some(BrokerCallback { sender: Sender { chan: Tx { inner: Chan { tx: Tx { block_tail: 0x7fc4b2893200, tail_position: 0 },
                        // semaphore: Semaphore { semaphore: Semaphore { permits: 10 }, bound: 10 }, rx_waker: AtomicWaker, tx_count: 2, rx_fields: "..." } } } }) }

                        // JsonRpcApiResponse { jsonrpc: "2.0", id: Some(9), result: Some(Object {"TTS_Status": Number(0), "isenabled": Bool(false), "success": Bool(true)}), error: None, method: None, params: None }

                        // let res: Result<JsonRpcApiResponse, SubBrokerErr> =
                        //     Ok(JsonRpcApiResponse {
                        //         id: Some(broker_request.rpc.ctx.call_id),
                        //         jsonrpc: "2.0".to_string(),
                        //         result: Some(json!({
                        //             "result": "yay reached extn: success"
                        //         })),
                        //         error: None,
                        //         method: Some(rpc_request.method.clone()),
                        //         params: Some(serde_json::Value::String(
                        //             rpc_request.params_json.clone(),
                        //         )),
                        //     });
                        // println!("**** extn_broker: start: res from extn_broker: {:?}", res);

                        // let error_response = Err(SubBrokerErr::JsonRpcApiError(
                        //     JsonRpcApiError::default()
                        //         .with_code(-32001)
                        //         .with_message(format!(
                        //             "extn_broker error for api {}",
                        //             broker_request.rpc.method
                        //         ))
                        //         .with_id(broker_request.rpc.ctx.call_id),
                        // ));

                        // match res {
                        //     Ok(yay) => {
                        //         println!(
                        //             "**** extn_broker: start: yay from extn_broker: {:?}",
                        //             yay
                        //         );
                        //         Self::send_broker_success_response(&callback, yay);
                        //     }
                        //     Err(err) => {
                        //         println!(
                        //             "**** extn_broker: start: error_response from extn_broker: {:?}",
                        //             err
                        //         );
                        //         Self::send_broker_failure_response(
                        //             &callback,
                        //             error_response.expect("REASON"),
                        //         );
                        //     }
                        // }
                    }
                    None => {
                        println!("**** extn_broker: start: Failed to receive message");
                        error!("Failed to receive message");
                        break;
                    }
                }
            }
        });
        BrokerSender { sender: tx }
    }
}

impl EndpointBroker for ExtnBroker {
    fn get_broker(
        ps: Option<PlatformState>,
        _request: BrokerConnectRequest,
        callback: BrokerCallback,
        broker_state: &mut EndpointBrokerState,
    ) -> Self {
        Self {
            sender: Self::start(ps, callback, broker_state.clone()),
        }
    }

    fn get_sender(&self) -> BrokerSender {
        self.sender.clone()
    }

    fn get_cleaner(&self) -> super::endpoint_broker::BrokerCleaner {
        BrokerCleaner::default()
    }
}
