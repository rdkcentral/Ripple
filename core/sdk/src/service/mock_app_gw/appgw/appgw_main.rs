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
    internal_api_config::{load_internal_api_map, load_service_rules},
    rpc_router::handle_jsonrpc_request,
    types::{ClientInfo, Clients, PendingAggregate, PendingMap, RoutedMap, RoutedRequest},
};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::{net::TcpListener, sync::Mutex};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use uuid::Uuid;

const APPGW_BIND_ADDRESS: &str = "127.0.0.1:1234";

pub async fn start_app_gw() {
    let listener = TcpListener::bind(APPGW_BIND_ADDRESS).await.unwrap();
    let clients: Clients = Arc::new(Mutex::new(HashMap::new()));
    let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
    let routed: RoutedMap = Arc::new(Mutex::new(HashMap::new()));
    let internal_api_map = load_internal_api_map();
    let service_rules = load_service_rules();

    println!(
        "[AppGW] Waiting for connections on ws://{}",
        APPGW_BIND_ADDRESS
    );

    while let Ok((stream, _)) = listener.accept().await {
        let clients = Arc::clone(&clients);
        let pending = Arc::clone(&pending);
        let routed = Arc::clone(&routed);
        let internal_api_map = internal_api_map.clone();
        let service_rules = service_rules.clone();

        tokio::spawn(async move {
            handle_client_connection(
                stream,
                clients,
                pending,
                routed,
                internal_api_map,
                service_rules,
            )
            .await;
        });
    }
}

async fn handle_client_connection(
    stream: tokio::net::TcpStream,
    clients: Clients,
    pending: PendingMap,
    routed: RoutedMap,
    internal_api_map: Arc<HashMap<String, Value>>,
    service_rules: HashMap<String, String>,
) {
    let ws_stream = accept_async(stream).await.unwrap();
    let (mut write, mut read) = ws_stream.split();
    let (tx, mut rx) = mpsc::channel::<Message>(32);

    let client_id = register_client(&clients, tx).await;

    //tokio::spawn(handle_client_writer(write, rx, clients.clone(), client_id.clone()));
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
            process_client_message(
                v,
                &clients,
                &pending,
                &routed,
                &client_id,
                internal_api_map.clone(),
                &service_rules,
            )
            .await;
        }
    }

    clients.lock().await.remove(&client_id);
    println!("[AppGW] Client {} disconnected", client_id);
}

async fn register_client(clients: &Clients, tx: mpsc::Sender<Message>) -> String {
    let client_id = Uuid::new_v4().to_string();
    clients.lock().await.insert(
        client_id.clone(),
        ClientInfo {
            id: client_id.clone(),
            tx,
            is_service: false,
            service_id: None,
        },
    );
    client_id
}

async fn process_client_message(
    v: Value,
    clients: &Clients,
    pending: &PendingMap,
    routed: &RoutedMap,
    client_id: &str,
    internal_api_map: Arc<HashMap<String, Value>>,
    service_rules: &HashMap<String, String>,
) {
    if v["method"] == "register" {
        handle_register_message(v, clients, client_id).await;
    } else if v["method"] == "unregister" {
        handle_unregister_message(clients, client_id).await;
    } else if v.get("method").is_some() {
        handle_jsonrpc_request(
            v,
            clients,
            pending,
            routed,
            client_id,
            internal_api_map,
            service_rules,
        )
        .await;
    } else if v.get("result").is_some() || v.get("error").is_some() {
        handle_service_response(v, clients, pending, routed, client_id).await;
    }
}

async fn handle_register_message(v: Value, clients: &Clients, client_id: &str) {
    if let Some(sid) = v["params"]["service_id"].as_str() {
        let mut map = clients.lock().await;
        if let Some(client_info) = map.get_mut(client_id) {
            client_info.service_id = Some(sid.to_string());
            client_info.is_service = true;
        } else {
            println!("[AppGW] Client {} not found in map", client_id);
        }
        println!("[AppGW] Client {} registered as service {}", client_id, sid);
    }
}

async fn handle_unregister_message(clients: &Clients, client_id: &str) {
    println!("[AppGW] Client {} requested unregister", client_id);
    clients.lock().await.remove(client_id);
}

async fn process_aggregate_response(
    v: Value,
    clients: &Clients,
    tracker: &mut PendingAggregate,
    client_id: &str,
) {
    // Extract the response value (result or error)
    let response_val = v
        .get("result")
        .cloned()
        .or_else(|| v.get("error").cloned())
        .unwrap_or_else(|| json!({ "error": "no result or error field" }));

    // Get the service_id from the clients map
    let service_id = clients
        .lock()
        .await
        .get(client_id)
        .and_then(|c| c.service_id.clone())
        .unwrap_or_else(|| "unknown".to_string());

    // Store the response in the tracker
    tracker.responses.insert(service_id.clone(), response_val);

    // Check if all expected responses have been received
    if tracker.responses.len() == tracker.expected.len() {
        let mut result = json!({});
        for s in &tracker.expected {
            let value = tracker
                .responses
                .get(s)
                .cloned()
                .unwrap_or(json!({ "error": "no response" }));
            result[s] = json!({ "from_service": s, "response": value });
        }

        // Construct the final message
        let final_msg = json!({
            "jsonrpc": "2.0",
            "id": tracker.original_id,
            "result": result
        });

        // Send the final message to the client
        let _ = tracker
            .sender_tx
            .send(Message::Text(final_msg.to_string()))
            .await;
    }
}

async fn process_direct_response(
    v: Value,
    clients: &Clients,
    routed_req: RoutedRequest,
    client_id: &str,
) {
    // Get the service_id from the clients map
    let service_id = clients
        .lock()
        .await
        .get(client_id)
        .and_then(|c| c.service_id.clone())
        .unwrap_or_else(|| "unknown".to_string());

    println!(
        "[AppGW] Found routed request for id {} from service {}",
        routed_req.original_id, service_id
    );

    // Wrap the response in JSON-RPC format
    let wrapped = if v.get("result").is_some() {
        json!({
            "jsonrpc": "2.0",
            "id": routed_req.original_id,
            "result": {
                "from_service": service_id,
                "response": v["result"]
            }
        })
    } else {
        json!({
            "jsonrpc": "2.0",
            "id": routed_req.original_id,
            "error": {
                "from_service": service_id,
                "response": v["error"]
            }
        })
    };

    // Send the wrapped response to the client
    let _ = routed_req
        .sender_tx
        .send(Message::Text(wrapped.to_string()))
        .await;
}

async fn handle_service_response(
    v: Value,
    clients: &Clients,
    pending: &PendingMap,
    routed: &RoutedMap,
    client_id: &str,
) {
    let id = v["id"].as_u64().unwrap_or_default();
    let mut pending_map = pending.lock().await;

    if let Some(p) = pending_map.get_mut(&id) {
        process_aggregate_response(v, clients, p, client_id).await;
        // Remove the tracker if all responses are processed
        if p.responses.len() == p.expected.len() {
            pending_map.remove(&id);
        }
    } else {
        let mut routed_map = routed.lock().await;
        if let Some(routed_req) = routed_map.remove(&id) {
            process_direct_response(v, clients, routed_req, client_id).await;
        }
    }
}
