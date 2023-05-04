// If not stated otherwise in this file or this component's license file the
// following copyright and licenses apply:
//
// Copyright 2023 RDK Management
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

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use ripple_sdk::{
    api::{apps::AppEvent, device::device_operator::DeviceSubscribeRequest},
    log::{error, trace},
    serde_json::Value,
};

use crate::thunder_state::ThunderState;

#[derive(Debug, Clone)]
pub struct ThunderEventHandler {
    pub request: DeviceSubscribeRequest,
    pub handle: fn(state: ThunderState, value: Value),
    pub listeners: Vec<String>,
    pub id: String,
}

pub trait ThunderEventHandlerProvider {
    fn provide(id: String) -> ThunderEventHandler;
    fn module() -> String;
    fn event_name() -> String;
    fn get_mapped_event() -> String;
    fn get_id(&self) -> String {
        Self::get_mapped_event()
    }
    fn get_device_request() -> DeviceSubscribeRequest {
        DeviceSubscribeRequest {
            module: Self::event_name(),
            event_name: Self::module(),
            params: None,
            sub_id: Some(Self::get_mapped_event()),
        }
    }
}

impl ThunderEventHandler {
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    pub fn add_listener(&mut self, id: String) {
        self.listeners.push(id);
    }

    pub fn remove_listener(&mut self, id: String) -> bool {
        self.listeners.retain(|x| !x.eq(&id));
        self.listeners.len() == 0
    }

    pub fn send_app_event(state: ThunderState, event_name: String, result: Value) {
        if state.event_processor.check_last_event(&event_name, &result) {
            state.event_processor.add_last_event(&event_name, &result);
            if let Err(_) = state.get_client().request_transient(AppEvent {
                context: None,
                event_name,
                result,
            }) {
                error!("Error while forwarding app event");
            }
        } else {
            trace!("Already sent")
        }
    }
}

pub trait DeviceSubscribeRequestProvider {
    fn get_subscribe_request(&self) -> DeviceSubscribeRequest;
    fn get_handler(&self) -> fn(state: ThunderState, value: Value);
}

#[derive(Debug, Clone)]
pub struct ThunderEventProcessor {
    event_map: Arc<RwLock<HashMap<String, ThunderEventHandler>>>,
    last_event: Arc<RwLock<HashMap<String, Value>>>,
}

impl ThunderEventProcessor {
    pub fn new() -> ThunderEventProcessor {
        ThunderEventProcessor {
            event_map: Arc::new(RwLock::new(HashMap::new())),
            last_event: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn get_handler(&self, event: String) -> Option<ThunderEventHandler> {
        let event_map = self.event_map.read().unwrap();
        event_map.get(&event).cloned()
    }

    pub fn handle_listener(
        &self,
        listen: bool,
        app_id: String,
        handler: ThunderEventHandler,
    ) -> bool {
        if listen {
            self.add_event_listener(app_id, handler)
        } else {
            self.remove_event_listener(handler.get_id(), app_id)
        }
    }

    pub fn add_event_listener(&self, app_id: String, handler: ThunderEventHandler) -> bool {
        let event_name = handler.get_id();
        let mut event_map = self.event_map.write().unwrap();
        if let Some(entry) = event_map.get_mut(&event_name) {
            entry.add_listener(app_id);
            return false;
        } else {
            event_map.insert(event_name, handler);
        }
        true
    }

    pub fn remove_event_listener(&self, event_name: String, app_id: String) -> bool {
        let mut event_map = self.event_map.write().unwrap();
        if let Some(entry) = event_map.get_mut(&event_name) {
            if !entry.remove_listener(app_id) {
                return false;
            }
        }
        event_map.remove(&event_name);
        true
    }

    pub fn add_last_event(&self, event_name: &str, value: &Value) {
        let mut last_event_map = self.last_event.write().unwrap();
        last_event_map.insert(event_name.to_string(), value.clone());
    }

    pub fn check_last_event(&self, event_name: &str, value: &Value) -> bool {
        let last_event_map = self.last_event.read().unwrap();
        if let Some(last_event) = last_event_map.get(event_name) {
            return last_event.eq(value);
        }
        false
    }
}
