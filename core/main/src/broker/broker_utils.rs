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
    endpoint_broker::{BrokerCallback, BrokerOutput, ATOMIC_ID},
    thunder_broker::ThunderBroker,
};
use crate::state::platform_state::PlatformState;
use crate::tokio::sync::mpsc;
use crate::tokio::sync::Mutex;
use futures::stream::{SplitSink, SplitStream};
use futures::SinkExt;
use futures_util::StreamExt;
use jsonrpsee::core::RpcResult;
use ripple_sdk::{
    api::gateway::rpc_gateway_api::{CallContext, JsonRpcApiError, RpcRequest},
    log::{error, info},
    tokio::{self, net::TcpStream},
    utils::rpc_utils::extract_tcp_port,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};
use tokio_tungstenite::{client_async, tungstenite::Message, WebSocketStream};

#[derive(Default)]
pub struct RippleMainSubscription {
    pub subscription_map: HashMap<u64, String>,
}

fn get_next_id() -> u64 {
    ATOMIC_ID.fetch_add(1, Ordering::SeqCst)
}

async fn register_custom_callback(
    broker: &ThunderBroker,
    request_id: u64,
) -> tokio::sync::mpsc::Receiver<BrokerOutput> {
    let (response_tx, response_rx) = mpsc::channel(1);
    // Register custom callback to handle the response
    broker
        .register_custom_callback(
            request_id,
            BrokerCallback {
                sender: response_tx,
            },
        )
        .await;
    response_rx
}

async fn send_thunder_request(
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

    pub async fn ripple_main_thunder_req_handler(
        rpc_req: RpcRequest,
        broker: Arc<ThunderBroker>,
        ws_tx: Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>>,
        mut subscription: RippleMainSubscription,
    ) {
        // create the unique request id for each request.
        let request_id = get_next_id();

        let mut response_rx = register_custom_callback(&broker, request_id).await;

        // create the request to the legacy storage
        let thunder_request = json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": rpc_req.method,
            "params": if rpc_req.params_json.is_empty() {
                json!({})
            } else {
                serde_json::from_str(&rpc_req.params_json).unwrap_or(json!({}))
            },
        })
        .to_string();

        info!(
            "ripple main to thunder sending request : {:?}",
            thunder_request
        );
        let method_name = rpc_req.method.clone();

        let contains_listen = method_name.contains("listen");
        if contains_listen {
            info!("The method_name '{}' contains 'listen'", method_name);

            // Check if the method is already present in the subscription map
            let already_subscribed = subscription
                .subscription_map
                .values()
                .any(|v| v == &method_name);

            if !already_subscribed {
                subscription
                    .subscription_map
                    .insert(request_id, method_name.clone());
                info!(
                    "Stored subscription: id={}, method={}",
                    request_id, method_name
                );
            } else {
                info!(
                    "Subscription for method '{}' already exists, not inserting again.",
                    method_name
                );
            }
        }

        // send the request to the legacy storage
        if let Err(e) = send_thunder_request(&ws_tx, &thunder_request).await {
            error!("Failed to send ripple main to thunder request: {:?}", e);
            broker.unregister_custom_callback(request_id).await;
            return;
        }

        tokio::spawn(async move {
            if let Some(response) = response_rx.recv().await {
                // Try to extract the "method" field from the response, if present
                let (response_method, response_id) = {
                    // Serialize the response.data to a JSON value before parsing
                    let msg = &response.data;
                    match serde_json::to_value(msg) {
                        Ok(val) => {
                            let method = val
                                .get("method")
                                .and_then(|m: &Value| m.as_str())
                                .map(|s| s.to_string());
                            let id = val.get("id").and_then(|i| i.as_u64());
                            (method, id)
                        }
                        Err(_) => (None, None),
                    }
                };
                info!(
                    "response_id:{:?}, method:{:?}",
                    response_id, response_method,
                );

                // If event response check the subscriptionmap for registered or not
                if method_name.ends_with(".register") {
                    let is_present = subscription
                        .subscription_map
                        .values()
                        .any(|v| v == &method_name);

                    if is_present {
                        //TO DO: send the response back to the caller
                    }
                }
            } else {
                error!(
                    "No response received for request_id {}: channel closed",
                    request_id
                );
            }
        });
    }
}

// #[cfg(test)]
// mod tests {
//     use crate::{
//         broker::{
//             endpoint_broker::{BrokerConnectRequest, EndpointBrokerState},
//             rules_engine::RuleEndpoint,
//         },
//         utils::test_utils::{MockWebsocket, WSMockData},
//     };

//     use super::*;

//     async fn get_thunderbroker(
//         tx: mpsc::Sender<bool>,
//         send_data: Vec<WSMockData>,
//         sender: mpsc::Sender<BrokerOutput>,
//         on_close: bool,
//     ) -> ThunderBroker {
//         // setup mock websocket server
//         let port = MockWebsocket::start(send_data, Vec::new(), tx, on_close).await;

//         let endpoint = RuleEndpoint {
//             url: format!("ws://127.0.0.1:{}", port),
//             protocol: crate::broker::rules_engine::RuleEndpointProtocol::Websocket,
//             jsonrpc: false,
//         };
//         let (tx, _) = mpsc::channel(1);
//         let request = BrokerConnectRequest::new("somekey".to_owned(), endpoint, tx);
//         let callback = BrokerCallback { sender };
//         let thunderbroker = ThunderBroker {
//             sender,
//             subscription_map: Default::default(),
//             cleaner: Default::default(),
//             status_manager: Default::default(),
//             default_callback: Default::default(),
//             data_migrator: Default::default(),
//             custom_callback_list: Default::default(),
//             composite_request_list: Default::default(),
//             composite_request_purge_started: Default::default(),
//         };
//         thunderbroker
//     }
// }
