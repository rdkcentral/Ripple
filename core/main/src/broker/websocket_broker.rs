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
    BrokerCallback, BrokerCleaner, BrokerConnectRequest, BrokerOutputForwarder, BrokerRequest,
    BrokerSender, EndpointBroker,
};
use crate::broker::endpoint_broker::EndpointBrokerState;
use crate::state::platform_state::PlatformState;
use futures_util::{SinkExt, StreamExt};
use ripple_sdk::tokio_tungstenite::tungstenite::Message;
use ripple_sdk::{
    api::observability::log_signal::LogSignal,
    log::{debug, error},
    tokio::{self, sync::mpsc},
    utils::ws_utils::{WebSocketConfigBuilder, WebSocketUtils},
};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
pub struct WebsocketBroker {
    sender: BrokerSender,
    cleaner: BrokerCleaner,
}

impl WebsocketBroker {
    fn start(request: BrokerConnectRequest, callback: BrokerCallback) -> Self {
        let endpoint = request.endpoint.clone();
        let (tx, mut tr) = mpsc::channel(10);
        let (cleaner_tx, mut cleaner_tr) = mpsc::channel::<String>(1);
        let non_json_rpc_map: Arc<RwLock<HashMap<String, Vec<mpsc::Sender<String>>>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let map_clone = non_json_rpc_map.clone();
        let broker = BrokerSender { sender: tx };
        tokio::spawn(async move {
            if endpoint.jsonrpc {
                let resp = WebSocketUtils::get_ws_stream(&endpoint.get_url(), None).await;
                if resp.is_err() {
                    error!("Error connecting to websocket broker");
                    tr.close();
                    return false;
                }
                let (mut ws_tx, mut ws_rx) = resp.unwrap();

                tokio::pin! {
                    let read = ws_rx.next();
                }
                loop {
                    tokio::select! {
                        Some(value) = &mut read => {
                            match value {
                                Ok(v) => {
                                    if let Message::Text(t) = v {
                                        // send the incoming text without context back to the sender
                                       match  Self::handle_jsonrpc_response(t.as_bytes(),callback.clone(), None) {
                                             Ok(_) => {},
                                             Err(e) => {
                                                  error!("error forwarding {}", e);
                                             }
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
                            LogSignal::new(
                                "websocket_broker".to_string(),
                                format!("Got request from receiver for broker: {:?}", request),
                                request.rpc.ctx.clone(),
                            )
                            .emit_debug();
                            if let Ok(updated_request) = Self::update_request(&request) {
                                LogSignal::new(
                                    "websocket_broker".to_string(),
                                    format!("update request: {:?}", request),
                                    request.rpc.ctx.clone(),
                                )
                                .emit_debug();
                                let _feed = ws_tx.feed(Message::Text(updated_request)).await;
                                let _flush = ws_tx.flush().await;
                            }

                        }
                    }
                }
            } else {
                let cleaner_clone = non_json_rpc_map.clone();
                tokio::spawn(async move {
                    while let Some(v) = cleaner_tr.recv().await {
                        {
                            if let Some(cleaner_list) =
                                { cleaner_clone.write().unwrap().remove(&v) }
                            {
                                for sender in cleaner_list {
                                    if sender.try_send(v.clone()).is_err() {
                                        error!("Cleaning up listener");
                                    }
                                }
                            }
                        }
                    }
                });

                while let Some(v) = tr.recv().await {
                    let id = v.get_id();
                    let cleaner = WSNotificationBroker::start(
                        v.clone(),
                        callback.clone(),
                        endpoint.get_url().clone(),
                    );
                    {
                        let mut map = map_clone.write().unwrap();
                        let mut sender_list = map.remove(&id).unwrap_or_default();
                        sender_list.push(cleaner);
                        let _ = map.insert(id, sender_list);
                    }
                }

                true
            }
        });

        Self {
            sender: broker,
            cleaner: BrokerCleaner {
                cleaner: Some(cleaner_tx),
            },
        }
    }
}

pub struct WSNotificationBroker;

impl WSNotificationBroker {
    fn start(
        request_c: BrokerRequest,
        callback_c: BrokerCallback,
        url: String,
    ) -> mpsc::Sender<String> {
        let (tx, mut tr) = mpsc::channel::<String>(1);
        tokio::spawn(async move {
            let app_id = request_c.get_id();
            let alias = request_c.rule.alias.clone();
            #[cfg(not(test))]
            let config = WebSocketConfigBuilder::default().alias(alias).build();
            #[cfg(test)]
            let config = WebSocketConfigBuilder::default()
                .alias(alias)
                .retry(0)
                .fail_after(1)
                .build();

            let resp = WebSocketUtils::get_ws_stream(&url, Some(config)).await;
            if resp.is_err() {
                error!("Error connecting to websocket broker");
                tr.close();
                return;
            }
            let (mut ws_tx, mut ws_rx) = resp.unwrap();

            tokio::pin! {
                let read = ws_rx.next();
            }

            loop {
                tokio::select!(
                    Some(value) = &mut read => {
                        match value {
                            Ok(v) => {
                                if let Message::Text(t) = v {
                                    // send the incoming text without context back to the sender
                                    if let Err(e) = BrokerOutputForwarder::handle_non_jsonrpc_response(
                                        t.as_bytes(),
                                        callback_c.clone(),
                                        request_c.clone(),
                                    ) {
                                        LogSignal::new("websocket_broker".to_string(), "handle_jsonrpc_response".to_string(), request_c.rpc.ctx.clone())
                                        .with_diagnostic_context_item("error forwarding", &format!("{:?}", e))
                                        .emit_error();
                                    }
                                }
                            },
                            Err(e) => {
                                LogSignal::new("websocket_broker".to_string(), "Broker Websocket error on read".to_string(), request_c.rpc.ctx.clone())
                                    .with_diagnostic_context_item("Broker Websocket error on read", &format!("{:?}", e))
                                    .emit_error();
                                break;
                            }
                        }

                    },
                    Some(request) = tr.recv() => {
                        debug!("Recieved cleaner request for {}", request);
                        if request.eq(&app_id) {
                            let _feed = ws_tx.feed(Message::Close(None)).await;
                            let _flush = ws_tx.flush().await;
                            break;
                        }
                    }
                )
            }
        });
        tx
    }
}

impl EndpointBroker for WebsocketBroker {
    fn get_broker(
        _ps: Option<PlatformState>,
        request: BrokerConnectRequest,
        callback: BrokerCallback,
        _broker_state: &mut EndpointBrokerState,
    ) -> Self {
        Self::start(request, callback)
    }

    fn get_sender(&self) -> BrokerSender {
        self.sender.clone()
    }

    fn get_cleaner(&self) -> BrokerCleaner {
        self.cleaner.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{
        broker::endpoint_broker::{BrokerOutput, BrokerRequest},
        utils::test_utils::{MockWebsocket, WSMockData},
    };
    use ripple_sdk::{
        api::{
            gateway::rpc_gateway_api::RpcRequest,
            rules_engine::{Rule, RuleEndpoint, RuleEndpointProtocol, RuleTransform},
        },
        utils::logger::init_logger,
    };
    use serde_json::json;

    use super::*;

    async fn setup_broker(
        tx: mpsc::Sender<bool>,
        send_data: Vec<WSMockData>,
        sender: mpsc::Sender<BrokerOutput>,
        on_close: bool,
    ) -> WebsocketBroker {
        // setup mock websocket server
        /*
        doing unrwap here because doing it "right" would require changing call chain for this function, as result types would
        have to change
         */
        let port = MockWebsocket::start(send_data, Vec::new(), tx, on_close)
            .await
            .unwrap();

        let endpoint = RuleEndpoint {
            url: format!("ws://127.0.0.1:{}", port),
            protocol: RuleEndpointProtocol::Websocket,
            jsonrpc: false,
        };
        let (tx, _) = mpsc::channel(1);
        let request = BrokerConnectRequest::new("somekey".to_owned(), endpoint, tx);
        let callback = BrokerCallback { sender };
        // Setup websocket broker
        WebsocketBroker::start(request, callback)
    }

    #[tokio::test]

    async fn connect_non_json_rpc_websocket() {
        let (tx, mut tr) = mpsc::channel(1);
        let (sender, mut rec) = mpsc::channel(1);
        let send_data = vec![WSMockData::get(json!({"key":"value"}).to_string(), None)];

        let broker = setup_broker(tx, send_data, sender, false).await;
        // Use Broker to connect to it
        let request = BrokerRequest {
            rpc: RpcRequest::get_new_internal("some_method".to_owned(), None),
            rule: Rule {
                alias: "".to_owned(),
                transform: RuleTransform::default(),
                endpoint: None,
                filter: None,
                event_handler: None,
                sources: None,
            },
            workflow_callback: None,
            subscription_processed: None,
            telemetry_response_listeners: vec![],
        };

        broker.sender.send(request).await.unwrap();

        // See if broker output gets the value

        let v = tokio::time::timeout(Duration::from_secs(2), rec.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(v
            .data
            .result
            .unwrap()
            .get("key")
            .unwrap()
            .as_str()
            .unwrap()
            .eq("value"));

        assert!(tr.recv().await.unwrap())
    }

    #[tokio::test]
    async fn connect_non_json_rpc_websocket_test_invalid_response() {
        let (tx, mut _tr) = mpsc::channel(1);
        let (sender, mut rec) = mpsc::channel(1);
        let send_data = vec![WSMockData::get(
            "invalid json rpc response".to_string(),
            None,
        )];

        let broker = setup_broker(tx, send_data, sender, false).await;
        // Use Broker to connect to it
        let request = BrokerRequest {
            rpc: RpcRequest::get_new_internal("some_method".to_owned(), None),
            rule: Rule {
                alias: "".to_owned(),
                transform: RuleTransform::default(),
                endpoint: None,
                filter: None,
                event_handler: None,
                sources: None,
            },
            workflow_callback: None,
            subscription_processed: None,
            telemetry_response_listeners: vec![],
        };

        broker.sender.send(request).await.unwrap();

        // See if broker output gets the value

        let v = tokio::time::timeout(Duration::from_secs(2), rec.recv()).await;
        assert!(v.is_err());
    }

    #[tokio::test]

    async fn cleanup_non_json_rpc_websocket() {
        let (tx, mut tr) = mpsc::channel(1);
        let (sender, _) = mpsc::channel(1);

        let broker = setup_broker(tx, Vec::new(), sender, true).await;
        // Use Broker to connect to it
        let request = BrokerRequest {
            rpc: RpcRequest::get_new_internal("some_method".to_owned(), None),
            rule: Rule {
                alias: "".to_owned(),
                transform: RuleTransform::default(),
                endpoint: None,
                filter: None,
                event_handler: None,
                sources: None,
            },
            workflow_callback: None,
            subscription_processed: None,
            telemetry_response_listeners: vec![],
        };
        let id = request.get_id();

        broker.sender.send(request).await.unwrap();

        broker.cleaner.cleaner.unwrap().send(id).await.unwrap();
        // See if ws is closed
        assert!(tr.recv().await.unwrap())
    }

    async fn setup_ws_notitification_broker(
        tx: mpsc::Sender<bool>,
        send_data: Vec<WSMockData>,
        callback: BrokerCallback,
        on_close: bool,
    ) -> mpsc::Sender<String> {
        // setup mock websocket server
        /*
        doing unrwap here because doing it "right" would require changing call chain for this function, as result types would
        have to change
         */
        let port = MockWebsocket::start(send_data, Vec::new(), tx, on_close)
            .await
            .unwrap();

        let endpoint = RuleEndpoint {
            url: format!("ws://127.0.0.1:{}", port),
            protocol: RuleEndpointProtocol::Websocket,
            jsonrpc: false,
        };

        let request = BrokerRequest {
            rpc: RpcRequest::get_new_internal("some_method".to_owned(), None),
            rule: Rule {
                alias: "".to_owned(),
                transform: RuleTransform::default(),
                endpoint: None,
                filter: None,
                event_handler: None,
                sources: None,
            },
            workflow_callback: None,
            subscription_processed: None,
            telemetry_response_listeners: vec![],
        };
        WSNotificationBroker::start(request, callback, endpoint.get_url().clone())
    }

    #[tokio::test]
    async fn ws_notification_broker_start_validate_handling_response() {
        let (tx, mut tr) = mpsc::channel(1);
        let (sender, mut rec) = mpsc::channel(1);
        let callback = BrokerCallback { sender };
        let send_data = vec![WSMockData::get(json!({"key":"value"}).to_string(), None)];

        let broker = setup_ws_notitification_broker(tx, send_data, callback, false).await;
        broker.send("test".to_owned()).await.unwrap();

        let v = tokio::time::timeout(Duration::from_secs(2), rec.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(v
            .data
            .result
            .unwrap()
            .get("key")
            .unwrap()
            .as_str()
            .unwrap()
            .eq("value"));
        assert!(tr.recv().await.unwrap());
    }

    #[tokio::test]
    async fn ws_notification_broker_start_validate_handling_invalid_json_rpc_response() {
        let (tx, mut _tr) = mpsc::channel(1);
        let (sender, mut rec) = mpsc::channel(1);
        let callback = BrokerCallback { sender };
        let send_data = vec![WSMockData::get(
            "invalid json rpc response".to_string(),
            None,
        )];

        let broker = setup_ws_notitification_broker(tx, send_data, callback, false).await;
        broker.send("test".to_owned()).await.unwrap();
        let v = tokio::time::timeout(Duration::from_secs(2), rec.recv()).await;
        assert!(v.is_err());
    }

    #[tokio::test]
    async fn ws_notification_broker_start_connection_timeout() {
        let (tx, mut _tr) = mpsc::channel(1);
        let (sender, mut rec) = mpsc::channel(1);
        let callback = BrokerCallback { sender };
        let send_data = vec![WSMockData::get(
            json!({"key":"value"}).to_string(),
            Some(1000000000),
        )];
        let _broker = setup_ws_notitification_broker(tx, send_data, callback, false).await;
        let v = tokio::time::timeout(Duration::from_secs(2), rec.recv()).await;
        assert!(v.is_err());
    }

    #[tokio::test]
    async fn ws_notification_broker_start_test_connection_error() {
        let _ = init_logger("ws tests".to_owned());
        let (sender, mut rec) = mpsc::channel(1);
        let callback = BrokerCallback { sender };

        let request = BrokerRequest {
            rpc: RpcRequest::get_new_internal("some_method".to_owned(), None),
            rule: Rule {
                alias: "".to_owned(),
                transform: RuleTransform::default(),
                endpoint: None,
                filter: None,
                event_handler: None,
                sources: None,
            },
            workflow_callback: None,
            subscription_processed: None,
            telemetry_response_listeners: vec![],
        };
        let port: u32 = 34743;
        let endpoint = RuleEndpoint {
            url: format!("ws://127.0.0.1:{}", port),
            protocol: RuleEndpointProtocol::Websocket,
            jsonrpc: false,
        };
        let _ = WSNotificationBroker::start(request, callback, endpoint.get_url().clone());
        assert!(rec.recv().await.is_none());
    }
}
