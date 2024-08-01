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

use crate::broker::broker_utils::BrokerUtils;

use super::endpoint_broker::{
    BrokerCallback, BrokerCleaner, BrokerConnectRequest, BrokerOutputForwarder, BrokerRequest,
    BrokerSender, EndpointBroker,
};
use futures_util::{SinkExt, StreamExt};
use ripple_sdk::{
    log::{debug, error},
    tokio::{self, sync::mpsc},
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
                let (mut ws_tx, mut ws_rx) =
                    BrokerUtils::get_ws_broker(&endpoint.get_url(), None).await;

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
                                        Self::handle_jsonrpc_response(t.as_bytes(),callback.clone())
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
                            if let Ok(updated_request) = Self::update_request(&request) {
                                debug!("Sending request to broker {}", updated_request);
                                let _feed = ws_tx.feed(tokio_tungstenite::tungstenite::Message::Text(updated_request)).await;
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
            let (mut ws_tx, mut ws_rx) =
                BrokerUtils::get_ws_broker(&url, Some(alias.clone())).await;

            tokio::pin! {
                let read = ws_rx.next();
            }

            loop {
                tokio::select!(
                    Some(value) = &mut read => {
                        match value {
                            Ok(v) => {
                                if let tokio_tungstenite::tungstenite::Message::Text(t) = v {
                                    // send the incoming text without context back to the sender
                                    if let Err(e) = BrokerOutputForwarder::handle_non_jsonrpc_response(
                                        t.as_bytes(),
                                        callback_c.clone(),
                                        request_c.clone(),
                                    ) {
                                        error!("error forwarding {}", e);
                                    }
                                }
                            },
                            Err(e) => {
                                error!("Broker Websocket error on read {:?}", e);
                                break;
                            }
                        }

                    },
                    Some(request) = tr.recv() => {
                        debug!("Recieved cleaner request for {}", request);
                        if request.eq(&app_id) {
                            let _feed = ws_tx.feed(tokio_tungstenite::tungstenite::Message::Close(None)).await;
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
    fn get_broker(request: BrokerConnectRequest, callback: BrokerCallback) -> Self {
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
        broker::{
            endpoint_broker::{BrokerOutput, BrokerRequest},
            rules_engine::{Rule, RuleEndpoint, RuleTransform},
        },
        utils::test_utils::{MockWebsocket, WSMockData},
    };
    use ripple_sdk::api::gateway::rpc_gateway_api::RpcRequest;
    use serde_json::json;

    use super::*;

    async fn setup_broker(
        tx: mpsc::Sender<bool>,
        send_data: Vec<WSMockData>,
        sender: mpsc::Sender<BrokerOutput>,
        on_close: bool,
    ) -> WebsocketBroker {
        // setup mock websocket server
        let port = MockWebsocket::start(send_data, Vec::new(), tx, on_close).await;

        let endpoint = RuleEndpoint {
            url: format!("ws://127.0.0.1:{}", port),
            protocol: crate::broker::rules_engine::RuleEndpointProtocol::Websocket,
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
        let send_data = vec![WSMockData::get(json!({"key":"value"}).to_string())];

        let broker = setup_broker(tx, send_data, sender, false).await;
        // Use Broker to connect to it
        let request = BrokerRequest {
            rpc: RpcRequest::get_new_internal("some_method".to_owned(), None),
            rule: Rule {
                alias: "".to_owned(),
                transform: RuleTransform::default(),
                endpoint: None,
            },
            subscription_processed: None,
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
            },
            subscription_processed: None,
        };
        let id = request.get_id();

        broker.sender.send(request).await.unwrap();

        broker.cleaner.cleaner.unwrap().send(id).await.unwrap();
        // See if ws is closed
        assert!(tr.recv().await.unwrap())
    }
}
