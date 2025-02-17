#![cfg(test)]
use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use ripple_sdk::log::error;
use ripple_sdk::tokio::{
    self,
    net::{TcpListener, TcpStream},
    sync::{
        mpsc::{self, Sender},
        Mutex,
    },
    time::sleep,
};
use serde::Serialize;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio_tungstenite::{accept_async, tungstenite::protocol::Message};

use ripple_sdk::api::gateway::rpc_gateway_api::{JsonRpcApiRequest, JsonRpcApiResponse};
const CONTROLLER_STATUS_METHOD: &str = "Controller.1.status@";
const CONTROLLER_ACTIVATE_METHOD: &str = "Controller.1.activate";
const CONTROLLER_REGISTER_METHOD: &str = "Controller.1.register";
const CONTROLLER_UNREGISTER_METHOD: &str = "Controller.1.unregister";

#[derive(Serialize)]
struct StateChangeEventData {
    callsign: String,
    state: String,
}
type JsonRpcResponseWithOptionalEvent = (JsonRpcApiResponse, Option<(JsonRpcApiResponse, u64)>);
type ThunderResponseList = Arc<Mutex<HashMap<String, JsonRpcResponseWithOptionalEvent>>>;

pub struct ServerHandle {
    stop_sender: Sender<()>,
    address: SocketAddr,
    //canned_responses: Arc<Mutex<HashMap<String, (JsonRpcApiResponse, Option<(JsonRpcApiResponse, u64)>)>>>,
}
pub struct MockThunderLiteServer {
    address: String,
    canned_responses: ThunderResponseList,
    stop_sender: Option<Sender<()>>,
}

impl MockThunderLiteServer {
    pub async fn new() -> Self {
        let port = find_available_port().await;
        let address = format!("127.0.0.1:{}", port);
        let canned_responses = Arc::new(Mutex::new(predefined_mock_thunder_responses()));
        Self {
            address,
            canned_responses,
            stop_sender: None,
        }
    }
    pub async fn with_mock_thunder_response_for_alias(
        self,
        method: &str,
        result: Option<serde_json::Value>,
        error: Option<serde_json::Value>,
        event: Option<(JsonRpcApiResponse, u64)>,
    ) -> Self {
        let response = JsonRpcApiResponse {
            jsonrpc: "2.0".to_string(),
            result,
            error,
            id: None,
            method: None,
            params: None,
        };

        // Clone the responses and insert the new response
        let canned_responses = self.canned_responses.clone();
        {
            let mut responses = canned_responses.lock().await;
            responses.insert(method.to_string(), (response, event));
        }

        self
    }
    pub async fn start(mut self) -> ServerHandle {
        let address = self.address.parse().unwrap();
        let listener = TcpListener::bind(address).await.unwrap();
        let (stop_sender, mut stop_receiver) = mpsc::channel(1);
        self.stop_sender = Some(stop_sender.clone());

        let canned_responses = self.canned_responses.clone();
        let server_handle = ServerHandle {
            stop_sender,
            address,
            //canned_responses: canned_responses.clone(),
        };

        tokio::spawn(async move {
            println!("WebSocket Server running on {}", address);
            loop {
                tokio::select! {
                    _ = stop_receiver.recv() => {
                        println!("Stopping WebSocket server...");
                        break;
                    }
                    Ok((stream, _)) = listener.accept() => {
                        let canned_responses = canned_responses.clone();
                        tokio::spawn(handle_connection(stream, canned_responses));
                    }
                }
            }
        });

        server_handle
    }
}

impl ServerHandle {
    pub fn get_address(&self) -> String {
        format!("ws://{}/jsonrpc", self.address)
    }

    pub async fn stop(&self) {
        let _ = self.stop_sender.send(()).await;
    }
}

async fn handle_connection(stream: TcpStream, canned_responses: ThunderResponseList) {
    match accept_async(stream).await {
        Ok(websocket) => {
            println!("WebSocket connection established.");
            let (ws_sender, mut ws_receiver) = websocket.split();
            let ws_sender = Arc::new(Mutex::new(ws_sender));

            while let Some(Ok(message)) = ws_receiver.next().await {
                if let Message::Text(text) = message {
                    println!("Received request: {}", text);

                    let req_json: JsonRpcApiRequest = match serde_json::from_str(&text) {
                        Ok(req) => req,
                        Err(_) => {
                            println!("Invalid JSON request: {}", text);
                            continue;
                        }
                    };

                    let response_option = {
                        let responses = canned_responses.lock().await;
                        if req_json.method.starts_with(CONTROLLER_STATUS_METHOD) {
                            let callsign = req_json.method.split('@').collect::<Vec<&str>>()[1];
                            let method = CONTROLLER_STATUS_METHOD.to_string();
                            let mut resp = responses.get(&method).cloned();
                            // add call sign to the response
                            if let Some((response, _)) = resp.as_mut() {
                                if let Some(serde_json::Value::Array(array)) =
                                    response.result.as_mut()
                                {
                                    if let Some(serde_json::Value::Object(obj)) = array.get_mut(0) {
                                        obj.insert(
                                            "callsign".to_string(),
                                            serde_json::Value::String(callsign.to_string()),
                                        );
                                    }
                                }
                            }
                            resp
                        } else if req_json.method == CONTROLLER_ACTIVATE_METHOD {
                            let method = "thunder.Broker.Controller.events.statechange".to_string();
                            let event_data = StateChangeEventData {
                                callsign: req_json
                                    .params
                                    .as_ref()
                                    .and_then(|params| params.as_object())
                                    .and_then(|obj| obj.get("callsign"))
                                    .and_then(|value| value.as_str())
                                    .map(|s| s.to_string())
                                    .unwrap_or_else(|| {
                                        error!("Failed to extract callsign from request JSON");
                                        String::new()
                                    }),
                                state: "activated".to_string(),
                            };
                            let event_response = JsonRpcApiResponse {
                                jsonrpc: "2.0".to_string(),
                                result: Some(serde_json::Value::Null),
                                error: None,
                                id: None,
                                method: Some(method),
                                params: Some(serde_json::to_value(event_data).unwrap()),
                            };
                            // return the tuple with None event response
                            Some((event_response, None))
                        } else {
                            responses.get(&req_json.method).cloned()
                        }
                    };

                    if let Some((mut response, event)) = response_option {
                        response.id = req_json.id;
                        let response_json = serde_json::to_string(&response).unwrap();
                        let sender_clone = Arc::clone(&ws_sender);

                        // Send response in a seperate task
                        tokio::spawn(async move {
                            let mut sender = sender_clone.lock().await;
                            println!("Sending response: {:?}", response_json);
                            let _ = sender.send(Message::Text(response_json)).await;
                        });

                        if let Some((mut event_response, delay)) = event {
                            if event_response.id.is_none() {
                                event_response.id = response.id;
                            }
                            let sender_clone = Arc::clone(&ws_sender);

                            // Send event after delay
                            tokio::spawn(async move {
                                sleep(Duration::from_millis(delay)).await;
                                let event_json = serde_json::to_string(&event_response).unwrap();
                                let mut sender = sender_clone.lock().await;
                                println!("Sending Event response: {:?}", event_json);
                                let _ = sender.send(Message::Text(event_json)).await;
                            });
                        }
                    }
                }
            }
        }
        Err(e) => println!("WebSocket handshake failed: {}", e),
    }
}

fn predefined_mock_thunder_responses() -> HashMap<String, JsonRpcResponseWithOptionalEvent> {
    let mut responses = HashMap::new();
    // id has been defined as None in the canned response table, but it will be replaced with the actual id
    // from the request when sending the response
    responses.insert(
        CONTROLLER_ACTIVATE_METHOD.to_string(),
        (
            JsonRpcApiResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(serde_json::Value::Null),
                error: None,
                id: None,
                method: None,
                params: None,
            },
            None,
        ),
    );
    responses.insert(
        CONTROLLER_STATUS_METHOD.to_string(),
        (
            JsonRpcApiResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(serde_json::json!([{"state": "activated"}])),
                error: None,
                id: None,
                method: None,
                params: None,
            },
            None,
        ),
    );
    responses.insert(
        CONTROLLER_REGISTER_METHOD.to_string(),
        (
            JsonRpcApiResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(serde_json::json!({ "message": "Registered successfully" })),
                error: None,
                id: None,
                method: None,
                params: None,
            },
            None,
        ),
    );
    responses.insert(
        CONTROLLER_UNREGISTER_METHOD.to_string(),
        (
            JsonRpcApiResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(serde_json::json!({ "message": "Unregistered successfully" })),
                error: None,
                id: None,
                method: None,
                params: None,
            },
            None,
        ),
    );
    responses
}

async fn find_available_port() -> u16 {
    loop {
        let port: u16 = rand::thread_rng().gen_range(3000..9000);
        if TcpListener::bind(format!("127.0.0.1:{}", port))
            .await
            .is_ok()
        {
            return port;
        }
    }
}
