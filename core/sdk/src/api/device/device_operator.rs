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
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;

pub const DEFAULT_DEVICE_OPERATION_TIMEOUT_SECS: u64 = 5;

/// Generic Operator trait used for Device Communications
/// Each Device platform should implement this trait based on the underlying service
#[async_trait]
pub trait DeviceOperator: Clone {
    async fn call(&self, request: DeviceCallRequest) -> DeviceResponseMessage;

    async fn subscribe(
        &self,
        request: DeviceSubscribeRequest,
        handler: mpsc::Sender<DeviceResponseMessage>,
    ) -> DeviceResponseMessage;

    async fn unsubscribe(&self, request: DeviceUnsubscribeRequest);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceChannelRequest {
    Call(DeviceCallRequest),
    Subscribe(DeviceSubscribeRequest),
    Unsubscribe(DeviceUnsubscribeRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCallRequest {
    pub method: String,
    pub params: Option<DeviceChannelParams>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceSubscribeRequest {
    pub module: String,
    pub event_name: String,
    pub params: Option<String>,
    pub sub_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceUnsubscribeRequest {
    pub module: String,
    pub event_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceChannelParams {
    Json(String),
    Literal(String),
    Bool(bool),
}

impl DeviceChannelParams {
    pub fn as_params(&self) -> String {
        match self {
            DeviceChannelParams::Json(json) => json.clone(),
            DeviceChannelParams::Literal(lit) => lit.clone(),
            DeviceChannelParams::Bool(_) => String::from(""),
        }
    }

    pub fn as_value(&self) -> Option<Value> {
        match self {
            DeviceChannelParams::Bool(b) => Some(Value::Bool(*b)),
            _ => None,
        }
    }

    pub fn is_json(&self) -> bool {
        match self {
            DeviceChannelParams::Json(_) => true,
            DeviceChannelParams::Literal(_) => false,
            DeviceChannelParams::Bool(_) => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceResponseMessage {
    pub message: Value,
    pub sub_id: Option<String>,
}

impl DeviceResponseMessage {
    pub fn call(message: Value) -> DeviceResponseMessage {
        DeviceResponseMessage {
            message,
            sub_id: None,
        }
    }

    pub fn sub(message: Value, sub_id: String) -> DeviceResponseMessage {
        DeviceResponseMessage {
            message,
            sub_id: Some(sub_id),
        }
    }
}
