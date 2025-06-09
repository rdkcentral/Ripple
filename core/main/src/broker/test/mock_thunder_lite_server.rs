use futures_util::{SinkExt, StreamExt};
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
use ripple_sdk::tokio_tungstenite::{accept_async, tungstenite::protocol::Message};
use serde::Serialize;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

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
    canned_responses: ThunderResponseList,
    stop_sender: Option<Sender<()>>,
}

impl MockThunderLiteServer {
    pub async fn new() -> Self {
        let canned_responses = Arc::new(Mutex::new(predefined_mock_thunder_responses()));
        Self {
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
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (stop_sender, mut stop_receiver) = mpsc::channel(1);
        self.stop_sender = Some(stop_sender.clone());

        let canned_responses = self.canned_responses.clone();
        let server_handle = ServerHandle {
            stop_sender,
            address,
            //canned_responses: canned_responses.clone(),
        };

        tokio::spawn(async move {
            println!(
                "[ThunderLite Server] WebSocket Server running on {}",
                address
            );
            loop {
                tokio::select! {
                    _ = stop_receiver.recv() => {
                        println!("[ThunderLite Server] Stopping WebSocket server...");
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

#[macro_export]
macro_rules! send_response {
    ($ws_sender:expr, $response_json:expr) => {
        let sender_clone = Arc::clone(&$ws_sender);
        tokio::spawn(async move {
            let mut sender = sender_clone.lock().await;
            println!("Sending response: {:?}", $response_json);
            let _ = sender.send(Message::Text($response_json)).await;
        });
    };
}

#[macro_export]
macro_rules! send_event_response {
    ($ws_sender:expr, $event_response:expr, $delay:expr) => {
        let sender_clone = Arc::clone(&$ws_sender);
        tokio::spawn(async move {
            sleep(Duration::from_millis($delay)).await;
            let event_json = serde_json::to_string(&$event_response).unwrap();
            let mut sender = sender_clone.lock().await;
            println!("Sending Event response: {:?}", event_json);
            let _ = sender.send(Message::Text(event_json)).await;
        });
    };
}

#[macro_export]
macro_rules! extract_callsign {
    ($req_json:expr) => {
        $req_json
            .params
            .as_ref()
            .and_then(|params| params.as_object())
            .and_then(|obj| obj.get("callsign"))
            .and_then(|value| value.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                error!("Failed to extract callsign from request JSON");
                String::new()
            })
    };
}
#[macro_export]
macro_rules! add_callsign_to_response {
    ($response:expr, $callsign:expr) => {
        if let Some(serde_json::Value::Array(array)) = $response.result.as_mut() {
            if let Some(serde_json::Value::Object(obj)) = array.get_mut(0) {
                obj.insert(
                    "callsign".to_string(),
                    serde_json::Value::String($callsign.to_string()),
                );
            }
        }
    };
}

fn create_state_change_event_response(req_json: &JsonRpcApiRequest) -> JsonRpcApiResponse {
    let method = "thunder.Broker.Controller.events.statechange".to_string();
    let event_data = StateChangeEventData {
        callsign: extract_callsign!(req_json),
        state: "Activated".to_string(),
    };
    JsonRpcApiResponse {
        jsonrpc: "2.0".to_string(),
        result: Some(serde_json::Value::Null),
        error: None,
        id: None,
        method: Some(method),
        params: Some(serde_json::to_value(event_data).unwrap()),
    }
}

async fn handle_connection(stream: TcpStream, canned_responses: ThunderResponseList) {
    match accept_async(stream).await {
        Ok(websocket) => {
            println!("[ThunderLite Server] WebSocket connection established.");
            let (ws_sender, mut ws_receiver) = websocket.split();
            let ws_sender = Arc::new(Mutex::new(ws_sender));

            while let Some(Ok(message)) = ws_receiver.next().await {
                if let Message::Text(text) = message {
                    println!("[ThunderLite Server] Received request: {}", text);

                    let req_json: JsonRpcApiRequest = match serde_json::from_str(&text) {
                        Ok(req) => req,
                        Err(_) => {
                            println!("[ThunderLite Server] Invalid JSON request: {}", text);
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
                                add_callsign_to_response!(response, callsign);
                            }
                            resp
                        } else if req_json.method == CONTROLLER_ACTIVATE_METHOD {
                            let event_response = create_state_change_event_response(&req_json);
                            // return the tuple with event_response as JsonRpcApiResponse and None as any specific event.
                            Some((event_response, None))
                        } else {
                            responses.get(&req_json.method).cloned()
                        }
                    };

                    if let Some((mut response, event)) = response_option {
                        response.id = req_json.id;
                        let response_json = serde_json::to_string(&response).unwrap();
                        send_response!(ws_sender, response_json);

                        if let Some((mut event_response, delay)) = event {
                            if event_response.id.is_none() {
                                event_response.id = response.id;
                            }
                            send_event_response!(ws_sender, event_response, delay);
                        }
                    }
                }
            }
        }
        Err(e) => println!("[ThunderLite Server] WebSocket handshake failed: {}", e),
    }
}

#[macro_export]
macro_rules! insert_response {
    ($responses:expr, $method:expr, $result:expr) => {
        $responses.insert(
            $method.to_string(),
            (
                JsonRpcApiResponse {
                    jsonrpc: "2.0".to_string(),
                    result: Some($result),
                    error: None,
                    id: None,
                    method: None,
                    params: None,
                },
                None,
            ),
        );
    };
}

fn predefined_mock_thunder_responses() -> HashMap<String, JsonRpcResponseWithOptionalEvent> {
    let mut responses = HashMap::new();

    // id has been defined as None in the canned response table, but it will be replaced with the actual id
    // from the request when sending the response
    insert_response!(
        responses,
        CONTROLLER_ACTIVATE_METHOD,
        serde_json::Value::Null
    );
    insert_response!(
        responses,
        CONTROLLER_STATUS_METHOD,
        serde_json::json!([{"state": "activated"}])
    );
    insert_response!(
        responses,
        CONTROLLER_REGISTER_METHOD,
        serde_json::json!({ "message": "Registered successfully" })
    );
    insert_response!(
        responses,
        CONTROLLER_UNREGISTER_METHOD,
        serde_json::json!({ "message": "Unregistered successfully" })
    );

    responses
}

#[macro_export]
macro_rules! setup_and_start_mock_thunder_lite_server {
    ($($method:expr, $result:expr, $error:expr, $event:expr),* $(,)?) => {{
        let mock_thunder_lite_server = MockThunderLiteServer::new().await;
        $(
            let mock_thunder_lite_server = mock_thunder_lite_server.with_mock_thunder_response_for_alias(
                $method,
                $result,
                $error,
                $event,
            ).await;
        )*
        let handle = mock_thunder_lite_server.start().await;
        handle
    }};
}

#[macro_export]
macro_rules! create_and_send_broker_request {
    ($broker:expr, $method:expr, $alias:expr, $call_id:expr, $params:expr) => {
        let mut request = create_mock_broker_request($method, $alias, $params, None, None, None);
        request.rpc.ctx.call_id = $call_id;
        let response = $broker.sender.send(request).await;
        assert!(response.is_ok());
    };
}

#[macro_export]
macro_rules! create_and_send_broker_request_with_jq_transform {
    ($thunder_broker:expr, $method:expr, $alias:expr, $call_id:expr, $params:expr, $transform:expr, $event_filter:expr, $event_handler_fn:expr) => {{
        let mut request = create_mock_broker_request(
            $method,
            $alias,
            $params,
            $transform,
            $event_filter,
            $event_handler_fn,
        );

        request.rpc.ctx.call_id = $call_id;

        let response = $thunder_broker.sender.send(request).await;
        assert!(response.is_ok());
    }};
}

#[macro_export]
macro_rules! read_broker_responses {
    ($receiver:expr, $expected_count:expr) => {
        let mut counter = 0;
        loop {
            let v = tokio::time::timeout(Duration::from_secs(2), $receiver.recv()).await;
            if v.is_err() {
                break;
            }
            counter += 1;
            println!("[Broker Output] {:?}", v);
        }
        assert_eq!(counter, $expected_count);
    };
}

#[macro_export]
macro_rules! process_broker_output {
    ($broker_request:expr, $broker_output:expr) => {{
        let output = $broker_output.unwrap();
        let mut response = output.data.clone();

        // Apply the jq transform to the response
        let rule_context_name = $broker_request.clone().rpc.method.clone();

        if let Some(filter) = $broker_request
            .clone()
            .rule
            .transform
            .get_transform_data(ripple_sdk::api::rules_engine::RuleTransformType::Response)
        {
            apply_response(filter, &rule_context_name, &mut response);
        } else if response.result.is_none() && response.error.is_none() {
            response.result = Some(Value::Null);
        }

        println!(
            "[Broker Output] Final output After applying Rule {:?}",
            response
        );
    }};
}

#[macro_export]
macro_rules! process_broker_output_event_resposne {
    ($broker_request:expr, $broker_output:expr, $value:expr) => {{
        let output = $broker_output.unwrap();
        let mut response = output.data.clone();

        // Apply the jq transform to the response

        if let Some(filter) = $broker_request.clone().rule.transform.get_transform_data(
            ripple_sdk::api::rules_engine::RuleTransformType::Event(false),
        ) {
            let broker_request_clone = $broker_request.clone();
            let result = response.clone().result.unwrap();
            let rpc = $broker_request.rpc.clone();
            apply_rule_for_event(&broker_request_clone, &result, &rpc, &filter, &mut response);
        } else if response.result.is_none() && response.error.is_none() {
            response.result = Some(Value::Null);
        }
        println!(
            "[Broker Output] Final Event Resposne After applying Rule {:?}",
            response.result
        );
        assert_eq!(response.result, $value);
    }};
}
