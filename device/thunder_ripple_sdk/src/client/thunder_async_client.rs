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
use futures_util::{SinkExt, StreamExt};
use ripple_sdk::api::device::device_operator::DeviceResponseMessage;
use ripple_sdk::{
    api::{
        device::device_operator::DeviceChannelRequest, gateway::rpc_gateway_api::JsonRpcApiResponse,
    },
    log::{debug, error, info},
    tokio::{self, net::TcpStream, sync::mpsc::Receiver},
    utils::{error::RippleError, rpc_utils::extract_tcp_port},
};
use serde_json::json;
use tokio_tungstenite::{client_async, tungstenite::Message, WebSocketStream};

use crate::utils::get_next_id;

use super::thunder_plugins_status_mgr::{BrokerCallback, BrokerSender, StatusManager};

#[derive(Clone, Debug)]
pub struct ThunderAsyncClient {
    status_manager: StatusManager,
    sender: BrokerSender,
    callback: BrokerCallback,
}

#[derive(Clone, Debug)]
pub struct ThunderAsyncRequest {
    pub id: u64,
    request: DeviceChannelRequest,
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

impl ThunderAsyncResponse {
    fn new_response(response: JsonRpcApiResponse) -> Self {
        Self {
            id: None,
            result: Ok(response),
        }
    }

    fn new_error(id: u64, e: RippleError) -> Self {
        Self {
            id: Some(id),
            result: Err(e),
        }
    }

    pub fn get_event(&self) -> Option<String> {
        if let Ok(e) = &self.result {
            return e.method.clone();
        }
        None
    }

    pub fn get_id(&self) -> Option<u64> {
        match &self.result {
            Ok(response) => response.id,
            Err(_) => None,
        }
    }

    pub fn get_device_resp_msg(&self, sub_id: Option<String>) -> Option<DeviceResponseMessage> {
        println!("@@NNA...get_device_resp_msg res:{:?}", self.result);
        let json_resp = match &self.result {
            Ok(json_resp_res) => {
                println!("@@NNA...get_device_resp_msg resp::{:?}", json_resp_res);
                json_resp_res
            }
            _ => return None,
        };

        let mut device_response_msg = None;

        if let Some(res) = &json_resp.result {
            device_response_msg = Some(DeviceResponseMessage::new(res.clone(), sub_id));
        } else if let Some(er) = &json_resp.error {
            device_response_msg = Some(DeviceResponseMessage::new(er.clone(), sub_id));
        } else if json_resp.clone().method.is_some() {
            if let Some(params) = &json_resp.params {
                let dev_resp = serde_json::to_value(params).unwrap();
                device_response_msg = Some(DeviceResponseMessage::new(dev_resp, sub_id));
            }
        } else {
            error!("deviceresponse msg extraction failed.");
        }
        println!("@@NNA...device_response_msg: {:?}", device_response_msg);
        device_response_msg
    }
}

impl ThunderAsyncClient {
    fn get_id_from_result(result: &[u8]) -> Option<u64> {
        serde_json::from_slice::<JsonRpcApiResponse>(result)
            .ok()
            .and_then(|data| data.id)
    }

    pub fn get_sender(&self) -> BrokerSender {
        self.sender.clone()
    }

    pub fn get_thndrasync_callback(&self) -> BrokerCallback {
        self.callback.clone()
    }
    async fn create_ws(
        endpoint: &str,
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

    fn prepare_request(&self, request: &ThunderAsyncRequest) -> Result<Vec<String>, RippleError> {
        println!("@@@NNA-----> in start prepare request id :{}", request.id);
        let mut requests = Vec::new();
        let id: u64 = request.id;
        let (callsign, method) = request.request.get_callsign_method();
        println!(
            "@@@NNA-----> in prepare request callsign :{}, method :{}",
            callsign, method
        );
        if method.is_empty() {
            return Err(RippleError::InvalidInput);
        }
        // check if the plugin is activated.
        let status = match self.status_manager.get_status(callsign.clone()) {
            Some(v) => {
                println!(
                    "@@@NNA....status_manager.get_status returned some value. v:{:?} ",
                    v
                );
                v.clone()
            }
            None => {
                println!("@@@NNA....status_manager.get_status return none.");
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
        println!(
            "@@@NNA-----> in prepare request ThunderPluginState :{:?} ",
            status
        );

        if status.state.is_missing() {
            error!("Plugin {} is missing", callsign);
            return Err(RippleError::ServiceError);
        }

        if status.state.is_activating() {
            info!("Plugin {} is activating", callsign);
            return Err(RippleError::ServiceNotReady);
        }

        if !status.state.is_activated() {
            println!("@@@NNA....plugin not activated yet...");
            // add the broker request to pending list
            self.status_manager
                .add_broker_request_to_pending_list(callsign.clone(), request.clone());
            // create an internal thunder request to activate the plugin
            let request = self
                .status_manager
                .generate_plugin_activation_request(callsign.clone());
            requests.push(request.to_string());
            return Ok(requests);
        } else {
            println!("@@@NNA....plugin is activated Now...&&&&&...");
        }

        println!(
            "@@@NNA....after plugin activation devicechannelrequest : {:?} ",
            request.request
        );

        match &request.request {
            DeviceChannelRequest::Call(c) => {
                println!(
                    "@@@NNA-----> in DeviceChannelRequest::Call request id :{}",
                    request.id
                );
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
            }
            DeviceChannelRequest::Unsubscribe(_) => {
                println!(
                    "@@@NNA-----> in DeviceChannelRequest::Unsubscribe request id :{}",
                    request.id
                );
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
                    .to_string(),
                )
            }
            DeviceChannelRequest::Subscribe(_) => {
                println!(
                    "@@@NNA-----> in DeviceChannelRequest::Subscribe request id :{}",
                    request.id
                );
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

    pub fn new(callback: BrokerCallback, sender: BrokerSender) -> Self {
        Self {
            status_manager: StatusManager::new(),
            sender,
            callback,
        }
    }

    pub async fn start(
        &self,
        url: &str,
        mut tr: Receiver<ThunderAsyncRequest>,
    ) -> Receiver<ThunderAsyncRequest> {
        let callback = self.callback.clone();
        let (mut ws_tx, mut ws_rx) = Self::create_ws(url).await;
        // send the first request to the broker. This is the controller statechange subscription request
        let status_request = self
            .status_manager
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
                    debug!("@@@NNA...Got response in thunder async client start");
                    match value {
                        Ok(v) => {
                            println!("@@@NNA----> msg v: {:?}",v);
                            if let tokio_tungstenite::tungstenite::Message::Text(t) = v {
                                println!("@@@NNA----> msg t: {:?}",t);
                                if client_c.status_manager.is_controller_response(client_c.get_sender(), callback.clone(), t.as_bytes()).await {
                                    debug!("@@@NNA---->Got response in thunder async client start before handle_controller_response. sender:{:?}, callback:{:?}",client_c.get_sender(), callback.clone() );
                                    client_c.status_manager.handle_controller_response(client_c.get_sender(), callback.clone(), t.as_bytes()).await;
                                    debug!("@@@NNA---->after step client_c.status_manager.handle_controller_response");
                                }
                                else {
                                    let id = Self::get_id_from_result(t.as_bytes());
                                    println!("@@@NNA.... jsn response id:{:?}, callback:{:?}", id, callback.clone());
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
                    debug!("Got request from receiver for broker for method{:?}", request.request.get_callsign_method());
                    match client_c.prepare_request(&request) {
                        Ok(updated_request) => {
                            debug!("Sending request to broker {:?}", updated_request);
                            for r in updated_request {
                                println!("@@@NNA----> we r at the feed, flush operations block....");
                                println!("@@@NNA----> r :{}", r);
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
                            println!("@@@NNA----> before callback_for_sender send with response: {:?}", response);
                            callback_for_sender.send(response).await;
                            println!("@@@NNA----> after callback_for_sender send ");
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
        debug!(
            "@@@NNA...in handle_jsonrpc_response with callback: {:?}, result:{:?}",
            callback, result
        );
        if let Ok(message) = serde_json::from_slice::<JsonRpcApiResponse>(result) {
            debug!(
                "@@@NNA...in handle_jsonrpc_response msg:{:?} to callback:{:?}",
                message, callback
            );
            callback
                .send(ThunderAsyncResponse::new_response(message))
                .await
        } else {
            error!("Invalid JSON RPC message sent by Thunder");
        }
    }

    pub async fn send(&self, request: ThunderAsyncRequest) {
        if let Err(e) = self.sender.send(request).await {
            error!("Failed to send thunder Async Request: {:?}", e);
        }
    }
}
