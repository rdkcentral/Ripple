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

const APPGW_WS_URL: &str = "127.0.0.1:3474";

struct Service2;

#[async_trait::async_trait]
impl Service for Service2 {
    fn service_id(&self) -> &str {
        "mock:service:appgw:service2"
    }

    async fn handle_inbound_request(&self, request: Value) -> Value {
        println!("[service2] received request: {}", request);

        let id = request["id"].clone(); // preserve same ID
        let method = request["method"].as_str().unwrap_or_default();
        let response = match method {
            "service2.get_status" => json!({"result": {"status": "running" }}),
            "service2.compute" => json!({"result": {"value": 42 }}),
            "service2.check" => json!({"error": {"value": "Not permitted" }}),
            _ => {
                return json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32601,
                        "message": "Method not found in service2"
                    }
                });
            }
        };

        let mut reply = json!({
            "jsonrpc": "2.0",
            "id": id,
        });

        if let Some(result) = response.get("result") {
            reply["result"] = result.clone();
        } else if let Some(error) = response.get("error") {
            reply["error"] = error.clone();
        } else {
            reply["error"] = json!({"message": "Unknown response format"});
        }
        reply
    }
}

pub async fn start_service2() {
    // Service creation and other Service init related operations here
    let svc = Arc::new(Service2);
    // DO: Add service initialization logic here, if needed
    // svc.start();
    println!("[service2] Service started with ID: {}", svc.service_id());
    // Create Inbound channel for collecting and processing messages from AppGW received through wx_receiver
    let (inbound_tx, mut inbound_rx) = mpsc::channel::<Value>(32);
    // Create Outbound channel for collecting and sending messages to AppGW through wx_transport
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<Message>(32);

    // Example: send request to AppGW to get device name
    // This is just an example, you can modify the request as needed
    let init_req = json!({
        "jsonrpc": "2.0",
        "id": 200,
        "method": "Localization.countryCode"
    });
    outbound_tx
        .send(Message::Text(init_req.to_string()))
        .await
        .unwrap();

    let svc_c = Arc::clone(&svc);
    tokio::spawn(async move {
        while let Some(req) = inbound_rx.recv().await {
            let result = svc_c.handle_inbound_request(req.clone()).await;
            println!("[service2] processed: {:?}", result);
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
