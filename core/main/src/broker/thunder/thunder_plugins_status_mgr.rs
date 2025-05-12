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
use ripple_sdk::{
    api::gateway::rpc_gateway_api::JsonRpcApiResponse,
    chrono::{DateTime, Duration, Utc},
    log::{error, info},
    utils::error::RippleError,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::HashMap,
    fmt,
    sync::{Arc, RwLock},
};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use crate::broker::endpoint_broker::{
    BrokerCallback, BrokerRequest, BrokerSender, EndpointBrokerState,
};

// defautl timeout for plugin activation in seconds
const DEFAULT_PLUGIN_ACTIVATION_TIMEOUT: i64 = 8;

// As per thunder 4_4 documentation, the statechange event is published under the method "client.events.1.statechange"
// But it didn't work, most probably a documentation issue.
// const STATE_CHANGE_EVENT_METHOD: &str = "client.events.1.statechange";

const STATE_CHANGE_EVENT_METHOD: &str = "thunder.Broker.Controller.events.statechange";

#[derive(Debug, EnumIter)]
pub enum ThunderPlugin {
    Controller,
    DeviceInfo,
    DisplaySettings,
    HdcpProfile,
    LocationSync,
    Network,
    RemoteControl,
    PersistentStorage,
    System,
    Wifi,
    TextToSpeech,
    Hdcp,
    Telemetry,
    Analytics,
    UserSettings,
}

impl fmt::Display for ThunderPlugin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThunderPlugin::Controller => write!(f, "Controller"),
            ThunderPlugin::DeviceInfo => write!(f, "DeviceInfo"),
            ThunderPlugin::DisplaySettings => write!(f, "org.rdk.DisplaySettings"),
            ThunderPlugin::HdcpProfile => write!(f, "org.rdk.HdcpProfile"),
            ThunderPlugin::LocationSync => write!(f, "LocationSync"),
            ThunderPlugin::Network => write!(f, "org.rdk.Network"),
            ThunderPlugin::RemoteControl => write!(f, "org.rdk.RemoteControl"),
            ThunderPlugin::PersistentStorage => write!(f, "org.rdk.PersistentStore"),
            ThunderPlugin::System => write!(f, "org.rdk.System"),
            ThunderPlugin::Wifi => write!(f, "org.rdk.Wifi"),
            ThunderPlugin::TextToSpeech => write!(f, "org.rdk.TextToSpeech"),
            ThunderPlugin::Hdcp => write!(f, "org.rdk.HdcpProfile"),
            ThunderPlugin::Telemetry => write!(f, "org.rdk.Telemetry"),
            ThunderPlugin::Analytics => write!(f, "org.rdk.Analytics"),
            ThunderPlugin::UserSettings => write!(f, "org.rdk.UserSettings"),
        }
    }
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct Status {
    pub callsign: String,
    pub state: String,
}

#[derive(Debug, Deserialize)]
pub struct ThunderError {
    pub code: i32,
    pub message: String,
}

impl ThunderError {
    pub fn get_state(&self) -> State {
        match self.message.as_str() {
            "ERROR_INPROGRESS" | "ERROR_PENDING_CONDITIONS" => State::InProgress,
            "ERROR_UNKNOWN_KEY" => State::Missing,
            _ => State::Unknown,
        }
    }
}

impl Status {
    pub fn to_state(&self) -> State {
        match self.state.as_str() {
            "activated" | "resumed" | "suspended" => State::Activated,
            "deactivated" => State::Deactivated,
            "deactivation" => State::Deactivation,
            "activation" | "precondition" => State::Activation,
            _ => State::Unavailable,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StateChangeEvent {
    pub callsign: String,
    pub state: State,
}

#[derive(Debug, Deserialize, PartialEq, Serialize, Clone)]
pub enum State {
    Activated,
    Activation,
    Deactivated,
    Deactivation,
    Unavailable,
    Precondition,
    Suspended,
    Resumed,
    Missing,
    Error,
    InProgress,
    Unknown,
}

impl State {
    pub fn is_activated(&self) -> bool {
        matches!(self, State::Activated)
    }
    pub fn is_activating(&self) -> bool {
        matches!(self, State::Activation)
    }
    pub fn is_missing(&self) -> bool {
        matches!(self, State::Missing)
    }
    pub fn is_unavailable(&self) -> bool {
        matches!(self, State::Unavailable | State::Unknown | State::Missing)
    }
}

#[derive(Debug, Clone)]
pub struct ThunderPluginState {
    pub state: State,
    pub activation_timestamp: DateTime<Utc>,
    pub pending_requests: Vec<BrokerRequest>,
}
#[derive(Debug, Clone)]
pub struct StatusManager {
    pub status: Arc<RwLock<HashMap<String, ThunderPluginState>>>,
    pub inprogress_plugins_request: Arc<RwLock<HashMap<u64, String>>>,
}

impl Default for StatusManager {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusManager {
    pub fn new() -> Self {
        Self {
            status: Arc::new(RwLock::new(HashMap::new())),
            inprogress_plugins_request: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn get_controller_call_sign() -> String {
        "Controller.1.".to_string()
    }

    pub fn update_status(&self, plugin_name: String, state: State) {
        info!(
            "Updating the status of the plugin: {:?} to state: {:?}",
            plugin_name, state
        );
        let mut status = self.status.write().unwrap();
        // get the current plugin state from hashmap and update the State
        if let Some(plugin_state) = status.get_mut(&plugin_name) {
            plugin_state.state = state;
        } else {
            // if the plugin is not present in the hashmap, add it
            status.insert(
                plugin_name,
                ThunderPluginState {
                    state,
                    activation_timestamp: Utc::now(),
                    pending_requests: Vec::new(),
                },
            );
        }
    }

    pub fn add_broker_request_to_pending_list(&self, plugin_name: String, request: BrokerRequest) {
        let mut status = self.status.write().unwrap();
        if let Some(plugin_state) = status.get_mut(&plugin_name) {
            plugin_state.pending_requests.push(request);
        } else {
            status.insert(
                plugin_name.clone(),
                ThunderPluginState {
                    state: State::Unknown,
                    activation_timestamp: Utc::now(),
                    pending_requests: vec![request],
                },
            );
        }
        // update the time stamp
        if let Some(plugin_state) = status.get_mut(&plugin_name) {
            plugin_state.activation_timestamp = Utc::now();
        }
    }

    // clear all pending requests for the given plugin and return the list of requests to the caller
    // Also return a flag to indicate if activation time has expired.
    pub fn retrive_pending_broker_requests(
        &self,
        plugin_name: String,
    ) -> (Vec<BrokerRequest>, bool) {
        let mut status = self.status.write().unwrap();
        if let Some(plugin_state) = status.get_mut(&plugin_name) {
            let pending_requests = plugin_state.pending_requests.clone();
            plugin_state.pending_requests.clear();
            // check if the activation time has expired.
            let now = Utc::now();
            if now - plugin_state.activation_timestamp
                > Duration::seconds(DEFAULT_PLUGIN_ACTIVATION_TIMEOUT)
            {
                return (pending_requests, true);
            } else {
                return (pending_requests, false);
            }
        }
        (Vec::new(), false)
    }

    pub fn get_all_pending_broker_requests(&self, plugin_name: String) -> Vec<BrokerRequest> {
        let status = self.status.read().unwrap();
        if let Some(plugin_state) = status.get(&plugin_name) {
            plugin_state.pending_requests.clone()
        } else {
            Vec::new()
        }
    }

    pub fn clear_all_pending_broker_requests(&self, plugin_name: String) {
        let mut status = self.status.write().unwrap();
        if let Some(plugin_state) = status.get_mut(&plugin_name) {
            plugin_state.pending_requests.clear();
        }
    }

    pub fn get_status(&self, plugin_name: String) -> Option<ThunderPluginState> {
        let status = self.status.read().unwrap();
        status.get(&plugin_name).cloned()
    }

    pub fn generate_plugin_activation_request(&self, plugin_name: String) -> String {
        let id = EndpointBrokerState::get_next_id();
        let controller_call_sign = Self::get_controller_call_sign();

        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": format!("{}activate", controller_call_sign),
            "params": json!({
                "callsign": plugin_name,
            })
        })
        .to_string();
        // Add this request to the inprogress_plugins_request
        self.add_thunder_request_to_inprogress_list(id, request.clone());
        request
    }

    pub fn generate_plugin_status_request(&self, plugin_name: Option<String>) -> String {
        let id = EndpointBrokerState::get_next_id();
        let controller_call_sign = Self::get_controller_call_sign();
        let mut method = format!("{}status", controller_call_sign);
        if let Some(p) = plugin_name {
            method = format!("{}status@{}", controller_call_sign, p);
        }

        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
        })
        .to_string();
        // Add this request to the inprogress_plugins_request
        self.add_thunder_request_to_inprogress_list(id, request.clone());
        request
    }

    pub fn generate_state_change_subscribe_request(&self) -> String {
        let id = EndpointBrokerState::get_next_id();
        let controller_call_sign = Self::get_controller_call_sign();

        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": format!("{}register", controller_call_sign),
            "params": json!({
                "event": "statechange",
                "id": "thunder.Broker.Controller.events"
            })
        })
        .to_string();
        // Add this request to the inprogress_plugins_request
        self.add_thunder_request_to_inprogress_list(id, request.clone());
        request
    }

    fn add_thunder_request_to_inprogress_list(&self, id: u64, request: String) {
        let mut inprogress_plugins_request = self.inprogress_plugins_request.write().unwrap();
        inprogress_plugins_request.insert(id, request);
    }

    pub async fn is_controller_response(
        &self,
        sender: BrokerSender,
        callback: BrokerCallback,
        result: &[u8],
    ) -> bool {
        let data = match serde_json::from_slice::<JsonRpcApiResponse>(result) {
            Ok(data) => data,
            Err(_) => return false,
        };

        if let Some(method) = data.method {
            info!("is_controller_response Method: {:?}", method);
            if method == STATE_CHANGE_EVENT_METHOD {
                // intercept the statechange event and update plugin status.
                let params = match data.params {
                    Some(params) => params,
                    None => return false,
                };

                let event: StateChangeEvent = match serde_json::from_value(params) {
                    Ok(event) => event,
                    Err(_) => return false,
                };

                self.update_status(event.callsign.clone(), event.state.clone());

                if event.state.is_activated() {
                    // get the pending BrokerRequest and process.
                    let (pending_requests, expired) =
                        self.retrive_pending_broker_requests(event.callsign);
                    if !pending_requests.is_empty() {
                        for pending_request in pending_requests {
                            if expired {
                                error!("Expired request: {:?}", pending_request);
                                callback
                                    .send_error(pending_request, RippleError::ServiceError)
                                    .await;
                            } else {
                                let _ = sender.send(pending_request).await;
                            }
                        }
                    }
                }

                return true;
            }
        }

        if let Some(id) = data.id {
            let inprogress_plugins_request = self.inprogress_plugins_request.read().unwrap();
            return inprogress_plugins_request.contains_key(&id);
        }

        false
    }
    async fn on_activate_response(
        &self,
        sender: BrokerSender,
        callback: BrokerCallback,
        data: &JsonRpcApiResponse,
        request: &str,
    ) {
        let result = match &data.result {
            Some(result) => result,
            None => return,
        };

        let callsign = match request.split("callsign\":").last() {
            Some(callsign) => callsign.trim_matches(|c| c == '"' || c == '}'),
            None => return,
        };

        let (pending_requests, expired) =
            self.retrive_pending_broker_requests(callsign.to_string());

        if result.is_null() {
            self.update_status(callsign.to_string(), State::Activated);

            for pending_request in pending_requests {
                if expired {
                    error!("Expired request: {:?}", pending_request);
                    callback
                        .send_error(pending_request, RippleError::ServiceError)
                        .await;
                } else {
                    let _ = sender.send(pending_request).await;
                }
            }
        } else if let Some(_e) = &data.error {
            self.on_thunder_error_response(callback, data, &callsign.to_string())
                .await;
        }
    }

    async fn on_status_response(
        &self,
        sender: BrokerSender,
        callback: BrokerCallback,
        data: &JsonRpcApiResponse,
        request: &str,
    ) {
        let mut callsigns: Vec<String> = Vec::new();

        let callsign = {
            if request.contains("@") {
                match request.split('@').last() {
                    Some(callsign) => callsign.trim_matches(|c| c == '"' || c == '}'),
                    // This would least likely happen because we check "@" before split request by "@",
                    // but if it does, we can just use the request as is.
                    None => request,
                }
            } else {
                ""
            }
        };

        if !request.contains("@") {
            let thunder_plugins: Vec<ThunderPlugin> =
                ThunderPlugin::iter().collect::<Vec<ThunderPlugin>>();
            for plugin in thunder_plugins {
                let c = plugin.to_string();
                callsigns.push(c);
            }
        } else {
            callsigns.push(callsign.to_string());
        }

        let result = match &data.result {
            Some(result) => result,
            None => {
                self.on_thunder_error_response(callback, data, &callsigns[0].to_string())
                    .await;
                return;
            }
        };

        let status_res: Vec<Status> = match serde_json::from_value(result.clone()) {
            Ok(status_res) => status_res,
            Err(_) => {
                self.on_thunder_error_response(callback, data, &callsigns[0].to_string())
                    .await;
                return;
            }
        };

        //filtering the status_res by matching status_res.callsign with callsigns
        let status_res: Vec<Status> = status_res
            .into_iter()
            .filter(|status| callsigns.contains(&status.callsign))
            .collect();

        for status in status_res {
            self.update_status(status.callsign.to_string(), status.to_state());

            let (pending_requests, expired) =
                self.retrive_pending_broker_requests(status.callsign.to_string());

            for pending_request in pending_requests {
                if expired {
                    error!("Expired request: {:?}", pending_request);
                    callback
                        .send_error(pending_request, RippleError::ServiceError)
                        .await;
                } else {
                    let _ = sender.send(pending_request).await;
                }
            }
        }
    }

    async fn on_thunder_error_response(
        &self,
        callback: BrokerCallback,
        data: &JsonRpcApiResponse,
        plugin_name: &String,
    ) {
        let error = match &data.error {
            Some(error) => error,
            None => return,
        };

        error!(
            "Error Received from Thunder on getting the status of the plugin: {:?}",
            error
        );

        let thunder_error: ThunderError = match serde_json::from_value(error.clone()) {
            Ok(error) => error,
            Err(_) => return,
        };

        let state = thunder_error.get_state();
        self.update_status(plugin_name.to_string(), state.clone());

        if state.is_unavailable() {
            let (pending_requests, _) =
                self.retrive_pending_broker_requests(plugin_name.to_string());

            for pending_request in pending_requests {
                callback
                    .send_error(pending_request, RippleError::ServiceError)
                    .await;
            }
        }
    }

    pub fn get_from_inprogress_plugins_request_list(&self, id: u64) -> Option<String> {
        let inprogress_plugins_request = self.inprogress_plugins_request.read().unwrap();
        inprogress_plugins_request.get(&id).cloned()
    }

    pub async fn handle_controller_response(
        &self,
        sender: BrokerSender,
        callback: BrokerCallback,
        result: &[u8],
    ) {
        let data = match serde_json::from_slice::<JsonRpcApiResponse>(result) {
            Ok(data) => data,
            Err(_) => return,
        };

        let id = match data.id {
            Some(id) => id,
            None => return,
        };

        let request = match self.get_from_inprogress_plugins_request_list(id) {
            Some(request) => request,
            None => return,
        };

        if request.contains("Controller.1.activate") {
            // handle activate response
            self.on_activate_response(sender, callback, &data, &request)
                .await;
        } else if request.contains("Controller.1.status") {
            // handle status response
            self.on_status_response(sender, callback, &data, &request)
                .await;
        } else if request.contains("Controller.1.register") {
            // nothing to do here
            info!("StatusManger Received response for register request");
        }

        let mut inprogress_plugins_request = self.inprogress_plugins_request.write().unwrap();
        inprogress_plugins_request.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ripple_sdk::tokio::{
        self,
        sync::mpsc::{self, channel},
    };

    #[test]
    fn test_generate_state_change_subscribe_request() {
        let status_manager = StatusManager::new();
        let request = status_manager.generate_state_change_subscribe_request();
        assert!(request.contains("register"));
        assert!(request.contains("statechange"));
    }

    #[tokio::test]
    async fn test_on_activate_response() {
        let status_manager = StatusManager::new();
        let (tx, _tr) = mpsc::channel(10);
        let broker = BrokerSender { sender: tx };

        let (tx_1, _tr_1) = channel(2);
        let callback = BrokerCallback { sender: tx_1 };

        let data = JsonRpcApiResponse {
            id: Some(1),
            jsonrpc: "2.0".to_string(),
            result: Some(serde_json::json!(null)),
            error: None,
            method: None,
            params: None,
        };
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"Controller.1.activate","params":{"callsign":"TestPlugin"}}"#;
        status_manager
            .on_activate_response(broker, callback, &data, request)
            .await;
        let status = status_manager.get_status("TestPlugin".to_string());
        assert_eq!(status.unwrap().state, State::Activated);
    }

    #[tokio::test]
    async fn test_on_status_response() {
        let status_manager = StatusManager::new();
        let (tx, _tr) = mpsc::channel(10);
        let broker = BrokerSender { sender: tx };

        let (tx_1, _tr_1) = channel(2);
        let callback = BrokerCallback { sender: tx_1 };

        let data = JsonRpcApiResponse {
            id: Some(1),
            jsonrpc: "2.0".to_string(),
            result: Some(serde_json::json!([{"callsign":"TestPlugin","state":"activated"}])),
            error: None,
            method: None,
            params: None,
        };
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"Controller.1.status@TestPlugin"}"#;
        status_manager
            .on_status_response(broker, callback, &data, request)
            .await;
        let status = status_manager.get_status("TestPlugin".to_string());
        assert_eq!(status.unwrap().state, State::Activated);
    }

    #[tokio::test]
    async fn test_on_thunder_error_response() {
        let status_manager = StatusManager::new();

        let (tx_1, _tr_1) = channel(2);
        let callback = BrokerCallback { sender: tx_1 };

        let data = JsonRpcApiResponse {
            id: Some(1),
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(serde_json::json!({"code":1,"message":"ERROR_UNKNOWN_KEY"})),
            method: None,
            params: None,
        };
        let plugin_name = "TestPlugin".to_string();
        status_manager
            .on_thunder_error_response(callback, &data, &plugin_name)
            .await;
        let status = status_manager.get_status("TestPlugin".to_string());
        assert_eq!(status.unwrap().state, State::Missing);
    }

    // Uncomment and use the following unit test only for local testing. Not use as part of the CI/CD pipeline.
    /*
    use ripple_sdk::{
        api::gateway::rpc_gateway_api::{ApiProtocol, CallContext, RpcRequest},
    };
    use crate::broker::rules_engine::{Rule, RuleTransform};

    #[tokio::test]
    async fn test_expired_broker_request() {
        let status_manager = StatusManager::new();
        let (tx, _tr) = mpsc::channel(10);
        let broker = BrokerSender { sender: tx };

        let (tx_1, _tr_1) = channel(2);
        let callback = BrokerCallback { sender: tx_1 };

        let data = JsonRpcApiResponse {
            id: Some(1),
            jsonrpc: "2.0".to_string(),
            result: Some(serde_json::json!(null)),
            error: None,
            method: None,
            params: None,
        };
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"Controller.1.activate","params":{"callsign":"TestPlugin"}}"#;
        status_manager
            .on_activate_response(broker, callback, &data, request)
            .await;
        let status = status_manager.get_status("TestPlugin".to_string());
        assert_eq!(status.unwrap().state, State::Activated);

        let ctx = CallContext::new(
            "session_id".to_string(),
            "request_id".to_string(),
            "app_id".to_string(),
            1,
            ApiProtocol::Bridge,
            "method".to_string(),
            Some("cid".to_string()),
            true,
        );

        // Add a request to the pending list
        let request = BrokerRequest {
            rpc: RpcRequest {
                ctx,
                params_json: "".to_string(),
                method: "TestPlugin".to_string(),
            },
            rule: Rule {
                alias: "TestPlugin".to_string(),
                transform: RuleTransform::default(),
                endpoint: None,
            },
            subscription_processed: None,
        };
        status_manager.add_broker_request_to_pending_list("TestPlugin".to_string(), request);

        // Sleep for 10 seconds to expire the request
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

        // Check if the request is expired
        let (pending_requests, expired) =
            status_manager.retrive_pending_broker_requests("TestPlugin".to_string());
        assert_eq!(expired, true);
        assert_eq!(pending_requests.len(), 1);
    }
    */
}
