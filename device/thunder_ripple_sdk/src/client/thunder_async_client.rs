// Copyright 2023 Comcast Cable Communications Management, LLC
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
    device_operator::{DeviceChannelParams, DeviceChannelRequest, DeviceResponseMessage},
    thunder_async_client_plugins_status_mgr::{AsyncCallback, AsyncSender, StatusManager},
};
use crate::utils::get_next_id;
use futures::{stream::SplitSink, SinkExt, StreamExt};
//use futures_util::{SinkExt, StreamExt};
use ripple_sdk::tokio_tungstenite::{tungstenite::Message, WebSocketStream};
use ripple_sdk::{
    api::gateway::rpc_gateway_api::{JsonRpcApiRequest, JsonRpcApiResponse},
    log::{debug, error, info},
    tokio::{self, net::TcpStream, sync::mpsc::Receiver},
    utils::{error::RippleError, ws_utils::WebSocketUtils},
};
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct ThunderAsyncClient {
    status_manager: StatusManager,
    sender: AsyncSender,
    callback: AsyncCallback,
    subscriptions: HashMap<String, JsonRpcApiRequest>,
}

#[derive(Clone, Debug)]
pub struct ThunderAsyncRequest {
    pub id: u64,
    pub request: DeviceChannelRequest,
}

impl std::fmt::Display for ThunderAsyncRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ThunderAsyncRequest {{ id: {}, request: {:?} }}",
            self.id, self.request
        )
    }
}

impl ThunderAsyncRequest {
    pub fn new(request: DeviceChannelRequest) -> Self {
        Self {
            id: get_next_id(),
            request,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ThunderAsyncResponse {
    pub id: Option<u64>,
    pub result: Result<JsonRpcApiResponse, RippleError>,
}

impl ThunderAsyncClient {}

impl ThunderAsyncResponse {
    fn new_response(response: JsonRpcApiResponse) -> Self {
        Self {
            id: response.id,
            result: Ok(response),
        }
    }

    pub fn new_error(id: u64, e: RippleError) -> Self {
        let error_response = JsonRpcApiResponse {
            id: Some(id),
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(serde_json::json!({"code":-32100,"message":e.to_string()})),
            method: None,
            params: None,
        };
        Self {
            id: Some(id),
            result: Ok(error_response),
        }
    }

    pub fn get_method(&self) -> Option<String> {
        if let Ok(e) = &self.result {
            return e.method.clone();
        }
        None
    }

    pub fn get_id(&self) -> Option<u64> {
        println!("@@@NNA...get_id from ThunderAsyncResponse :{:?}", self);
        match &self.result {
            Ok(response) => response.id,
            Err(_) => None,
        }
    }

    pub fn get_device_resp_msg(&self, sub_id: Option<String>) -> Option<DeviceResponseMessage> {
        let json_resp = match &self.result {
            Ok(json_resp_res) => json_resp_res,
            _ => return None,
        };
        DeviceResponseMessage::create(json_resp, sub_id)
    }
}

impl ThunderAsyncClient {
    pub fn get_sender(&self) -> AsyncSender {
        self.sender.clone()
    }

    pub fn get_callback(&self) -> AsyncCallback {
        self.callback.clone()
    }

    fn check_plugin_status_n_prepare_request(
        &self,
        request: &ThunderAsyncRequest,
    ) -> Result<String, RippleError> {
        let id: u64 = request.id;
        let (callsign, method) = request.request.get_callsign_method();
        // Check if the method is empty and return an error if it is
        if method.is_empty() {
            return Err(RippleError::InvalidInput);
        }
        // Check the status of the plugin using the status manager
        let status = match self.status_manager.get_status(callsign.clone()) {
            Some(v) => v.clone(),
            None => {
                // If the plugin status is not available, add the request to the pending list
                self.status_manager
                    .add_async_client_request_to_pending_list(callsign.clone(), request.clone());
                // Generate a request to check the plugin status and add it to the requests list
                let request = self
                    .status_manager
                    .generate_plugin_status_request(Some(callsign.clone()));
                return Ok(request.to_string());
            }
        };
        // If the plugin is missing, return a service error
        if status.state.is_missing() {
            error!("Plugin {} is missing", callsign);
            return Err(RippleError::ServiceError);
        }
        // If the plugin is activating, return a service not ready error
        if status.state.is_activating() {
            info!(
                "Plugin {} is activating. Adding broker request to pending list",
                callsign
            );
            self.status_manager
                .add_async_client_request_to_pending_list(callsign.clone(), request.clone());
            return Err(RippleError::ServiceNotReady);
        }
        // If the plugin is not activated, add the request to the pending list and generate an activation request
        if !status.state.is_activated() {
            self.status_manager
                .add_async_client_request_to_pending_list(callsign.clone(), request.clone());
            let request = self
                .status_manager
                .generate_plugin_activation_request(callsign.clone());
            return Ok(request.to_string());
        }

        // Generate the appropriate JSON-RPC request based on the type of DeviceChannelRequest
        let json_rpc_api_request = match &request.request {
            DeviceChannelRequest::Call(device_call_request) => {
                let mut params_value = None;
                if let Some(device_channel_params) = &device_call_request.params {
                    match device_channel_params {
                        DeviceChannelParams::Json(params_str) => {
                            params_value = serde_json::from_str::<Value>(params_str).ok();
                        }
                        DeviceChannelParams::Bool(params_bool) => {
                            params_value = Some(Value::Bool(*params_bool));
                        }
                        DeviceChannelParams::Literal(params_literal) => {
                            params_value = Some(Value::String(params_literal.clone()));
                        }
                    };
                }

                JsonRpcApiRequest::new(device_call_request.method.clone(), params_value).with_id(id)
            }
            DeviceChannelRequest::Unsubscribe(_) => JsonRpcApiRequest::new(
                format!("{}.unregister", callsign),
                Some(json!({
                    "event": method,
                    "id": "client.events"
                })),
            )
            .with_id(id),
            DeviceChannelRequest::Subscribe(_) => JsonRpcApiRequest::new(
                format!("{}.register", callsign),
                Some(json!({
                    "event": method,
                    "id": "client.events"
                })),
            )
            .with_id(id),
        };

        serde_json::to_string(&json_rpc_api_request).map_err(|_e| RippleError::ParseError)
    }

    pub fn new(callback: AsyncCallback, sender: AsyncSender) -> Self {
        Self {
            status_manager: StatusManager::new(),
            sender,
            callback,
            subscriptions: HashMap::new(),
        }
    }

    async fn handle_response(&mut self, message: Message) {
        if let Message::Text(t) = message {
            debug!("thunder_async_response: {}", t);
            let request = t.as_bytes();

            //check controller response or not
            if self
                .status_manager
                .is_controller_response(self.get_sender(), self.callback.clone(), request)
                .await
            {
                self.status_manager
                    .handle_controller_response(self.get_sender(), self.callback.clone(), request)
                    .await;
            } else {
                self.handle_jsonrpc_response(request).await
            }
        }
    }

    async fn process_subscribe_requests(
        &mut self,
        ws_tx: &mut SplitSink<WebSocketStream<TcpStream>, Message>,
    ) {
        for (_, subscription_request) in self.subscriptions.iter_mut() {
            let new_id = get_next_id();

            debug!(
                "process_subscribe_requests: method={}, params={:?}, old_id={:?}, new_id={}",
                subscription_request.method,
                subscription_request.params,
                subscription_request.id,
                new_id
            );

            subscription_request.id = Some(new_id);

            let request_json = serde_json::to_string(&subscription_request).unwrap();
            let _feed = ws_tx.feed(Message::Text(request_json)).await;
        }
    }

    pub async fn start(
        &mut self,
        url: &str,
        mut thunder_async_request_rx: Receiver<ThunderAsyncRequest>,
        status_check: bool,
    ) {
        loop {
            info!("start: (re)establishing websocket connection: url={}", url);
            let resp = WebSocketUtils::get_ws_stream(url, None).await;
            if resp.is_err() {
                error!("FATAL ERROR Thunder URL badly configured.");
                break;
            }
            let (mut thunder_tx, mut thunder_rx) = resp.unwrap();

            // send the controller statechange subscription request
            let status_request = self
                .status_manager
                .generate_state_change_subscribe_request();

            let _feed = thunder_tx
                .feed(Message::Text(status_request.to_string()))
                .await;

            self.process_subscribe_requests(&mut thunder_tx).await;

            let _flush = thunder_tx.flush().await;

            // Check if the plugin status check is enabled
            if status_check {
                debug!("thunder plugin status check at thunder async client startup");
                //send thunder plugin status check request for all plugins
                let status_check_request = self.status_manager.generate_plugin_status_request(None);
                {
                    // let mut ws_tx = ws_tx_wrap.lock().await;
                    let _feed = thunder_tx
                        .feed(Message::Text(status_check_request.to_string()))
                        .await;
                    let _flush = thunder_tx.flush().await;
                }
            } else {
                debug!("thunder plugin status check at thunder async client startup is disabled");
            }

            tokio::pin! {
                let subscriptions_socket = thunder_rx.next();
            }

            loop {
                tokio::select! {
                    Some(value) = &mut subscriptions_socket => {
                        match value {
                            Ok(message) => {
                                self.handle_response(message).await;
                            },
                            Err(e) => {
                                error!("Thunder_async_client Websocket error on read {:?}", e);
                                break;
                            }
                        }
                    },
                    Some(request) = thunder_async_request_rx.recv() => {
                        match self.check_plugin_status_n_prepare_request(&request) {
                            Ok(updated_request) => {
                                if let Ok(jsonrpc_request) = serde_json::from_str::<JsonRpcApiRequest>(&updated_request) {
                                    if jsonrpc_request.method.ends_with(".register") {
                                        if let Some(Value::Object(ref params)) = jsonrpc_request.params {
                                            if let Some(Value::String(event)) = params.get("event") {
                                                debug!("thunder_async_request_rx: Rerouting subscription request for {}", event);

                                                // Store the subscription request in the subscriptions list in case we need to
                                                // resubscribe later due to a socket disconnect.
                                                self.subscriptions.insert(event.to_string(), jsonrpc_request.clone());
                                                debug!("thunder_async_request_rx: subscription request={}", updated_request);
                                                // Reroute subsubscription requests through the persistent websocket so all notifications
                                                // are sent to the same websocket connection.
                                                let _feed = thunder_tx.feed(Message::Text(updated_request)).await;
                                                let _flush = thunder_tx.flush().await;
                                            } else {
                                                error!("thunder_async_request_rx: Missing 'event' parameter");
                                            }
                                        } else {
                                            error!("thunder_async_request_rx: Missing 'params' object");
                                        }
                                    }
                                    else {
                                        debug!("thunder_async_request_rx: call request={}", updated_request);
                                        let _feed = thunder_tx.feed(Message::Text(updated_request)).await;
                                        let _flush = thunder_tx.flush().await;
                                    }
                                }
                            }
                            Err(e) => {
                                match e {
                                    RippleError::ServiceNotReady => {
                                        info!("Thunder Service not ready, request is now in pending list {:?}", request);
                                    },
                                    _ => {
                                        error!("error preparing request {:?}", e);
                                        let response = ThunderAsyncResponse::new_error(request.id,e.clone());
                                        self.callback.send(response).await;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    async fn handle_jsonrpc_response(&mut self, result: &[u8]) {
        if let Ok(message) = serde_json::from_slice::<JsonRpcApiResponse>(result) {
            self.callback
                .send(ThunderAsyncResponse::new_response(message))
                .await
        } else {
            error!("handle_jsonrpc_response: Invalid JSON RPC message sent by Thunder");
        }
    }

    pub async fn send(&self, request: ThunderAsyncRequest) {
        if let Err(e) = self.sender.send(request).await {
            error!("Failed to send thunder Async Request: {:?}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::device_operator::DeviceCallRequest;
    use ripple_sdk::api::gateway::rpc_gateway_api::JsonRpcApiResponse;
    use ripple_sdk::utils::error::RippleError;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_thunder_async_request_new() {
        let callrequest = DeviceCallRequest {
            method: "org.rdk.System.1.getSerialNumber".to_string(),
            params: None,
        };

        let request = DeviceChannelRequest::Call(callrequest);
        let _async_request = ThunderAsyncRequest::new(request.clone());
        assert_eq!(
            _async_request.request.get_callsign_method(),
            request.get_callsign_method()
        );
    }

    #[tokio::test]
    async fn test_thunder_async_response_new_response() {
        let response = JsonRpcApiResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(6),
            result: Some(json!({"key": "value"})),
            error: None,
            method: None,
            params: None,
        };

        let _async_response = ThunderAsyncResponse::new_response(response.clone());
        assert_eq!(_async_response.result.unwrap().result, response.result);
    }

    #[tokio::test]
    async fn test_thunder_async_response_new_error() {
        let error = RippleError::ServiceError;
        let async_response = ThunderAsyncResponse::new_error(1, error.clone());
        assert_eq!(
            async_response.result.unwrap().error,
            Some(json!({"code":-32100,"message":error.to_string()}))
        );
    }

    #[tokio::test]
    async fn test_thunder_async_response_get_event() {
        let response = JsonRpcApiResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(6),
            result: Some(json!({"key": "value"})),
            error: None,
            method: Some("event_1".to_string()),
            params: None,
        };
        let async_response = ThunderAsyncResponse::new_response(response);
        assert_eq!(async_response.get_method(), Some("event_1".to_string()));
    }

    #[tokio::test]
    async fn test_thunder_async_response_get_id() {
        let response = JsonRpcApiResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(42),
            result: Some(json!({"key": "value"})),
            error: None,
            method: Some("event_1".to_string()),
            params: None,
        };
        let async_response = ThunderAsyncResponse::new_response(response);
        assert_eq!(async_response.get_id(), Some(42));
    }

    #[tokio::test]
    async fn test_thunder_async_response_get_device_resp_msg() {
        let response = JsonRpcApiResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(6),
            result: Some(json!({"key": "value"})),
            error: None,
            method: Some("event_1".to_string()),
            params: None,
        };
        let async_response = ThunderAsyncResponse::new_response(response);
        let device_resp_msg = async_response.get_device_resp_msg(None);
        assert_eq!(device_resp_msg.unwrap().message, json!({"key": "value"}));
    }

    #[tokio::test]
    async fn test_thunder_async_client_prepare_request() {
        let (resp_tx, _resp_rx) = mpsc::channel(10);
        let callback = AsyncCallback { sender: resp_tx };
        let (async_tx, _async_rx) = mpsc::channel(10);
        let async_sender = AsyncSender { sender: async_tx };
        let client = ThunderAsyncClient::new(callback, async_sender);

        let callrequest = DeviceCallRequest {
            method: "org.rdk.System.1.getSerialNumber".to_string(),
            params: None,
        };

        let request = DeviceChannelRequest::Call(callrequest);
        let async_request = ThunderAsyncRequest::new(request);
        let result = client.check_plugin_status_n_prepare_request(&async_request);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_thunder_async_client_send() {
        let (resp_tx, _resp_rx) = mpsc::channel(10);
        let callback = AsyncCallback { sender: resp_tx };
        let (async_tx, mut async_rx) = mpsc::channel(10);
        let async_sender = AsyncSender { sender: async_tx };
        let client = ThunderAsyncClient::new(callback, async_sender);

        let callrequest = DeviceCallRequest {
            method: "org.rdk.System.1.getSerialNumber".to_string(),
            params: None,
        };

        let request = DeviceChannelRequest::Call(callrequest);
        let async_request = ThunderAsyncRequest::new(request);
        client.send(async_request.clone()).await;
        let received = async_rx.recv().await;
        assert_eq!(received.unwrap().id, async_request.id);
    }

    #[tokio::test]
    async fn test_thunder_async_client_handle_jsonrpc_response() {
        let (resp_tx, mut resp_rx) = mpsc::channel(10);
        let callback = AsyncCallback { sender: resp_tx };
        let response = JsonRpcApiResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(6),
            result: Some(json!({"key": "value"})),
            error: None,
            method: Some("event_1".to_string()),
            params: None,
        };
        let response_bytes = serde_json::to_vec(&response).unwrap();
        let (async_tx, _async_rx) = mpsc::channel(1);
        let async_sender = AsyncSender { sender: async_tx };
        let mut client = ThunderAsyncClient::new(callback, async_sender);
        client.handle_jsonrpc_response(&response_bytes).await;

        let received = resp_rx.recv().await;
        assert_eq!(
            received.unwrap().result.unwrap().result,
            Some(json!({"key": "value"}))
        );
    }

    #[tokio::test]
    async fn test_thunder_async_client_start() {
        let (resp_tx, mut resp_rx) = mpsc::channel(10);
        let callback = AsyncCallback { sender: resp_tx };
        let (async_tx, _async_rx) = mpsc::channel(10);
        let async_sender = AsyncSender { sender: async_tx };
        let mut client = ThunderAsyncClient::new(callback.clone(), async_sender);

        let response = json!({
            "jsonrpc": "2.0",
            "result": {
                "key": "value"
            }
        });

        client
            .handle_jsonrpc_response(response.to_string().as_bytes())
            .await;
        let received = resp_rx.recv().await;
        assert!(received.is_some());
        let async_response = received.unwrap();
        assert_eq!(
            async_response.result.unwrap().result,
            Some(json!({"key": "value"}))
        );
    }
}
