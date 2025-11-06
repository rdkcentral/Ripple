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
use crate::appgw::types::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use std::sync::atomic::{AtomicU64, Ordering};

// Define a global atomic counter for generating unique u64 IDs
static REQ_ID_COUNTER: AtomicU64 = AtomicU64::new(1000);

struct CleanupContext<'a> {
    service_id: &'a str,
    clients: &'a Clients,
    routed: Option<&'a RoutedMap>,
    pending: Option<&'a PendingMap>,
    sender_id: &'a str,
    original_id: u64,
    error: CleanupError,
}

struct CleanupError {
    code: i64,
    message: String,
}

pub async fn handle_jsonrpc_request(
    v: &Value,
    clients: &Clients,
    pending: &PendingMap,
    routed: &RoutedMap,
    sender_id: &str,
    internal_api_map: Arc<HashMap<String, Value>>,
    service_rules: &HashMap<String, String>,
) {
    let method = v["method"].as_str().unwrap_or_default();

    match method {
        "aggregate.service_status" => {
            handle_aggregate_call(v, clients, pending, sender_id).await;
        }
        _ => {
            handle_direct_or_internal_call(
                v,
                clients,
                routed,
                sender_id,
                internal_api_map,
                service_rules,
            )
            .await;
        }
    }
}

async fn handle_aggregate_call(
    v: &Value,
    clients: &Clients,
    pending: &PendingMap,
    sender_id: &str,
) {
    let id = match parse_request_id(v).await {
        Some(id) => id,
        None => {
            send_invalid_id_error(v, clients, sender_id).await;
            return;
        }
    };

    let available_services = find_available_services(
        clients,
        &["mock:service:appgw:service1", "mock:service:appgw:service2"],
    )
    .await;

    let Some(sender) = get_sender(clients, sender_id).await else {
        return;
    };

    if available_services.is_empty() {
        send_no_services_error(id, sender).await;
        return;
    }

    let mut tracker = initialize_tracker(id, sender.clone(), available_services.as_slice());

    let agg_id = send_requests_to_services(
        available_services,
        &mut tracker,
        clients,
        pending,
        sender_id,
        id,
    )
    .await
    .unwrap_or(0);

    finalize_or_store_tracker(tracker, pending, id, agg_id).await;
}

async fn handle_direct_or_internal_call(
    v: &Value,
    clients: &Clients,
    routed: &RoutedMap,
    sender_id: &str,
    internal_api_map: Arc<HashMap<String, Value>>,
    service_rules: &HashMap<String, String>,
) {
    let id = match parse_request_id(v).await {
        Some(id) => id,
        None => {
            send_invalid_id_error(v, clients, sender_id).await;
            return;
        }
    };

    let method = v["method"].as_str().unwrap_or_default();

    if let Some(service_id) = match_service_rules(method, service_rules) {
        route_to_service(v, clients, routed, sender_id, id, &service_id).await;
    } else if let Some(response) = internal_api_map.get(method) {
        send_internal_api_response(id, response, clients, sender_id).await;
    } else {
        send_method_not_found_error(id, clients, sender_id, method).await;
    }
}

// Reusable helper functions
async fn parse_request_id(v: &Value) -> Option<u64> {
    v["id"].as_u64()
}

async fn find_available_services(
    clients: &Clients,
    services: &[&str],
) -> Vec<(String, mpsc::Sender<Message>)> {
    let mut available = vec![];
    let map = clients.lock().await;
    for &svc in services {
        if let Some(client_info) = map
            .iter()
            .find(|(_, c)| c.is_service && c.service_id == Some(svc.to_string()))
        {
            available.push((svc.to_string(), client_info.1.tx.clone()));
        }
    }
    available
}

async fn get_sender(clients: &Clients, sender_id: &str) -> Option<mpsc::Sender<Message>> {
    clients.lock().await.get(sender_id).map(|c| c.tx.clone())
}

fn initialize_tracker(
    id: u64,
    sender: mpsc::Sender<Message>,
    available_services: &[(String, mpsc::Sender<Message>)],
) -> PendingAggregate {
    PendingAggregate {
        original_id: id,
        sender_tx: sender,
        expected: available_services.iter().map(|(s, _)| s.clone()).collect(),
        responses: HashMap::new(),
    }
}

async fn send_requests_to_services(
    available_services: Vec<(String, mpsc::Sender<Message>)>,
    tracker: &mut PendingAggregate,
    clients: &Clients,
    pending: &PendingMap,
    sender_id: &str,
    id: u64,
) -> Result<u64, ()> {
    let agg_id = REQ_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    for (svc, tx) in available_services {
        let method = svc.split(':').last().unwrap_or_default();
        let req = json!({
            "jsonrpc": "2.0",
            "id": agg_id,
            "method": format!("{}.get_status", method)
        });
        if (tx.send(Message::Text(req.to_string())).await).is_err() {
            tracker
                .responses
                .insert(svc.clone(), json!({ "error": "send failed" }));
            cleanup_disconnected_service(CleanupContext {
                service_id: &svc,
                clients,
                routed: None,
                pending: Some(pending),
                sender_id,
                original_id: id,
                error: CleanupError {
                    code: -32002,
                    message: format!("Service {} is not available", svc),
                },
            })
            .await;
        }
    }
    Ok(agg_id)
}

async fn finalize_or_store_tracker(
    mut tracker: PendingAggregate,
    pending: &PendingMap,
    id: u64,
    agg_id: u64,
) {
    if tracker.responses.len() == tracker.expected.len() {
        let mut result = json!({});
        for s in &tracker.expected {
            let val = tracker
                .responses
                .remove(s)
                .unwrap_or(json!({ "error": "no response" }));
            result[s] = json!({ "from_service": s, "response": val });
        }
        let _ = tracker
            .sender_tx
            .send(Message::Text(
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": result
                })
                .to_string(),
            ))
            .await;
    } else {
        pending.lock().await.insert(agg_id, tracker);
    }
}
///
/// Matches a method name against a set of service routing rules and returns the alias
/// of the best-matching rule, if one exists.
///
/// Exact matches are given the highest priority (`usize::MAX`).
/// Wildcard matches are given a lower priority based on the length of the prefix.
/// The function selects the best match based on priority
///    - Exact matches take precedence over prefix matches.
///    - Among prefix matches, the longest prefix is chosen.
///
fn match_service_rules(method: &str, service_rules: &HashMap<String, String>) -> Option<String> {
    service_rules
        .iter()
        .filter_map(|(pattern, alias)| {
            if pattern.ends_with(".*") {
                let prefix = &pattern[..pattern.len() - 1];
                if method.starts_with(prefix) {
                    return Some((prefix.len(), alias));
                }
            } else if pattern == method {
                return Some((usize::MAX, alias));
            }
            None
        })
        .max_by_key(|(priority, _)| *priority)
        .map(|(_, alias)| alias.clone())
}

async fn route_to_service(
    v: &Value,
    clients: &Clients,
    routed: &RoutedMap,
    sender_id: &str,
    id: u64,
    service_id: &str,
) {
    // Extract necessary data while holding the lock
    println!("Routing message {:#?} to service: {}", v, service_id);
    let (svc_tx, sender_tx) = {
        let map = clients.lock().await;

        // Find the service transmitter
        let svc_tx = map
            .iter()
            .find(|(_, c)| c.is_service && c.service_id == Some(service_id.to_string()))
            .map(|(_, c)| c.tx.clone());

        // Find the sender's transmitter
        let sender_tx = map.get(sender_id).map(|c| c.tx.clone());

        (svc_tx, sender_tx)
    }; // Lock is released here

    // If the service transmitter is not found, send an error
    let Some(svc_tx) = svc_tx else {
        send_service_not_available_error(id, clients, sender_id, service_id).await;
        return;
    };

    // If the sender's transmitter is not found, return early
    let Some(sender_tx) = sender_tx else {
        return;
    };

    // Add to the routed map
    let req_id = REQ_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    routed.lock().await.insert(
        req_id,
        RoutedRequest {
            service_id: service_id.to_string(),
            original_id: id,
            sender_tx: sender_tx.clone(),
        },
    );

    // Send the message to the service
    let req = json!({
        "jsonrpc": "2.0",
        "id": req_id,
        "method": v["method"],
        "params": v["params"]
    });
    if let Err(err) = svc_tx.send(Message::Text(req.to_string())).await {
        eprintln!(
            "Failed to send message to service {} for method {}: {:?}",
            service_id, v["method"], err
        );

        // Call the cleanup function
        cleanup_disconnected_service(CleanupContext {
            service_id,
            clients,
            routed: Some(routed),
            pending: None,
            sender_id,
            original_id: id,
            error: CleanupError {
                code: -32002,
                message: format!(
                    "Service {} is not available for the method {}",
                    service_id, v["method"]
                ),
            },
        })
        .await;
    }
}

/// Helper to send an internal API response if the request has an "id".
pub async fn send_response(v: &Value, resp: Value, clients: &Clients, client_id: &str) {
    if let Some(id) = v["id"].as_u64() {
        send_internal_api_response(id, &resp, clients, client_id).await;
    }
}

async fn send_internal_api_response(id: u64, response: &Value, clients: &Clients, sender_id: &str) {
    println!("Request Processed by AppGW : Response: {:#?}", response);
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
    if let Some(tx) = clients.lock().await.get(sender_id).map(|c| c.tx.clone()) {
        let _ = tx.send(Message::Text(reply.to_string())).await;
    }
}

async fn cleanup_disconnected_service(context: CleanupContext<'_>) {
    let CleanupContext {
        service_id,
        clients,
        routed,
        pending,
        sender_id,
        original_id,
        error,
    } = context;

    // Lock the clients map and remove the service
    let mut map = clients.lock().await;
    if let Some(service_key) = map
        .iter()
        .find(|(_, c)| c.service_id.as_deref() == Some(service_id))
        .map(|(key, _)| key.clone())
    {
        map.remove(&service_key);
    }

    // Clean up entries in the routed map
    if let Some(routed) = routed {
        let mut routed_map = routed.lock().await;
        routed_map.retain(|_, req| req.service_id != service_id);
    }

    // Clean up entries in the pending map (if provided)
    if let Some(pending_map) = pending {
        let mut pending_map = pending_map.lock().await;
        pending_map.retain(|_, tracker| !tracker.expected.contains(service_id));
    }

    // Send an error response back to the caller
    if let Some(tx) = map.get(sender_id).map(|c| c.tx.clone()) {
        let err_response = json!({
            "jsonrpc": "2.0",
            "id": original_id,
            "error": { "code": error.code, "message": error.message }
        });
        let _ = tx.send(Message::Text(err_response.to_string())).await;
    }

    // Flush tokio worker thread allocator caches after service cleanup
    tokio::task::yield_now().await;
}

// Error handling functions
async fn send_invalid_id_error(v: &Value, clients: &Clients, sender_id: &str) {
    eprintln!(
        "Invalid id format. Expected u64, but received: {:?}",
        v["id"]
    );
    let err = json!({
        "jsonrpc": "2.0",
        "id": v["id"],
        "error": { "code": -32600, "message": "Invalid id format" }
    });
    if let Some(tx) = clients.lock().await.get(sender_id).map(|c| c.tx.clone()) {
        let _ = tx.send(Message::Text(err.to_string())).await;
    }
}

async fn send_no_services_error(id: u64, sender: mpsc::Sender<Message>) {
    eprintln!("No services available for processing the request");
    let err = json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": -32001, "message": "No services available" }
    });
    let _ = sender.send(Message::Text(err.to_string())).await;
}

async fn send_method_not_found_error(id: u64, clients: &Clients, sender_id: &str, method: &str) {
    eprint!("send_method_not_found_error {}", method);
    let err = json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": -32601, "message": format!("Method {} not found", method) }
    });
    if let Some(tx) = clients.lock().await.get(sender_id).map(|c| c.tx.clone()) {
        let _ = tx.send(Message::Text(err.to_string())).await;
    }
}

async fn send_service_not_available_error(
    id: u64,
    clients: &Clients,
    sender_id: &str,
    service_id: &str,
) {
    eprintln!(
        "The Service {} is not available for processing the method",
        service_id
    );
    let err = json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": -32001, "message": format!("The service {} is not available for processing the method", service_id) }
    });
    if let Some(tx) = clients.lock().await.get(sender_id).map(|c| c.tx.clone()) {
        let _ = tx.send(Message::Text(err.to_string())).await;
    }
}
