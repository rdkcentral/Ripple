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
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

use ripple_sdk::{
    api::gateway::rpc_gateway_api::JsonRpcApiResponse, log::info, tokio, utils::error::RippleError,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::broker::endpoint_broker::{
    BrokerCallback, BrokerRequest, BrokerSender, EndpointBrokerState,
};

const DEFAULT_PLUGIN_ACTIVATION_TIMEOUT: u64 = 5;

#[derive(Debug, Deserialize, PartialEq, Serialize, Clone)]
pub struct Status {
    pub callsign: String,
    pub state: String,
    pub startmode: String,
}

#[derive(Debug, Deserialize)]
pub struct ThunderError {
    pub code: i32,
    pub message: String,
}

impl ThunderError {
    pub fn get_plugin_state(&self) -> State {
        match self.message.as_str() {
            "ERROR_INPROGRESS" | "ERROR_PENDING_CONDITIONS" => State::InProgress,
            "ERROR_UNKNOWN_KEY" => State::Missing,
            _ => State::Unknown,
        }
    }
}

impl Status {
    pub fn to_plugin_state(&self) -> State {
        match self.state.as_str() {
            "activated" | "resumed" | "suspended" => State::Activated,
            "deactivated" => State::Deactivated,
            "deactivation" => State::Deactivation,
            "activation" | "precondition" => State::Activation,
            _ => State::Unavailable,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
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
    pub last_seen: Duration,
    pub pending_requests: Vec<BrokerRequest>,
}
#[derive(Debug, Clone)]
pub struct StatusManager {
    pub status: Arc<RwLock<HashMap<String, ThunderPluginState>>>,
    pub active_plugins_request: Arc<RwLock<HashMap<u64, String>>>,
}

impl StatusManager {
    pub fn new() -> Self {
        Self {
            status: Arc::new(RwLock::new(HashMap::new())),
            active_plugins_request: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn get_controller_call_sign() -> String {
        "Controller.1.".to_string()
    }

    pub fn update_status(&self, plugin_name: String, state: State) {
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
                    last_seen: Duration::from_secs(0),
                    pending_requests: Vec::new(),
                },
            );
        }
    }

    pub fn add_request_to_pending(&self, plugin_name: String, request: BrokerRequest) {
        let mut status = self.status.write().unwrap();
        if let Some(plugin_state) = status.get_mut(&plugin_name) {
            plugin_state.pending_requests.push(request);
        } else {
            status.insert(
                plugin_name.clone(),
                ThunderPluginState {
                    state: State::Unknown,
                    last_seen: Duration::from_secs(0),
                    pending_requests: vec![request],
                },
            );
        }
        // update the last seen time
        if let Some(plugin_state) = status.get_mut(&plugin_name) {
            plugin_state.last_seen = Duration::from_secs(0);
        }
    }

    // clear all pending requests for the given plugin and return the list of requests to the caller
    // Also return a flag to indicate if activation time has expired.
    pub fn retrive_pending_request(&self, plugin_name: String) -> (Vec<BrokerRequest>, bool) {
        let mut status = self.status.write().unwrap();
        if let Some(plugin_state) = status.get_mut(&plugin_name) {
            let pending_requests = plugin_state.pending_requests.clone();
            plugin_state.pending_requests.clear();
            // check if the activation time has expired.
            if plugin_state.last_seen.as_secs() > DEFAULT_PLUGIN_ACTIVATION_TIMEOUT {
                return (pending_requests, true);
            } else {
                return (pending_requests, false);
            }
        }
        (Vec::new(), false)
    }

    pub fn get_pending_requests(&self, plugin_name: String) -> Vec<BrokerRequest> {
        let status = self.status.read().unwrap();
        if let Some(plugin_state) = status.get(&plugin_name) {
            plugin_state.pending_requests.clone()
        } else {
            Vec::new()
        }
    }

    pub fn clear_pending_requests(&self, plugin_name: String) {
        let mut status = self.status.write().unwrap();
        if let Some(plugin_state) = status.get_mut(&plugin_name) {
            plugin_state.pending_requests.clear();
        }
    }

    pub fn update_pending_requests(&self, plugin_name: String, requests: Vec<BrokerRequest>) {
        let mut status = self.status.write().unwrap();
        if let Some(plugin_state) = status.get_mut(&plugin_name) {
            plugin_state.pending_requests = requests;
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
        // Add this request to the active_plugins_request
        self.add_active_thunder_request(id, request.clone());
        request
    }

    pub fn generate_plugin_status_request(&self, plugin_name: String) -> String {
        let id = EndpointBrokerState::get_next_id();
        let controller_call_sign = Self::get_controller_call_sign();

        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": format!("{}status@{}", controller_call_sign, plugin_name),
        })
        .to_string();
        // Add this request to the active_plugins_request
        self.add_active_thunder_request(id, request.clone());
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
                "id": "client.Controller.1.events"
            })
        })
        .to_string();
        // Add this request to the active_plugins_request
        self.add_active_thunder_request(id, request.clone());
        request
    }

    fn add_active_thunder_request(&self, id: u64, request: String) {
        let mut active_plugins_request = self.active_plugins_request.write().unwrap();
        active_plugins_request.insert(id, request);
    }

    pub async fn is_controller_response(
        &self,
        sender: BrokerSender,
        callback: BrokerCallback,
        result: &[u8],
    ) -> bool {
        if let Ok(data) = serde_json::from_slice::<JsonRpcApiResponse>(result) {
            if let Some(method) = data.method {
                if method == "client.events.1.statechange" {
                    // intercept the statechange event and update plugin status.
                    if let Some(params) = data.params {
                        let event: Result<StateChangeEvent, serde_json::Error> =
                            serde_json::from_value(params);
                        if let Ok(event) = event {
                            self.update_status(event.callsign.clone(), event.state.clone());
                            if event.state.is_activated() {
                                // get the pending BrokerRequest and process.
                                let (pending_requests, expired) =
                                    self.retrive_pending_request(event.callsign);
                                if pending_requests.len() > 0 {
                                    for pending_request in pending_requests {
                                        if expired {
                                            info!("Expired request: {:?}", pending_request);
                                            callback
                                                .send_error(
                                                    pending_request,
                                                    RippleError::ServiceError,
                                                )
                                                .await;
                                        } else {
                                            let _ = sender.send(pending_request).await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // return false from here so that other subscribers can also process the response
                    return false;
                }
            }
            if let Some(id) = data.id {
                let active_plugins_request = self.active_plugins_request.read().unwrap();
                active_plugins_request.contains_key(&id)
            } else {
                false
            }
        } else {
            false
        }
    }

    async fn on_activate_response(
        &self,
        sender: BrokerSender,
        callback: BrokerCallback,
        data: &JsonRpcApiResponse,
        request: &String,
    ) {
        if let Some(result) = &data.result {
            if let Some(callsign) = request.split("callsign\":").last() {
                let plugin_name = callsign.trim_matches(|c| c == '"' || c == '}');
                // get the pending BrokerRequest and process.
                let (pending_requests, expired) =
                    self.retrive_pending_request(plugin_name.to_string());
                if result.is_null() {
                    self.update_status(plugin_name.to_string(), State::Activated);
                    if pending_requests.len() > 0 {
                        for pending_request in pending_requests {
                            if expired {
                                info!("Expired request: {:?}", pending_request);
                                callback
                                    .send_error(pending_request, RippleError::ServiceError)
                                    .await;
                            } else {
                                let _ = sender.send(pending_request).await;
                            }
                        }
                    }
                } else if let Some(_e) = &data.error {
                    Self::on_thunder_error_response(self, callback, data, &plugin_name.to_string())
                        .await;
                }
            }
        }
    }

    async fn on_status_response(
        &self,
        sender: BrokerSender,
        callback: BrokerCallback,
        data: &JsonRpcApiResponse,
        request: &String,
    ) {
        // handle status response
        if let Some(result) = &data.result {
            if let Some(callsign) = request.split("@").last() {
                let plugin_name = callsign.trim_matches(|c| c == '"' || c == '}');
                let status_res: Result<Vec<Status>, serde_json::Error> =
                    serde_json::from_value(result.clone());
                match status_res {
                    Ok(status_res) => {
                        for status in status_res {
                            if status.callsign == plugin_name {
                                self.update_status(
                                    plugin_name.to_string(),
                                    status.to_plugin_state(),
                                );

                                if status.to_plugin_state().is_activated() {
                                    // get the pending BrokerRequest and process.
                                    let (pending_requests, expired) =
                                        self.retrive_pending_request(plugin_name.to_string());
                                    if pending_requests.len() > 0 {
                                        for pending_request in pending_requests {
                                            if expired {
                                                info!("Expired request: {:?}", pending_request);
                                                callback
                                                    .send_error(
                                                        pending_request,
                                                        RippleError::ServiceError,
                                                    )
                                                    .await;
                                            } else {
                                                let _ = sender.send(pending_request).await;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(_e) => {
                        Self::on_thunder_error_response(
                            self,
                            callback,
                            data,
                            &plugin_name.to_string(),
                        )
                        .await;
                    }
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
        if let Some(error) = &data.error {
            info!(
                "Error Received from Thunder on getting the status of the plugin : {:?}",
                error
            );
            if let Ok(error) = serde_json::from_value::<ThunderError>(error.clone()) {
                let state = error.get_plugin_state();
                self.update_status(plugin_name.to_string(), state.clone());
                if state.is_unavailable() {
                    // get the pending BrokerRequest and send error resposne.
                    let (pending_requests, _) =
                        self.retrive_pending_request(plugin_name.to_string());
                    if pending_requests.len() > 0 {
                        for pending_request in pending_requests {
                            callback
                                .send_error(pending_request, RippleError::ServiceError)
                                .await;
                        }
                    }
                }
            }
        }
    }

    pub fn get_request_from_active_plugins_request(&self, id: u64) -> Option<String> {
        let active_plugins_request = self.active_plugins_request.read().unwrap();
        active_plugins_request.get(&id).cloned()
    }

    pub async fn handle_controller_response(
        &self,
        sender: BrokerSender,
        callback: BrokerCallback,
        result: &[u8],
    ) {
        if let Ok(data) = serde_json::from_slice::<JsonRpcApiResponse>(result) {
            if let Some(id) = data.id {
                if let Some(request) = self.get_request_from_active_plugins_request(id) {
                    if request.contains("Controller.1.activate") {
                        // move this to spawned task
                        let state_mgr = StatusManager {
                            status: self.status.clone(),
                            active_plugins_request: self.active_plugins_request.clone(),
                        };
                        //let request_c = request.clone();
                        tokio::spawn(async move {
                            state_mgr
                                .on_activate_response(sender, callback, &data, &request)
                                .await;
                        });
                        //Self::on_activate_response(self, sender, callback, &data, request).await;
                    } else if request.contains("Controller.1.status@") {
                        // handle status response
                        Self::on_status_response(self, sender, callback, &data, &request).await;
                    } else if request.contains("Controller.1.register") {
                        // nothing to do here
                        info!("StatusManger Received response for register request");
                    }
                }
                let mut active_plugins_request = self.active_plugins_request.write().unwrap();
                active_plugins_request.remove(&id);
            }
        }
    }
}

// Add unit test for generate_state_change_subscribe_request
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_state_change_subscribe_request() {
        let status_manager = StatusManager::new();
        let request = status_manager.generate_state_change_subscribe_request();
        println!("Request: {}", request);
        let expected_request = r#"{"id":1,"jsonrpc":"2.0","method":"Controller.1.register","params":{"event":"statechange","id":"client.Controller.1.events"}}"#;
        assert_eq!(request, expected_request);
    }
}
