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
use crate::client::jsonrpc_method_locator::JsonRpcMethodLocator;
use futures::SinkExt;
use futures::StreamExt;
use ripple_sdk::log::info;
use ripple_sdk::{futures, serde_json, tokio};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::protocol::Message;

pub trait MethodHandler: Send + Sync {
    fn handle_method(&self, request: &Value) -> Option<Value>;
    fn get_callsign(&self) -> String;
}

pub struct CustomMethodHandler;

impl MethodHandler for CustomMethodHandler {
    fn handle_method(&self, request: &Value) -> Option<Value> {
        let method = request.get("method");
        let locator = method
            .map(|method| JsonRpcMethodLocator::from_str(method.as_str().unwrap()).unwrap())
            .unwrap_or_default();
        match locator.method_name.as_str() {
            "register" => {
                // Handle subscription request
                let response = json!({
                    "jsonrpc": "2.0",
                    "result": "Subscribed",
                    "id": request.get("id"),
                });
                Some(response)
            }
            "unregister" => {
                // Handle unsubscription request
                let response = json!({
                    "jsonrpc": "2.0",
                    "result": "Unsubscribed",
                    "id": request.get("id"),
                });
                Some(response)
            }
            "testMethod" => {
                // Handle test method request
                let response = json!({
                    "jsonrpc": "2.0",
                    "result": "testMethod Request Response",
                    "id": request.get("id"),
                });
                Some(response)
            }
            _ => {
                if let Some(method) = request.get("method") {
                    info!("Received unknown method: {:?}", method);
                }
                // echo response here
                let response = json!({
                    "jsonrpc": "2.0",
                    "result": "Echo Response",
                    "id": request.get("id"),
                });
                Some(response)
            }
        }
    }
    fn get_callsign(&self) -> String {
        "org.test.threadpool".to_string()
    }
}
pub struct MockWebSocketServer {
    address: SocketAddr,
    sender: mpsc::Sender<Message>,
    method_handler: Arc<dyn MethodHandler + Send + Sync>,
}

impl MockWebSocketServer {
    pub fn new(address: &str, method_handler: Arc<dyn MethodHandler + Send + Sync>) -> Self {
        let address: SocketAddr = address.parse().unwrap();
        let (sender, _) = mpsc::channel(32);
        Self {
            address,
            sender,
            method_handler,
        }
    }

    pub async fn start(&self) {
        let sender = self.sender.clone();
        let address = self.address;
        let server = TcpListener::bind(&address)
            .await
            .expect("Failed to bind mock WebSocket server");
        while let Ok((stream, _)) = server.accept().await {
            let sender = sender.clone();
            let method_handler = self.method_handler.clone();

            tokio::spawn(async move {
                if let Ok(ws_stream) = accept_async(stream).await {
                    handle_connection(ws_stream, sender, method_handler).await;
                }
            });
        }
    }

    pub async fn stop(&self) {
        // Implement any specific server stopping logic here
    }
}

async fn handle_connection(
    ws_stream: tokio_tungstenite::WebSocketStream<TcpStream>,
    _sender: mpsc::Sender<Message>,
    method_handler: Arc<dyn MethodHandler>,
) {
    // Handle WebSocket connections here
    let (mut tx, mut rx) = ws_stream.split();

    while let Some(message) = rx.next().await {
        match message {
            Ok(msg) => {
                if let Message::Text(text) = msg {
                    if let Ok(request) = serde_json::from_str::<Value>(&text) {
                        if let Some(response) = method_handler.handle_method(&request) {
                            // Send the response if the method was handled
                            let response_text = serde_json::to_string(&response).unwrap();
                            if let Err(e) = tx.send(Message::Text(response_text)).await {
                                eprintln!("Failed to send response: {}", e);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Error receiving message: {}", e);
            }
        }
    }
}
