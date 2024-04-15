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

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use serde_json::json;

    #[rstest]
    #[case(DeviceChannelParams::Json(r#"{"key": "value"}"#.to_string()), true)]
    #[case(DeviceChannelParams::Literal("literal_value".to_string()), false)]
    #[case(DeviceChannelParams::Bool(true), false)]
    fn test_is_json(#[case] params: DeviceChannelParams, #[case] expected: bool) {
        assert_eq!(params.is_json(), expected);
    }

    #[rstest]
    #[case(DeviceChannelParams::Json(r#"{"key": "value"}"#.to_string()), r#"{"key": "value"}"#.to_string())]
    #[case(DeviceChannelParams::Literal("literal_value".to_string()), "literal_value".to_string())]
    #[case(DeviceChannelParams::Bool(true), "".to_string())]
    fn test_as_params(#[case] params: DeviceChannelParams, #[case] expected: String) {
        assert_eq!(params.as_params(), expected);
    }

    #[rstest]
    #[case(DeviceChannelParams::Json(r#"{"key": "value"}"#.to_string()), None)]
    #[case(DeviceChannelParams::Literal("literal_value".to_string()), None)]
    #[case(DeviceChannelParams::Bool(true), Some(Value::Bool(true)))]
    fn test_as_value(#[case] params: DeviceChannelParams, #[case] expected: Option<Value>) {
        assert_eq!(params.as_value(), expected);
    }

    #[test]
    fn test_call() {
        let message = json!({"key": "value"});
        let response = DeviceResponseMessage::call(message.clone());

        assert_eq!(response.message, message);
        assert_eq!(response.sub_id, None);
    }

    #[test]
    fn test_sub() {
        let message = json!({"key": "value"});
        let sub_id = "12345".to_string();
        let response = DeviceResponseMessage::sub(message.clone(), sub_id.clone());

        assert_eq!(response.message, message);
        assert_eq!(response.sub_id, Some(sub_id));
    }
}
