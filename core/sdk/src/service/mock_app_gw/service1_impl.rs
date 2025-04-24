// Copyright 2025 Comcast Cable Communications Management, LLC
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
use std::sync::Arc;

use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use crate::service_trait::Service;

// Define the service ID as a constant
const SERVICE1_ID: &str = "mock:service:appgw:service1";
const APPGW_WS_URL: &str = "ws://127.0.0.1:1234";

pub struct MockService {
    id: String,
    status: bool,
    outbound_tx: Option<mpsc::Sender<Message>>,
}

impl MockService {
    pub fn new(service_id: &str, tx: Option<mpsc::Sender<Message>>) -> Self {
        Self {
            id: service_id.to_string(),
            status: false,
            outbound_tx: tx,
        }
    }

    pub fn start(&mut self) {
        self.status = true;
        println!("MockService with service id {} started.", self.id);
    }

    /*
    async fn stop(&mut self) {
        self.status = false;
        println!("MockService with service id {} stopped.", self.id);
        // send the close message to AppGW
        if let Some(sender) = self.get_sender() {
            sender.send(Message::Close(None)).await.unwrap();
        }
    }
    */

    pub fn is_running(&self) -> bool {
        self.status
    }

    fn get_sender(&self) -> Option<mpsc::Sender<Message>> {
        self.outbound_tx.clone()
    }

    async fn send_request_to_appgw(
        &self,
        request: Value,
    ) -> Result<(), tokio_tungstenite::tungstenite::Error> {
        if let Some(sender) = self.get_sender() {
            sender
                .send(Message::Text(request.to_string()))
                .await
                .unwrap();
            Ok(())
        } else {
            Err(tokio_tungstenite::tungstenite::Error::Io(
                std::io::Error::new(std::io::ErrorKind::Other, "No sender available"),
            ))
        }
    }
}

#[async_trait::async_trait]
impl Service for MockService {
    fn service_id(&self) -> &str {
        &self.id
    }

    async fn handle_inbound_request(&self, request: Value) -> Value {
        println!("[service1] received request: {}", request);

        let id = request["id"].clone(); // preserve same ID
        let method = request["method"].as_str().unwrap_or_default();

        let response = match method {
            "service1.get_status" => json!({ "status": "running" }),
            "service1.compute" => json!({ "value": 42 }),
            "service1.info" => json!({ "info": "service1 reporting" }),
            "service1.check" => json!({ "check": "ok" }),
            "service1.stats" => json!({ "cpu": 12.3, "mem": 256 }),
            "service1.good_bye" => {
                // send the unregitser request to AppGW
                let unregister_msg = json!({
                    "jsonrpc": "2.0",
                    "id": 888, // TBD get a unique ID
                    "method": "unregister",
                    "params": { "service_id": self.service_id() }
                });

                if let Err(e) = self.send_request_to_appgw(unregister_msg).await {
                    eprintln!(
                        "[{}] Failed to send unregister request: {}",
                        self.service_id(),
                        e
                    );
                }

                // Spawn a task to send a close message to the service after 2 seconds
                if let Some(sender) = self.get_sender() {
                    tokio::spawn(async move {
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                        sender.send(Message::Close(None)).await.unwrap();
                    });
                }

                // Return a response indicating that the service is stopping in 2 seconds
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "from_service": self.service_id(),
                        "response": "I'm exiting in 2s :-) bye bye"
                    }
                })
            }
            _ => {
                return json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32601,
                        "message": "Method not found in service1"
                    }
                });
            }
        };

        json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": response
        })
    }
}

pub async fn start_service1() {
    // Create channels for communication between AppGW and service
    // Inbound channel for receiving messages from AppGW
    let (inbound_tx, mut inbound_rx) = mpsc::channel::<Value>(32);
    // Outbound channel for sending messages to AppGW
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<Message>(32);

    // Example: send init request to AppGW
    let init_req = json!({
        "jsonrpc": "2.0",
        "id": 100,
        "method": "get_device_name"
    });
    outbound_tx
        .send(Message::Text(init_req.to_string()))
        .await
        .unwrap();

    let mut service = MockService::new(SERVICE1_ID, Some(outbound_tx.clone()));
    // DO: Service related init operations here
    // For example, start the service
    service.start();

    let svc = Arc::new(service);
    let svc_c = Arc::clone(&svc);
    tokio::spawn(async move {
        while let Some(req) = inbound_rx.recv().await {
            let result = svc_c.handle_inbound_request(req.clone()).await;
            println!("[service1] processed: {:?}", result);
            outbound_tx
                .send(Message::Text(result.to_string()))
                .await
                .unwrap();
        }
    });

    svc.clone()
        .run(APPGW_WS_URL, &mut outbound_rx, inbound_tx)
        .await;
}
