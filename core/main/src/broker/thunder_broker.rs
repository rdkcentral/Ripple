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
use super::{
    endpoint_broker::{BrokerCallback, BrokerOutput, BrokerRequest, BrokerSender, EndpointBroker},
    rules_engine::RuleEndpoint,
    thunder::thunder_plugins_status_mgr::StatusManager,
};
use crate::utils::rpc_utils::extract_tcp_port;
use futures_util::{SinkExt, StreamExt};
use ripple_sdk::{
    api::gateway::rpc_gateway_api::JsonRpcApiResponse,
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

#[derive(Debug, Clone)]
pub struct ThunderBroker {
    sender: BrokerSender,
    subscription_map: Arc<RwLock<HashMap<String, BrokerRequest>>>,
    status_manager: StatusManager,
}

impl ThunderBroker {
    fn start(endpoint: RuleEndpoint, callback: BrokerCallback) -> Self {
        let (tx, mut tr) = mpsc::channel(10);
        let sender = BrokerSender { sender: tx };
        let subscription_map = Arc::new(RwLock::new(HashMap::new()));
        let broker = Self {
            sender,
            subscription_map,
            status_manager: StatusManager::new(),
        };
        let broker_c = broker.clone();
        let callback_for_sender = callback.clone();
        tokio::spawn(async move {
            info!("Broker Endpoint url {}", endpoint.url);
            let url = url::Url::parse(&endpoint.url).unwrap();
            let port = extract_tcp_port(&endpoint.url);
            info!("Url host str {}", url.host_str().unwrap());
            let mut index = 0;
            //let tcp_url = url.host_str()
            let tcp = loop {
                if let Ok(v) = TcpStream::connect(&port).await {
                    break v;
                } else {
                    if (index % 10).eq(&0) {
                        error!(
                            "Thunder Broker failed with retry for last {} secs in {}",
                            index, port
                        );
                    }
                    index += 1;
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            };

            let (stream, _) = client_async(url, tcp).await.unwrap();
            let (mut ws_tx, mut ws_rx) = stream.split();

            // send the first request to the broker. This is the controller statechange subscription request
            let request = broker_c
                .status_manager
                .generate_state_change_subscribe_request();

            let _feed = ws_tx
                .feed(tokio_tungstenite::tungstenite::Message::Text(
                    request.to_string(),
                ))
                .await;
            let _flush = ws_tx.flush().await;

            tokio::pin! {
                let read = ws_rx.next();
            }
            loop {
                tokio::select! {
                    Some(value) = &mut read => {
                        match value {
                            Ok(v) => {
                                if let tokio_tungstenite::tungstenite::Message::Text(t) = v {
                                    if broker_c.status_manager.is_controller_response(broker_c.get_sender(), callback.clone(), t.as_bytes()).await {
                                        broker_c.status_manager.handle_controller_response(broker_c.get_sender(), callback.clone(), t.as_bytes()).await;
                                    }
                                    else {
                                        // send the incoming text without context back to the sender
                                        Self::handle_response(t.as_bytes(),callback.clone())
                                    }
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
                        match broker_c.prepare_request(&request) {
                            Ok(updated_request) => {
                                debug!("Sending request to broker {:?}", updated_request);
                                for r in updated_request {
                                    let _feed = ws_tx.feed(tokio_tungstenite::tungstenite::Message::Text(r)).await;
                                let _flush = ws_tx.flush().await;
                                }
                            }
                            Err(e) => {
                                match e {
                                    RippleError::ServiceNotReady => {
                                        info!("Thunder Service not ready, adding request to pending {:?}", request);
                                    },
                                    _ =>
                                callback_for_sender.send_error(request,e).await
                            }
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

    fn get_callsign_and_method_from_alias(alias: &str) -> (String, Option<&str>) {
        let mut collection: Vec<&str> = alias.split('.').collect();
        let method = collection.pop();
        let callsign = collection.join(".");
        (callsign, method)
    }
}

impl EndpointBroker for ThunderBroker {
    fn get_broker(endpoint: RuleEndpoint, callback: BrokerCallback) -> Self {
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
        let rpc = rpc_request.clone().rpc;
        let id = rpc.ctx.call_id;
        let app_id = rpc.ctx.app_id;
        let (callsign, method) = Self::get_callsign_and_method_from_alias(&rpc_request.rule.alias);
        if method.is_none() {
            return Err(RippleError::InvalidInput);
        }
        // TBD: Move these changes to a separate method
        // check if the plugin is activated.
        let status = match self.status_manager.get_status(callsign.clone()) {
            Some(v) => v.clone(),
            None => {
                self.status_manager
                    .add_request_to_pending(callsign.clone(), rpc_request.clone());
                // PluginState is not available with StateManager,  create an internal thunder request to activate the plugin
                let request = self
                    .status_manager
                    .generate_plugin_status_request(callsign.clone());
                requests.push(request.to_string());
                return Ok(requests);
            }
        };

        if status.state.is_missing() {
            /*self.status_manager
            .add_request_to_pending(callsign.clone(), rpc_request.clone());*/
            error!("Plugin {} is missing", callsign);
            return Err(RippleError::ServiceError);
        }

        if status.state.is_activating() {
            info!("Plugin {} is activating", callsign);
            return Err(RippleError::ServiceNotReady);
        }

        if !status.state.is_activated() {
            // add the broker request to pending list
            self.status_manager
                .add_request_to_pending(callsign.clone(), rpc_request.clone());
            // create an internal thunder request to activate the plugin
            let request = self
                .status_manager
                .generate_plugin_activation_request(callsign.clone());
            requests.push(request.to_string());
            return Ok(requests);
        }

        let method = method.unwrap();
        if rpc_request.rpc.is_subscription() {
            let listen = rpc_request.rpc.is_listening();
            let notif_id = {
                let mut sub_map = self.subscription_map.write().unwrap();

                if listen {
                    if let Some(cleanup) = sub_map.insert(app_id, rpc_request.clone()) {
                        requests.push(
                            json!({
                                "jsonrpc": "2.0",
                                "id": cleanup.rpc.ctx.call_id,
                                "method": format!("{}.{}", callsign, "unregister".to_owned()),
                                "params": {
                                    "event": method,
                                    "id": format!("{}", cleanup.rpc.ctx.call_id)
                                }
                            })
                            .to_string(),
                        )
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
                    "method": format!("{}.{}", callsign, match listen {
                        true => "register",
                        false => "unregister"
                    }),
                    "params": json!({
                        "event": method,
                        "id": format!("{}", notif_id)
                    })
                })
                .to_string(),
            )
        } else {
            let request = Self::update_request(rpc_request)?;
            requests.push(request)
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
