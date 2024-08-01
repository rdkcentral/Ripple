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
    endpoint_broker::{
        BrokerCallback, BrokerCleaner, BrokerConnectRequest, BrokerOutput, BrokerRequest,
        BrokerSender, BrokerSubMap, EndpointBroker,
    },
    thunder::thunder_plugins_status_mgr::StatusManager,
};
use crate::broker::broker_utils::BrokerUtils;
use futures_util::{SinkExt, StreamExt};

use ripple_sdk::{
    api::gateway::rpc_gateway_api::JsonRpcApiResponse,
    log::{debug, error, info},
    tokio::{self, sync::mpsc},
    utils::error::RippleError,
};
use serde_json::json;
use std::{
    sync::{Arc, RwLock},
    vec,
};

#[derive(Debug, Clone)]
pub struct ThunderBroker {
    sender: BrokerSender,
    subscription_map: Arc<RwLock<BrokerSubMap>>,
    cleaner: BrokerCleaner,
    status_manager: StatusManager,
}

impl ThunderBroker {
    fn start(request: BrokerConnectRequest, callback: BrokerCallback) -> Self {
        let endpoint = request.endpoint.clone();
        let (tx, mut tr) = mpsc::channel(10);
        let (c_tx, mut c_tr) = mpsc::channel(2);
        let sender = BrokerSender { sender: tx };
        let subscription_map = Arc::new(RwLock::new(request.sub_map.clone()));
        let broker = Self {
            sender,
            subscription_map,
            cleaner: BrokerCleaner {
                cleaner: Some(c_tx.clone()),
            },
            status_manager: StatusManager::new(),
        };
        let broker_c = broker.clone();
        let broker_for_cleanup = broker.clone();
        let callback_for_sender = callback.clone();
        let broker_for_reconnect = broker.clone();
        tokio::spawn(async move {
            let (mut ws_tx, mut ws_rx) =
                BrokerUtils::get_ws_broker(&endpoint.get_url(), None).await;

            // send the first request to the broker. This is the controller statechange subscription request
            let status_request = broker_c
                .status_manager
                .generate_state_change_subscribe_request();

            let _feed = ws_tx
                .feed(tokio_tungstenite::tungstenite::Message::Text(
                    status_request.to_string(),
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
                                        Self::handle_jsonrpc_response(t.as_bytes(),callback.clone())
                                    }
                                }
                            },
                            Err(e) => {
                                error!("Broker Websocket error on read {:?}", e);
                                // Time to reconnect Thunder with existing subscription
                                break;
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
                                        info!("Thunder Service not ready, request is now in pending list {:?}", request);
                                    },
                                    _ =>
                                callback_for_sender.send_error(request,e).await
                            }
                        }
                    }
                },
                    Some(cleanup_request) = c_tr.recv() => {
                        let value = {
                            broker_for_cleanup.subscription_map.write().unwrap().remove(&cleanup_request)
                        };
                        if let Some(mut cleanup) = value {
                            let sender = broker_for_cleanup.get_sender();
                            while let Some(mut v) = cleanup.pop() {
                                v.rpc = v.rpc.get_unsubscribe();
                                if (sender.send(v).await).is_err() {
                                    error!("Cleanup Error for {}",&cleanup_request);
                                }
                            }

                        }

                    }
                    }
            }

            let mut reconnect_request = request.clone();
            // Thunder Disconnected try reconnecting.
            {
                let mut subs = broker_for_reconnect.subscription_map.write().unwrap();
                for (k, v) in subs.drain().take(1) {
                    let _ = reconnect_request.sub_map.insert(k, v);
                }
            }
            if request.reconnector.send(reconnect_request).await.is_err() {
                error!("Error reconnecting to thunder");
            }
        });
        broker
    }

    fn update_response(response: &JsonRpcApiResponse) -> JsonRpcApiResponse {
        let mut new_response = response.clone();
        if response.params.is_some() {
            new_response.result = response.params.clone();
        } else if response.error.is_some() {
            new_response.result = response.error.clone();
        }
        new_response
    }

    fn get_callsign_and_method_from_alias(alias: &str) -> (String, Option<&str>) {
        let mut collection: Vec<&str> = alias.split('.').collect();
        let method = collection.pop();
        let callsign = collection.join(".");
        (callsign, method)
    }

    fn subscribe(&self, request: &BrokerRequest) -> Option<BrokerRequest> {
        let mut sub_map = self.subscription_map.write().unwrap();
        let app_id = &request.rpc.ctx.session_id;
        let method = &request.rpc.ctx.method;
        let listen = request.rpc.is_listening();
        let mut response = None;
        debug!(
            "Initial subscription map of {:?} app_id {:?}",
            sub_map, app_id
        );

        if let Some(mut v) = sub_map.remove(app_id) {
            debug!("Subscription map after removing app {:?}", v);
            if let Some(i) = v
                .iter()
                .position(|x| x.rpc.ctx.method.eq_ignore_ascii_case(method))
            {
                debug!(
                    "Removing subscription for method {} for app {}",
                    method, app_id
                );
                response = Some(v.remove(i));
            }
            if listen {
                v.push(request.clone());
            }
            let _ = sub_map.insert(app_id.clone(), v);
        } else {
            let _ = sub_map.insert(app_id.clone(), vec![request.clone()]);
        }
        response
    }
}

impl EndpointBroker for ThunderBroker {
    fn get_broker(request: BrokerConnectRequest, callback: BrokerCallback) -> Self {
        Self::start(request, callback)
    }

    fn get_sender(&self) -> BrokerSender {
        self.sender.clone()
    }

    fn get_cleaner(&self) -> BrokerCleaner {
        self.cleaner.clone()
    }

    fn prepare_request(
        &self,
        rpc_request: &super::endpoint_broker::BrokerRequest,
    ) -> Result<Vec<String>, RippleError> {
        let mut requests = Vec::new();
        let rpc = rpc_request.clone().rpc;
        let id = rpc.ctx.call_id;
        let (callsign, method) = Self::get_callsign_and_method_from_alias(&rpc_request.rule.alias);
        if method.is_none() {
            return Err(RippleError::InvalidInput);
        }
        // check if the plugin is activated.
        let status = match self.status_manager.get_status(callsign.clone()) {
            Some(v) => v.clone(),
            None => {
                self.status_manager
                    .add_broker_request_to_pending_list(callsign.clone(), rpc_request.clone());
                // PluginState is not available with StateManager,  create an internal thunder request to activate the plugin
                let request = self
                    .status_manager
                    .generate_plugin_status_request(callsign.clone());
                requests.push(request.to_string());
                return Ok(requests);
            }
        };

        if status.state.is_missing() {
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
                .add_broker_request_to_pending_list(callsign.clone(), rpc_request.clone());
            // create an internal thunder request to activate the plugin
            let request = self
                .status_manager
                .generate_plugin_activation_request(callsign.clone());
            requests.push(request.to_string());
            return Ok(requests);
        }

        let method = method.unwrap();
        // Below chunk of code is basically for subscription where thunder needs some special care based on
        // the JsonRpc specification
        if rpc_request.rpc.is_subscription() {
            let listen = rpc_request.rpc.is_listening();
            // If there was an existing app and method combo for the same subscription just unregister that
            if let Some(cleanup) = self.subscribe(rpc_request) {
                requests.push(
                    json!({
                        "jsonrpc": "2.0",
                        "id": cleanup.rpc.ctx.call_id,
                        "method": format!("{}.unregister", callsign),
                        "params": {
                            "event": method,
                            "id": format!("{}", cleanup.rpc.ctx.call_id)
                        }
                    })
                    .to_string(),
                )
            }

            // Given unregistration is already performed by previous step just do registration
            if listen {
                requests.push(
                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "method": format!("{}.register", callsign),
                        "params": json!({
                            "event": method,
                            "id": format!("{}", id)
                        })
                    })
                    .to_string(),
                )
            }
        } else {
            // Simple request and response handling
            let request = Self::update_request(rpc_request)?;
            requests.push(request)
        }

        Ok(requests)
    }

    /// Default handler method for the broker to remove the context and send it back to the
    /// client for consumption
    fn handle_jsonrpc_response(result: &[u8], callback: BrokerCallback) {
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
