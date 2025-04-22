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
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

pub async fn handle_jsonrpc(
    v: Value,
    clients: &Clients,
    pending: &PendingMap,
    routed: &RoutedMap,
    sender_id: &str,
    internal_api_map: Arc<HashMap<String, Value>>,
) {
    let method = v["method"].as_str().unwrap_or_default();

    match method {
        "aggregate.system_health" | "aggregate.device_status" => {
            handle_aggregate_call(v, clients, pending, sender_id).await;
        }
        _ => {
            handle_direct_or_internal_call(v, clients, routed, sender_id, internal_api_map).await;
        }
    }
}

async fn handle_aggregate_call(v: Value, clients: &Clients, pending: &PendingMap, sender_id: &str) {
    let _method = v["method"].as_str().unwrap_or_default();
    let id = v["id"].as_str().unwrap_or("unknown").to_string();

    let services = ["service1", "service2"];
    let mut available = vec![];

    {
        let map = clients.lock().await;
        for &svc in &services {
            if let Some(c) = map.get(svc) {
                available.push((svc.to_string(), c.tx.clone()));
            }
        }
    }

    let Some(tester) = clients.lock().await.get(sender_id).map(|c| c.tx.clone()) else {
        return;
    };

    if available.is_empty() {
        let err = json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32001, "message": "No services available" }
        });
        let _ = tester.send(Message::Text(err.to_string())).await;
        return;
    }

    let mut tracker = PendingAggregate {
        original_id: id.clone(),
        tester_tx: tester.clone(),
        expected: available.iter().map(|(s, _)| s.clone()).collect(),
        responses: HashMap::new(),
    };

    let agg_id = format!("agg-{}", Uuid::new_v4());
    for (svc, tx) in available {
        let mid = format!("{}|{}", agg_id, svc);
        let req = json!({
            "jsonrpc": "2.0",
            "id": mid,
            "method": format!("{}.get_status", svc)
        });
        if let Err(_) = tx.send(Message::Text(req.to_string())).await {
            tracker
                .responses
                .insert(svc, json!({ "error": "send failed" }));
        }
    }

    if tracker.responses.len() == tracker.expected.len() {
        let mut result = json!({});
        for s in &tracker.expected {
            let val = tracker
                .responses
                .remove(s)
                .unwrap_or(json!({ "error": "no response" }));
            result[s] = json!({ "from_service": s, "response": val });
        }
        let _ = tester
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

async fn handle_direct_or_internal_call(
    v: Value,
    clients: &Clients,
    routed: &RoutedMap,
    sender_id: &str,
    internal_api_map: Arc<HashMap<String, Value>>,
) {
    let method = v["method"].as_str().unwrap_or_default();
    let id = v["id"].as_str().unwrap_or("unknown").to_string();

    if let Some((prefix, _)) = method.split_once('.') {
        let map = clients.lock().await;
        if let Some((_, svc)) = map.iter().find(|(_, c)| c.is_service && c.id == prefix) {
            if let Some(tester_tx) = map.get(sender_id).map(|c| c.tx.clone()) {
                routed.lock().await.insert(id.clone(), tester_tx);
            }

            let _ = svc.tx.send(Message::Text(v.to_string())).await;
            return;
        }
    }

    if let Some(response) = internal_api_map.get(method) {
        let reply = json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": response
        });
        if let Some(tx) = clients.lock().await.get(sender_id).map(|c| c.tx.clone()) {
            let _ = tx.send(Message::Text(reply.to_string())).await;
        }
        return;
    }

    if let Some(tester) = clients.lock().await.get(sender_id) {
        let err = json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32601, "message": "Method not found" }
        });
        let _ = tester.tx.send(Message::Text(err.to_string())).await;
    }
}
