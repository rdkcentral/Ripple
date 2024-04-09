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
    BrokerCallback, BrokerOutput, BrokerRequest, BrokerSender, EndpointBroker,
};
use futures_util::{SinkExt, StreamExt};
use ripple_sdk::{
    api::{
        gateway::rpc_gateway_api::JsonRpcApiResponse, manifest::extn_manifest::PassthroughEndpoint,
        session::AccountSession,
    },
    log::{debug, error, info},
    tokio::{self, net::TcpStream, sync::mpsc},
    utils::error::RippleError,
};
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};
use tokio_tungstenite::client_async;

fn extract_tcp_port(url: &str) -> String {
    let url_split: Vec<&str> = url.split("://").collect();
    if let Some(domain) = url_split.get(1) {
        let domain_split: Vec<&str> = domain.split('/').collect();
        domain_split.first().unwrap().to_string()
    } else {
        url.to_owned()
    }
}

#[derive(Debug, Clone)]
pub struct ThunderBroker {
    sender: BrokerSender,
    subscription_map: Arc<RwLock<HashMap<String, BrokerRequest>>>,
}

impl ThunderBroker {
    fn start(endpoint: PassthroughEndpoint, callback: BrokerCallback) -> Self {
        let (tx, mut tr) = mpsc::channel(10);
        let sender = BrokerSender { sender: tx };
        let subscription_map = Arc::new(RwLock::new(HashMap::new()));
        let broker = Self {
            sender,
            subscription_map,
        };
        let broker_c = broker.clone();
        tokio::spawn(async move {
            info!("Broker Endpoint url {}", endpoint.url);
            let url = url::Url::parse(&endpoint.url).unwrap();
            let port = extract_tcp_port(&endpoint.url);
            info!("Url host str {}", url.host_str().unwrap());
            //let tcp_url = url.host_str()
            let tcp = loop {
                if let Ok(v) = TcpStream::connect(&port).await {
                    break v;
                } else {
                    error!("Broker Wait for a sec and retry {}", port);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            };

            let (stream, _) = client_async(url, tcp).await.unwrap();
            let (mut ws_tx, mut ws_rx) = stream.split();

            tokio::pin! {
                let read = ws_rx.next();
            }
            loop {
                tokio::select! {
                    Some(value) = &mut read => {
                        match value {
                            Ok(v) => {
                                if let tokio_tungstenite::tungstenite::Message::Text(t) = v {
                                    // send the incoming text without context back to the sender
                                    Self::handle_response(t.as_bytes(),callback.clone())
                                }
                            },
                            Err(e) => {
                                error!("Broker Websocket error on read {:?}", e);
                                break false
                            }
                        }

                    },
                    Some(request) = tr.recv() => {
                        debug!("Got request from receiver for broker {:?}", request);
                        if let Ok(updated_request) = broker_c.prepare_request(&request) {
                            debug!("Sending request to broker {:?}", updated_request);
                            for r in updated_request {
                                let _feed = ws_tx.feed(tokio_tungstenite::tungstenite::Message::Text(r)).await;
                            let _flush = ws_tx.flush().await;
                            }

                        }

                    }
                }
            }
        });
        broker
    }

    fn update_response(response: &JsonRpcApiResponse) -> JsonRpcApiResponse {
        let mut new_response = response.clone();
        if response.params.is_some() {
            new_response.result = response.params.clone();
        }
        new_response
    }
}

impl EndpointBroker for ThunderBroker {
    fn get_broker(
        _: Option<AccountSession>,
        endpoint: PassthroughEndpoint,
        callback: BrokerCallback,
    ) -> Self {
        Self::start(endpoint, callback)
    }

    fn get_sender(&self) -> BrokerSender {
        self.sender.clone()
    }

    fn prepare_request(
        &self,
        rpc_request: &super::endpoint_broker::BrokerRequest,
    ) -> Result<Vec<String>, RippleError> {
        let mut requests = Vec::new();
        if let Some(transformer) = &rpc_request.transformer {
            let rpc = rpc_request.clone().rpc;
            let id = rpc.ctx.call_id;
            let app_id = rpc.ctx.app_id;
            if rpc_request.rpc.is_subscription() {
                let listen = rpc_request.rpc.is_listening();
                let notif_id = {
                    let mut sub_map = self.subscription_map.write().unwrap();

                    if listen {
                        if let Some(cleanup) = sub_map.insert(app_id, rpc_request.clone()) {
                            requests.push(json!({
                                "jsonrpc": "2.0",
                                "id": cleanup.rpc.ctx.call_id,
                                "method": format!("{}.{}", transformer.module, "unregister".to_owned()),
                                "params": {
                                    "event": transformer.method.clone(),
                                    "id": format!("{}", cleanup.rpc.ctx.call_id)
                                }
                            }).to_string())
                        }
                        id
                    } else if let Some(v) = sub_map.remove(&app_id) {
                        v.rpc.ctx.call_id
                    } else {
                        id
                    }
                };
                requests.push(
                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "method": format!("{}.{}", transformer.module, match listen {
                            true => "register",
                            false => "unregister"
                        }),
                        "params": json!({
                            "event": transformer.method.clone(),
                            "id": format!("{}", notif_id)
                        })
                    })
                    .to_string(),
                )
            } else {
                requests.push(
                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "method": format!("{}.{}", transformer.module, transformer.method),
                        "params": rpc_request.rpc.params_json
                    })
                    .to_string(),
                )
            }
        } else {
            let rpc = rpc_request.rpc.clone();
            if let Some(params) = rpc_request.rpc.get_params() {
                requests.push(
                    json!({
                        "jsonrpc": "2.0",
                        "id": rpc.ctx.call_id,
                        "method": rpc.method,
                        "params": params
                    })
                    .to_string(),
                )
            } else {
                requests.push(
                    json!({
                        "jsonrpc": "2.0",
                        "id": rpc.ctx.call_id,
                        "method": rpc.method,
                    })
                    .to_string(),
                )
            }
        }
        Ok(requests)
    }

    /// Default handler method for the broker to remove the context and send it back to the
    /// client for consumption
    fn handle_response(result: &[u8], callback: BrokerCallback) {
        let mut final_result = Err(RippleError::ParseError);
        if let Ok(data) = serde_json::from_slice::<JsonRpcApiResponse>(result) {
            let updated_data = Self::update_response(&data);
            final_result = Ok(BrokerOutput { data: updated_data });
        }
        if let Ok(output) = final_result {
            tokio::spawn(async move { callback.sender.send(output).await });
        } else {
            error!("Bad broker response {}", String::from_utf8_lossy(result));
        }
    }
}
