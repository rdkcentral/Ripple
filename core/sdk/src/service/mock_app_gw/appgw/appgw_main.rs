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
use super::{
    internal_api_config::load_internal_api_map,
    rpc_router::handle_jsonrpc,
    types::{ClientInfo, Clients, PendingMap, RoutedMap},
};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::{net::TcpListener, sync::Mutex};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use uuid::Uuid;

pub async fn start_app_gw() {
    let listener = TcpListener::bind("127.0.0.1:1234").await.unwrap();
    let clients: Clients = Arc::new(Mutex::new(HashMap::new()));
    let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
    let routed: RoutedMap = Arc::new(Mutex::new(HashMap::new()));
    let internal_api_map = load_internal_api_map();

    while let Ok((stream, _)) = listener.accept().await {
        let clients = Arc::clone(&clients);
        let pending = Arc::clone(&pending);
        let routed = Arc::clone(&routed);
        let internal_api_map = internal_api_map.clone();

        tokio::spawn(async move {
            let ws_stream = accept_async(stream).await.unwrap();
            let (mut write, mut read) = ws_stream.split();
            let (tx, mut rx) = mpsc::channel::<Message>(32);

            let client_id = Uuid::new_v4().to_string();
            clients.lock().await.insert(
                client_id.clone(),
                ClientInfo {
                    id: client_id.clone(),
                    tx: tx.clone(),
                    is_service: false,
                },
            );

            let writer_clients = Arc::clone(&clients);
            let writer_id = client_id.clone();
            tokio::spawn(async move {
                while let Some(msg) = rx.recv().await {
                    if let Err(e) = write.send(msg).await {
                        eprintln!("[AppGW] Write error for {}: {:?}", writer_id, e);
                        writer_clients.lock().await.remove(&writer_id);
                        break;
                    }
                }
            });

            while let Some(Ok(Message::Text(msg))) = read.next().await {
                println!("[AppGW] Received message from {}: {}", client_id, msg);
                if let Ok(v) = serde_json::from_str::<Value>(&msg) {
                    if v["method"] == "register" {
                        if let Some(sid) = v["params"]["service_id"].as_str() {
                            let mut map = clients.lock().await;
                            let short_id = sid.rsplit(':').next().unwrap_or(sid);
                            println!("Adding service {} to clients map", short_id.to_string());
                            map.insert(
                                short_id.to_string(),
                                ClientInfo {
                                    id: short_id.to_string(),
                                    tx: tx.clone(),
                                    is_service: true,
                                },
                            );
                        }
                    } else if v["method"] == "unregister" {
                        println!("[AppGW] Client {} requested unregister", client_id);
                        clients.lock().await.remove(&client_id);
                        break;
                    } else if v.get("method").is_some() {
                        handle_jsonrpc(
                            v,
                            &clients,
                            &pending,
                            &routed,
                            &client_id,
                            internal_api_map.clone(),
                        )
                        .await;
                    } else if v.get("result").is_some() || v.get("error").is_some() {
                        let id = v["id"].as_str().unwrap_or_default();
                        if let Some((agg_id, svc)) = id.split_once('|') {
                            let mut pending_map = pending.lock().await;
                            if let Some(p) = pending_map.get_mut(agg_id) {
                                let response_val = v
                                    .get("result")
                                    .cloned()
                                    .or_else(|| v.get("error").cloned())
                                    .unwrap_or_else(
                                        || json!({ "error": "no result or error field" }),
                                    );

                                p.responses.insert(svc.to_string(), response_val);
                                if p.responses.len() == p.expected.len() {
                                    let mut result = json!({});
                                    for s in &p.expected {
                                        let value = p
                                            .responses
                                            .get(s)
                                            .cloned()
                                            .unwrap_or(json!({ "error": "no response" }));
                                        result[s] = json!({ "from_service": s, "response": value });
                                    }
                                    let final_msg = json!({
                                        "jsonrpc": "2.0",
                                        "id": p.original_id,
                                        "result": result
                                    });
                                    let _ = p
                                        .tester_tx
                                        .send(Message::Text(final_msg.to_string()))
                                        .await;
                                    pending_map.remove(agg_id);
                                }
                            }
                        } else {
                            let mut routed_map = routed.lock().await;
                            if let Some(tester_tx) = routed_map.remove(id) {
                                let wrapped = if v.get("result").is_some() {
                                    json!({
                                        "jsonrpc": "2.0",
                                        "id": id,
                                        "result": {
                                            "from_service": client_id,
                                            "response": v["result"]
                                        }
                                    })
                                } else {
                                    json!({
                                        "jsonrpc": "2.0",
                                        "id": id,
                                        "error": {
                                            "from_service": client_id,
                                            "response": v["error"]
                                        }
                                    })
                                };
                                let _ = tester_tx.send(Message::Text(wrapped.to_string())).await;
                            }
                        }
                    }
                }
            }

            clients.lock().await.remove(&client_id);
            println!("[AppGW] Client {} disconnected", client_id);
        });
    }
}
