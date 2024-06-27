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

use crate::utils::rpc_utils::extract_tcp_port;

use super::{
    endpoint_broker::{BrokerCallback, BrokerCleaner, BrokerSender, EndpointBroker},
    rules_engine::RuleEndpoint,
};
use futures_util::{SinkExt, StreamExt};
use ripple_sdk::{
    log::{debug, error, info},
    tokio::{self, net::TcpStream, sync::mpsc},
};
use std::time::Duration;
use tokio_tungstenite::client_async;

pub struct WebsocketBroker {
    sender: BrokerSender,
    cleaner: BrokerCleaner,
}

impl WebsocketBroker {
    fn start(endpoint: RuleEndpoint, callback: BrokerCallback) -> Self {
        let (tx, mut tr) = mpsc::channel(10);
        let broker = BrokerSender { sender: tx };
        tokio::spawn(async move {
            if endpoint.jsonrpc {
                let (url, tcp) = connect(&endpoint.url, None).await;

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
                while let Some(v) = tr.recv().await {
                    let request_c = v.clone();
                    let callback_c = callback.clone();
                    let url = endpoint.url.clone();
                    tokio::spawn(async move {
                        let (final_url, tcp) = connect(&url, Some(v.rule.alias)).await;

                        let (stream, _) = client_async(final_url, tcp).await.unwrap();
                        let (_, ws_rx) = stream.split();

                        let _ = ws_rx
                            .for_each(|message| async {
                                if let Ok(tokio_tungstenite::tungstenite::Message::Text(t)) =
                                    message
                                {
                                    // send the incoming text without context back to the sender
                                    if let Err(e) = Self::handle_non_jsonrpc_response(
                                        t.as_bytes(),
                                        callback_c.clone(),
                                        &request_c,
                                    ) {
                                        error!("error forwarding {}", e);
                                    }
                                }
                            })
                            .await;
                    });
                }
                true
            }
        });
        Self {
            sender: broker,
            cleaner: BrokerCleaner { cleaner: None },
        }
    }
}

async fn connect(endpoint: &str, alias: Option<String>) -> (url::Url, TcpStream) {
    info!("Broker Endpoint url {}", endpoint);
    let url_path = if let Some(a) = alias {
        format!("{}{}", endpoint, a)
    } else {
        endpoint.to_owned()
    };
    let url = url::Url::parse(&url_path).unwrap();
    let port = extract_tcp_port(endpoint);
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
    (url, tcp)
}

impl EndpointBroker for WebsocketBroker {
    fn get_broker(endpoint: RuleEndpoint, callback: BrokerCallback) -> Self {
        Self::start(endpoint, callback)
    }

    fn get_sender(&self) -> BrokerSender {
        self.sender.clone()
    }

    fn get_cleaner(&self) -> BrokerCleaner {
        self.cleaner.clone()
    }
}
