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
    rpc_router::{handle_jsonrpc_request, send_response},
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

const APPGW_BIND_ADDRESS: &str = "127.0.0.1:3474";

/// Starts the Mock Application Gateway (AppGW) service.
///
/// This function initializes the WebSocket server, listens for incoming client connections,
/// and spawns a new task to handle each client connection. It also sets up shared state
/// for managing connected clients, pending requests, routed requests, and service rules.
///
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
/// Handles a single client connection to the Application Gateway (AppGW).
///
/// This function manages the lifecycle of a client connection, including:
/// - Accepting the WebSocket connection.
/// - Registering the client in the `clients` Registry.
/// - Spawning a writer task to handle outgoing messages to the client.
/// - Processing incoming messages from the client.
/// - Cleaning up the client upon disconnection.
///
/// # Parameters
/// - `stream`: The TCP stream representing the client connection.
/// - `clients`: A reference to the `Clients` map, which stores information about connected clients.
/// - `pending`: A reference to the `PendingMap`, which tracks aggregate requests awaiting responses.
/// - `routed`: A reference to the `RoutedMap`, which tracks routed requests awaiting responses.
/// - `internal_api_map`: A reference to the internal API map, which contains predefined API responses.
/// - `service_rules`: A map of service routing rules used to route client requests.
///
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

    let client_id = add_client_to_registry(&clients, tx).await;

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
        println!("[AppGW] Received Message from {}: {:#?}", client_id, msg);
        if let Ok(v) = serde_json::from_str::<Value>(&msg) {
            process_client_request(
                &v,
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

/// Adds a new client to the registry of connected clients.
///
/// This function generates a unique client ID, creates a `ClientInfo` object for the client,
/// and inserts it into the `clients` map. The client is initialized with default values,
/// such as not being a service and having no associated service ID.
///
/// # Parameters
/// - `clients`: A reference to the `Clients` map, which stores information about connected clients.
/// - `tx`: The `mpsc::Sender<Message>` used to send messages to the client.
///
/// # Returns
/// - A `String` representing the unique client ID assigned to the client.
///
async fn add_client_to_registry(clients: &Clients, tx: mpsc::Sender<Message>) -> String {
    let client_id = Uuid::new_v4().to_string();
    println!("[AppGW] New client connected: {}", client_id);
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
/// Processes a client request received by the Application Gateway (AppGW).
///
/// This function determines the type of request received from the client and delegates
/// it to the appropriate handler. It supports the following types of requests:
/// - `register`: Registers the client as a service.
/// - `unregister`: Unregisters a service and remove the client.
/// - Other JSON-RPC requests: Routes the request to the appropriate service or internal API.
/// - JSON-RPC responses: Handles responses from services for routed or aggregate requests.
///
/// # Parameters
/// - `v`: The JSON-RPC request or response received from the client.
/// - `clients`: A reference to the `Clients` map, which stores information about connected clients.
/// - `pending`: A reference to the `PendingMap`, which tracks aggregate requests awaiting responses.
/// - `routed`: A reference to the `RoutedMap`, which tracks routed requests awaiting responses.
/// - `client_id`: The identifier of the client that sent the request.
/// - `internal_api_map`: A reference to the internal API map, which contains predefined API responses.
/// - `service_rules`: A map of service routing rules used to route client requests.
///
async fn process_client_request(
    v: &Value,
    clients: &Clients,
    pending: &PendingMap,
    routed: &RoutedMap,
    client_id: &str,
    internal_api_map: Arc<HashMap<String, Value>>,
    service_rules: &HashMap<String, String>,
) {
    if v["method"] == "register" {
        handle_service_register_request(v, clients, client_id).await;
    } else if v["method"] == "unregister" {
        handle_service_unregister_request(v, clients, client_id).await;
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
/// Handles a service register request by associating a client with a service ID.
///
/// This function allows a client to register as a service by associating it with a `service_id`.
/// If the `service_id` is already in use by another client, the old client is removed, and the
/// new client is registered with the same `service_id`.
///
/// # Parameters
/// - `v`: The JSON-RPC request containing the `service_id` to register.
/// - `clients`: A reference to the `Clients` map, which stores information about connected clients.
/// - `client_id`: The identifier of the client requesting to register.
///
/// # Behavior
/// - If the `service_id` is already in use:
///   - The old client associated with the `service_id` is removed from the `clients` map.
///   - The old client's connection is cleaned up.
/// - The new client is registered with the `service_id` and marked as a service.
async fn handle_service_register_request(v: &Value, clients: &Clients, client_id: &str) {
    if let Some(sid) = v["params"]["service_id"].as_str() {
        let (response_value, old_client_tx) = {
            let mut map = clients.lock().await;

            // Check if the `service_id` is already in use
            let old_client = map
                .iter()
                .find(|(_, c)| c.service_id.as_deref() == Some(sid))
                .map(|(id, _)| id.clone());

            let old_client_tx = old_client
                .as_ref()
                .and_then(|id| map.get(id).map(|c| c.tx.clone()));

            if let Some(ref old_client_id) = old_client {
                println!(
                    "[AppGW] Service ID {} is already in use by client {}. Cleaning up old client.",
                    sid, old_client_id
                );
                map.remove(old_client_id);
            }

            // Register the new client with the `service_id`
            let response_value = if let Some(client_info) = map.get_mut(client_id) {
                client_info.service_id = Some(sid.to_string());
                client_info.is_service = true;
                println!("[AppGW] Client {} registered as service {}", client_id, sid);
                json!({
                    "result": {
                        "status": "success"
                    }
                })
            } else {
                println!("[AppGW] Client {} not found in map", client_id);
                json!({
                    "error": {
                        "code": -32602,
                        "message": "Invalid service_id provided"
                    }
                })
            };
            (response_value, old_client_tx)
        };

        // Now, outside the lock, optionally send a disconnect message to the old client
        if let Some(tx) = old_client_tx {
            let _ = tx.send(Message::Close(None)).await;
        }

        send_response(v, response_value, clients, client_id).await;
    } else {
        println!("[AppGW] Invalid register request: no service_id provided");
        let error_response_value = json!({
            "error": {
                "code": -32602,
                "message": "Invalid params: no service_id provided"
            }
        });
        send_response(v, error_response_value, clients, client_id).await;
    }
}
/// Handles a service unregister request by updating the client's service information.
///
/// This function updates the `service_id` of the client to `None` and sets `is_service` to `false`,
/// effectively unregistering the client as a service without removing it from the `clients` map.
///
/// # Parameters
/// - `clients`: A reference to the `Clients` map, which stores information about connected clients.
/// - `client_id`: The identifier of the client requesting to unregister.
async fn handle_service_unregister_request(v: &Value, clients: &Clients, client_id: &str) {
    println!("[AppGW] Client {} requested unregister", client_id);

    // Gather response while holding the lock, but call send_response after releasing it
    let response_value = {
        let mut map = clients.lock().await;
        if let Some(client_info) = map.get_mut(client_id) {
            client_info.service_id = None;
            client_info.is_service = false;
            println!("[AppGW] Client {} unregistered as a service", client_id);
            json!({
                "result": {
                    "status": "success"
                }
            })
        } else {
            println!("[AppGW] Client {} not found in map", client_id);
            json!({
                "error": {
                    "code": -32602,
                    "message": "Invalid client_id provided"
                }
            })
        }
    };
    // Lock is dropped here before calling send_response
    send_response(v, response_value, clients, client_id).await;
}
/// Handles a service response for either an aggregate or direct request.
///
/// This function determines whether the response is part of an aggregate request or a direct request.
/// It processes the response accordingly by calling `process_aggregate_response` for aggregate requests
/// or `process_direct_response` for direct requests.
///
/// # Parameters
/// - `v`: The JSON-RPC response from the service, containing either a `result` or an `error`.
/// - `clients`: A reference to the `Clients` map, which stores information about connected clients and services.
/// - `pending`: A reference to the `PendingMap`, which tracks aggregate requests awaiting responses from multiple services.
/// - `routed`: A reference to the `RoutedMap`, which tracks routed requests awaiting responses from specific services.
/// - `client_id`: The identifier of the client that initiated the request.
///
async fn handle_service_response(
    v: &Value,
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
/// Processes a response from a service as part of an aggregate request.
///
/// This function is responsible for tracking responses from multiple services
/// involved in an aggregate request. It updates the tracker with the received
/// response, checks if all expected responses have been received, and sends the
/// final aggregated response back to the client if complete.
///
/// # Parameters
/// - `v`: The JSON-RPC response from a service, containing either a `result` or an `error`.
/// - `clients`: A reference to the `Clients` map, which stores information about connected clients and services.
/// - `tracker`: A mutable reference to the `PendingAggregate` tracker, which keeps track of the responses received so far.
/// - `client_id`: The identifier of the client that initiated the aggregate request.
///
async fn process_aggregate_response(
    v: &Value,
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

        // Construct the final response message
        let final_msg = json!({
            "jsonrpc": "2.0",
            "id": tracker.original_id,
            "result": result
        });

        // Send the final response message to the client
        let _ = tracker
            .sender_tx
            .send(Message::Text(final_msg.to_string()))
            .await;
    }
}
/// Processes a direct response from a service for a routed request.
///
/// This function handles the response from a service for a direct or routed request.
/// It wraps the response in JSON-RPC format and sends it back to the client that initiated the request.
///
/// # Parameters
/// - `v`: The JSON-RPC response from the service, containing either a `result` or an `error`.
/// - `clients`: A reference to the `Clients` map, which stores information about connected clients and services.
/// - `routed_req`: The `RoutedRequest` object containing details about the routed request, such as the original request ID and the sender's transmitter.
/// - `client_id`: The identifier of the client that initiated the request.
///
async fn process_direct_response(
    v: &Value,
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
            "result": v["result"]
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
