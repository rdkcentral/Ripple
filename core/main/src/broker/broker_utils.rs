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
        BrokerCallback, BrokerCleaner, BrokerConnectRequest, BrokerRequest, BrokerSender,
        BrokerSubMap, EndpointBroker,
    },
    thunder_broker::ThunderBroker,
};

use crate::state::platform_state::PlatformState;
use crate::tokio::sync::Mutex;
use futures::stream::{SplitSink, SplitStream};
use futures::SinkExt;
use futures_util::StreamExt;
use jsonrpsee::core::RpcResult;
use ripple_sdk::{
    api::gateway::rpc_gateway_api::{CallContext, JsonRpcApiError, RpcRequest},
    log::{error, info, trace},
    tokio::{self, net::TcpStream},
    utils::rpc_utils::extract_tcp_port,
};

use serde_json::json;
use serde_json::Value;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;

use ripple_sdk::tokio::sync::mpsc;

use tokio_tungstenite::{client_async, tungstenite::Message, WebSocketStream};

// use thunder_ripple_sdk::client::{
//     device_operator::{DeviceCallRequest, DeviceChannelParams, DeviceOperator},
//     thunder_plugin::ThunderPlugin,
// };

fn get_callsign_and_method_from_alias(alias: &str) -> (String, Option<String>) {
    let mut collection: Vec<&str> = alias.split('.').collect();
    let method = collection.pop();

    // Check if the second-to-last element is a digit (version number)
    if let Some(&version) = collection.last() {
        if version.chars().all(char::is_numeric) {
            collection.pop(); // Remove the version number
        }
    }

    let callsign = collection.join(".");
    (callsign, method.map(|m| m.to_string()))
}

pub struct BrokerUtils;
impl BrokerUtils {
    pub async fn get_ws_broker(
        endpoint: &str,
        alias: Option<String>,
    ) -> (
        SplitSink<WebSocketStream<TcpStream>, Message>,
        SplitStream<WebSocketStream<TcpStream>>,
    ) {
        info!("Broker Endpoint url {}", endpoint);
        let url_path = if let Some(a) = alias {
            format!("{}{}", endpoint, a)
        } else {
            endpoint.to_owned()
        };
        let url = url::Url::parse(&url_path).unwrap();
        let port = extract_tcp_port(endpoint);
        let tcp_port = port.unwrap();

        info!("Url host str {}", url.host_str().unwrap());
        let mut index = 0;

        loop {
            // Try connecting to the tcp port first
            if let Ok(v) = TcpStream::connect(&tcp_port).await {
                // Setup handshake for websocket with the tcp port
                // Some WS servers lock on to the Port but not setup handshake till they are fully setup
                if let Ok((stream, _)) = client_async(url_path.clone(), v).await {
                    break stream.split();
                }
            }
            if (index % 10).eq(&0) {
                error!(
                    "Broker with {} failed with retry for last {} secs in {}",
                    url_path, index, tcp_port
                );
            }
            index += 1;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    pub async fn process_for_app_main_request(
        state: &mut PlatformState,
        method: &str,
        params: Option<Value>,
        app_id: &str,
    ) -> RpcResult<Value> {
        let mut rpc_request = RpcRequest::internal(method, None).with_params(params);
        rpc_request.ctx.app_id = app_id.to_owned();
        Self::internal_request(state, rpc_request).await
    }

    pub async fn process_internal_main_request<'a>(
        state: &mut PlatformState,
        method: &'a str,
        params: Option<Value>,
    ) -> RpcResult<Value> {
        Self::process_internal_request(state, None, method, params).await
    }

    pub async fn process_internal_request<'a>(
        state: &mut PlatformState,
        on_behalf_of: Option<CallContext>,
        method: &'a str,
        params: Option<Value>,
    ) -> RpcResult<Value> {
        let rpc_request = RpcRequest::internal(method, on_behalf_of).with_params(params);
        state
            .metrics
            .add_api_stats(&rpc_request.ctx.request_id, method);
        Self::internal_request(state, rpc_request).await
    }

    async fn internal_request(
        state: &mut PlatformState,
        rpc_request: RpcRequest,
    ) -> RpcResult<Value> {
        let method = rpc_request.method.clone();
        match state.internal_rpc_request(&rpc_request).await {
            Ok(res) => match res.as_value() {
                Some(v) => Ok(v),
                None => Err(JsonRpcApiError::default()
                    .with_code(-32100)
                    .with_message(format!("failed to get {} : {:?}", method, res))
                    .into()),
            },
            Err(e) => Err(JsonRpcApiError::default()
                .with_code(-32100)
                .with_message(format!("failed to get {} : {}", method, e))
                .into()),
        }
    }

    // async fn register_custom_callback(
    //     broker: &ThunderBroker,
    //     request_id: u64,
    // ) -> tokio::sync::mpsc::Receiver<BrokerOutput> {
    //     let (response_tx, response_rx) = mpsc::channel(10);
    //     // Register custom callback to handle the response
    //     broker
    //         .register_custom_callback(
    //             request_id,
    //             BrokerCallback {
    //                 sender: response_tx,
    //             },
    //         )
    //         .await;
    //     response_rx
    // }

    // fn get_next_id() -> u64 {
    //     ATOMIC_ID.fetch_add(1, Ordering::SeqCst)
    // }

    async fn send_thunder_request(
        &self,
        ws_tx: &Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>>,
        request: &str,
    ) -> Result<(), ()> {
        let mut ws_tx = ws_tx.lock().await;
        if let Err(e) = ws_tx.feed(Message::Text(request.to_string())).await {
            error!("Failed to send Thunder request: {}", e);
            return Err(());
        }
        if let Err(e) = ws_tx.flush().await {
            error!("Failed to flush Thunder request: {}", e);
            return Err(());
        }
        Ok(())
    }

    pub async fn handle_subscribe(
        broker_request: BrokerRequest,
        connection_req: BrokerConnectRequest,
        callback: BrokerCallback,
    ) {
        let mut requests = Vec::new();
        let endpoint = connection_req.endpoint.clone();
        let (response_tx, _response_rx) = mpsc::channel::<BrokerRequest>(10);
        let (c_tx, _c_tr) = mpsc::channel(2);

        let subscription_map: Arc<RwLock<BrokerSubMap>> =
            Arc::new(RwLock::new(connection_req.sub_map.clone()));

        let alias = broker_request.rule.alias.clone();
        let (callsign, method) = get_callsign_and_method_from_alias(&alias);
        let method = method.clone();

        let broker_sender = BrokerSender {
            sender: response_tx,
        };
        let cleaner = BrokerCleaner {
            cleaner: Some(c_tx.clone()),
        };

        let thunder_broker = ThunderBroker::new(broker_sender, subscription_map, cleaner, callback);

        tokio::spawn(async move {
            let (ws_tx, mut ws_rx) = BrokerUtils::get_ws_broker(&endpoint.get_url(), None).await;
            let id = broker_request.rpc.ctx.call_id;

            if broker_request.rpc.is_subscription() && !broker_request.rpc.is_unlisten() {
                let listen = broker_request.rpc.is_listening();
                if let Some(sub_req) = thunder_broker.subscribe(&broker_request) {
                    requests.push(
                        json!({
                            "jsonrpc": "2.0",
                            "id": sub_req.rpc.ctx.call_id,
                            "method": format!("{}.unregister", callsign),
                            "params": {
                                "event": method,
                                "id": format!("{}", sub_req.rpc.ctx.call_id)
                            }
                        })
                        .to_string(),
                    )
                }
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
            } else if broker_request.rpc.is_unlisten() {
                if let Some(cleanup) = thunder_broker.unsubscribe(&broker_request) {
                    trace!(
                        "Unregistering thunder listener for call_id {} and method {}",
                        cleanup.rpc.ctx.call_id,
                        method.clone().unwrap()
                    );
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
            } else {
                match ThunderBroker::update_request(&broker_request) {
                    Ok(request) => requests.push(request),
                    Err(e) => error!("Failed to update request: {}", e),
                }
            }

            let ws_tx = Arc::new(Mutex::new(ws_tx));

            for r in requests {
                if BrokerUtils.send_thunder_request(&ws_tx, &r).await.is_err() {
                    error!("Failed to send subscription request for {}", callsign);
                }
            }

            tokio::pin! {
                let read = ws_rx.next();
            }

            loop {
                tokio::select! {
                    Some(msg) = read.as_mut() => {
                        match msg {
                            Ok(Message::Text(ref text)) => {
                                trace!("Received message: {}", text);
                                let id = ThunderBroker::get_id_from_result(text.as_bytes());
                                        let composite_resp_params = ThunderBroker::get_composite_response_params_by_id(thunder_broker.clone(), id).await;
                                        if let Ok(Message::Text(t)) = msg {
                                            let _ = ThunderBroker::handle_jsonrpc_response(t.clone().as_bytes(), thunder_broker.clone().get_broker_callback(id).await, composite_resp_params);
                                        }
                            }
                            Ok(Message::Close(_)) => {
                                info!("WebSocket connection closed");
                                break;
                            }
                            Err(e) => {
                                error!("WebSocket error: {}", e);
                                break;
                            }
                            _ => {}
                        }
                    }
                    else => {
                        trace!("No more messages from WebSocket");
                        break;
                    }
                }
            }
        });
    }
}
