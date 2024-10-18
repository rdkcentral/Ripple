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

use std::time::Duration;

use futures::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt,StreamExt};
use ripple_sdk::{
    api::{device::device_operator::DeviceChannelRequest, gateway::rpc_gateway_api::JsonRpcApiResponse}, log::{debug, error, info}, tokio::{self, net::TcpStream, sync::mpsc::{Receiver, Sender}}, utils::{error::RippleError, rpc_utils::extract_tcp_port}
};

use serde_json::json;
use tokio_tungstenite::{client_async, tungstenite::Message, WebSocketStream};

use crate::utils::get_next_id;

use super::thunder_plugins_status_mgr::{BrokerCallback, BrokerSender, StatusManager};

#[derive(Clone,Debug)]
pub struct ThunderAsyncClient {
    status_manager: StatusManager,
    sender: BrokerSender,
    callback: BrokerCallback
}

#[derive(Clone,Debug)]
pub struct ThunderAsyncRequest{
    pub id: u64,
    request: DeviceChannelRequest
}

impl ThunderAsyncRequest {
    pub fn new(request:DeviceChannelRequest) -> Self {
        Self {
            id: get_next_id(),
            request
        }
    }
}

#[derive(Clone,Debug)]
pub struct ThunderAsyncResponse{
    pub id: Option<u64>,
    pub result: Result<JsonRpcApiResponse,RippleError>
}

impl ThunderAsyncResponse {
    fn new_response(response:JsonRpcApiResponse) -> Self {
        Self {
            id: None,
            result: Ok(response)
        }
    }

    fn new_error(id:u64,e:RippleError) -> Self {
        Self {
            id: Some(id),
            result: Err(e)
        }
    }
}

impl ThunderAsyncClient {

    pub fn get_sender(&self) -> BrokerSender {
        self.sender.clone()
    }

    async fn create_ws(
        endpoint: &str
    ) -> (
        SplitSink<WebSocketStream<TcpStream>, Message>,
        SplitStream<WebSocketStream<TcpStream>>,
    ) {
        info!("Broker Endpoint url {}", endpoint);
       
        let url = url::Url::parse(endpoint).unwrap();
        let port = extract_tcp_port(endpoint);
        info!("Url host str {}", url.host_str().unwrap());
        let mut index = 0;

        loop {
            // Try connecting to the tcp port first
            if let Ok(v) = TcpStream::connect(&port).await {
                // Setup handshake for websocket with the tcp port
                // Some WS servers lock on to the Port but not setup handshake till they are fully setup
                if let Ok((stream, _)) = client_async(endpoint, v).await {
                    break stream.split();
                }
            }
            if (index % 10).eq(&0) {
                error!(
                    "Broker with {} failed with retry for last {} secs in {}",
                    endpoint, index, port
                );
            }
            index += 1;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    fn prepare_request(
        &self,
        request: &ThunderAsyncRequest,
    ) -> Result<Vec<String>, RippleError> {
        let mut requests = Vec::new();
        let id = request.id;
        let (callsign, method) = request.request.get_callsign_method();
        if method.is_empty() {
            return Err(RippleError::InvalidInput);
        }
        // check if the plugin is activated.
        let status = match self.status_manager.get_status(callsign.clone()) {
            Some(v) => v.clone(),
            None => {
                self.status_manager
                    .add_broker_request_to_pending_list(callsign.clone(), request.clone());
                // PluginState is not available with StateManager,  create an internal thunder request to activate the plugin
                let request = self
                    .status_manager
                    .generate_plugin_status_request(callsign.clone());
                requests.push(request.to_string());
                return Ok(requests);
            }
        };

        if status.state.is_missing() {
            error!("Plugin {} is missing", callsign);
            return Err(RippleError::ServiceError);
        }

        if status.state.is_activating() {
            info!("Plugin {} is activating", callsign);
            return Err(RippleError::ServiceNotReady);
        }

        if !status.state.is_activated() {
            // add the broker request to pending list
            self.status_manager
                .add_broker_request_to_pending_list(callsign.clone(), request.clone());
            // create an internal thunder request to activate the plugin
            let request = self
                .status_manager
                .generate_plugin_activation_request(callsign.clone());
            requests.push(request.to_string());
            return Ok(requests);
        }


        match &request.request {
            DeviceChannelRequest::Call(c) => {
                // Simple request and response handling
                requests.push(
                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "method": c.method,
                        "params": c.params
                    })
                    .to_string(),
                )
                
            },
            DeviceChannelRequest::Unsubscribe(_) => {
                    requests.push(
                        json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "method": format!("{}.unregister", callsign),
                            "params": {
                                "event": method,
                                "id": "client.events"
                            }
                        })
                        .to_string()
                    )
            },
            DeviceChannelRequest::Subscribe(_) => {
                requests.push(
                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "method": format!("{}.register", callsign),
                        "params": json!({
                            "event": method,
                            "id": "client.events"
                        })
                    })
                    .to_string(),
                )
            }
        }

        Ok(requests)
    }

    pub fn new(callback: BrokerCallback, tx: Sender<ThunderAsyncRequest>) -> Self {
        Self {
            status_manager: StatusManager::new(),
            sender: BrokerSender { sender: tx.clone() },
            callback
        }
    }

    pub async fn start(&self, url:&str, mut tr: Receiver<ThunderAsyncRequest>) -> Receiver<ThunderAsyncRequest> {
        let callback = self.callback.clone();
        let (mut ws_tx, mut ws_rx) =
                Self::create_ws(url).await;
        // send the first request to the broker. This is the controller statechange subscription request
        let status_request = self.status_manager
        .generate_state_change_subscribe_request();

        let _feed = ws_tx
                .feed(tokio_tungstenite::tungstenite::Message::Text(
                    status_request.to_string(),
                ))
                .await;
        let _flush = ws_tx.flush().await;
        let client_c = self.clone();
        let callback_for_sender = callback.clone();
        tokio::pin! {
            let read = ws_rx.next();
        }
        loop {
            tokio::select! {
                Some(value) = &mut read => {
                    match value {
                        Ok(v) => {
                            if let tokio_tungstenite::tungstenite::Message::Text(t) = v {
                                if client_c.status_manager.is_controller_response(client_c.get_sender(), callback.clone(), t.as_bytes()).await {
                                    client_c.status_manager.handle_controller_response(client_c.get_sender(), callback.clone(), t.as_bytes()).await;
                                }
                                else {
                                    // send the incoming text without context back to the sender
                                    Self::handle_jsonrpc_response(t.as_bytes(),callback.clone()).await
                                }
                            }
                        },
                        Err(e) => {
                            error!("Broker Websocket error on read {:?}", e);
                            // Time to reconnect Thunder with existing subscription
                            break;
                        }
                    }
                },
                Some(request) = tr.recv() => {
                    debug!("Got request from receiver for broker {:?}", request);
                    match client_c.prepare_request(&request) {
                        Ok(updated_request) => {
                            debug!("Sending request to broker {:?}", updated_request);
                            for r in updated_request {
                                let _feed = ws_tx.feed(tokio_tungstenite::tungstenite::Message::Text(r)).await;
                                let _flush = ws_tx.flush().await;
                            }
                        }
                        Err(e) => {
                            let response = ThunderAsyncResponse::new_error(request.id,e.clone());
                            match e {
                                RippleError::ServiceNotReady => {
                                    info!("Thunder Service not ready, request is now in pending list {:?}", request);
                                },
                                _ => {
                                    error!("error preparing request {:?}", e)
                                }
                            }
                            callback_for_sender.send(response).await
                        }
                    }
                }
            }
        }
        // when WS is disconnected return the tr back to caller helps restabilish connection
        tr
    }

    /// Default handler method for the broker to remove the context and send it back to the
    /// client for consumption
    async fn handle_jsonrpc_response(result: &[u8], callback: BrokerCallback) {
        if let Ok(message) = serde_json::from_slice::<JsonRpcApiResponse>(result) {
            callback.send(ThunderAsyncResponse::new_response(message)).await
        } else {
            error!("Invalid JSON RPC message sent by Thunder");
        }
        
    }

    pub async fn send(&self, request: ThunderAsyncRequest) {
        
    }
}
