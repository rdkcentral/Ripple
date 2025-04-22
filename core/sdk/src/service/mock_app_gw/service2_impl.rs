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

struct Service2;

#[async_trait::async_trait]
impl Service for Service2 {
    fn service_id(&self) -> &str {
        "urn:mydomain:appgw:service2"
    }

    async fn handle_inbound_request(&self, request: Value) -> Value {
        println!("[service2] received request: {}", request);

        let id = request["id"].clone(); // preserve same ID
        let method = request["method"].as_str().unwrap_or_default();
        let response = match method {
            "service2.get_status" => json!({"status": "running"}),
            "service2.compute" => json!({"result": 42}),
            "service2.info" => json!({"info": "service2 reporting"}),
            "service2.check" => json!({"check": "ok"}),
            "service2.stats" => json!({"cpu": 12.3, "mem": 256}),
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

        json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": response
        })
    }
}

pub async fn start_service2() {
    let (tx, mut rx) = mpsc::channel::<Message>(32);
    let (appgw_tx, mut appgw_rx) = mpsc::channel::<Value>(32);

    // Example: send init request to AppGW
    let init_req = json!({
        "jsonrpc": "2.0",
        "id": "svc1-init-1",
        "method": "get_device_name"
    });
    tx.send(Message::Text(init_req.to_string())).await.unwrap();

    let svc = Arc::new(Service2);
    // Todo: Service related init operations

    let svc_task = Arc::clone(&svc);
    tokio::spawn(async move {
        while let Some(req) = appgw_rx.recv().await {
            let result = svc_task.handle_inbound_request(req.clone()).await;
            println!("[service2] processed: {:?}", result);
            let _ = tx.send(Message::Text(result.to_string())).await.unwrap();
        }
    });

    svc.clone()
        .run("ws://127.0.0.1:1234", &mut rx, appgw_tx)
        .await;
}
