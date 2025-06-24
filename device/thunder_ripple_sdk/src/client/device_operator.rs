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
    async_trait::async_trait,
    log::error,
    tokio::sync::{mpsc, oneshot::error::RecvError},
};
use serde_json::Value;

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
    ) -> Result<DeviceResponseMessage, RecvError>;

    //async fn unsubscribe(&self, request: DeviceUnsubscribeRequest); //not used anywhere commented out for now.
}

#[derive(Debug, Clone)]
pub struct DeviceResponseSubscription {
    pub sub_id: Option<String>,
    pub handlers: Vec<mpsc::Sender<DeviceResponseMessage>>,
}

#[derive(Debug, Clone)]
pub enum DeviceChannelRequest {
    Call(DeviceCallRequest),
    Subscribe(DeviceSubscribeRequest),
    Unsubscribe(DeviceUnsubscribeRequest),
}

impl DeviceChannelRequest {
    pub fn get_callsign_method(&self) -> (String, String) {
        match self {
            DeviceChannelRequest::Call(c) => {
                let mut collection: Vec<&str> = c.method.split('.').collect();
                let method = collection.pop().unwrap_or_default();

                // Check if the second-to-last element is a digit (version number)
                if let Some(&version) = collection.last() {
                    if version.chars().all(char::is_numeric) {
                        collection.pop(); // Remove the version number
                    }
                }

                let callsign = collection.join(".");
                (callsign, method.into())
            }
            DeviceChannelRequest::Subscribe(s) => {
                let mut parts: Vec<&str> = s.module.split('.').collect();
                if let Some(&last) = parts.last() {
                    if last.chars().all(char::is_numeric) {
                        parts.pop(); // Remove the version number
                    }
                }
                let module_name = parts.join(".");
                (module_name, s.event_name.clone())
            }
            DeviceChannelRequest::Unsubscribe(u) => {
                let mut parts: Vec<&str> = u.module.split('.').collect();
                if let Some(&last) = parts.last() {
                    if last.chars().all(char::is_numeric) {
                        parts.pop(); // Remove the version number
                    }
                }
                let module_name = parts.join(".");
                (module_name, u.event_name.clone())
            }
        }
    }

    pub fn get_dev_call_request(&self) -> Option<DeviceCallRequest> {
        if let DeviceChannelRequest::Call(c) = self {
            Some(c.clone())
        } else {
            None
        }
    }

    pub fn get_dev_subscribe_request(&self) -> Option<DeviceSubscribeRequest> {
        if let DeviceChannelRequest::Subscribe(s) = self {
            Some(s.clone())
        } else {
            None
        }
    }

    pub fn get_dev_unsubscribe_request(&self) -> Option<DeviceUnsubscribeRequest> {
        if let DeviceChannelRequest::Unsubscribe(u) = self {
            Some(u.clone())
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeviceCallRequest {
    pub method: String,
    pub params: Option<DeviceChannelParams>,
}

#[derive(Debug, Clone)]
pub struct DeviceSubscribeRequest {
    pub module: String,
    pub event_name: String,
    pub params: Option<String>,
    pub sub_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeviceUnsubscribeRequest {
    pub module: String,
    pub event_name: String,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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

    pub fn new(message: Value, sub_id: Option<String>) -> DeviceResponseMessage {
        DeviceResponseMessage { message, sub_id }
    }

    pub fn create(
        json_resp: &JsonRpcApiResponse,
        sub_id: Option<String>,
    ) -> Option<DeviceResponseMessage> {
        let mut device_response_msg = None;
        if let Some(res) = &json_resp.result {
            device_response_msg = Some(DeviceResponseMessage::new(res.clone(), sub_id));
        } else if let Some(er) = &json_resp.error {
            device_response_msg = Some(DeviceResponseMessage::new(er.clone(), sub_id));
        } else if json_resp.clone().method.is_some() {
            if let Some(params) = &json_resp.params {
                if let Ok(dev_resp) = serde_json::to_value(params) {
                    device_response_msg = Some(DeviceResponseMessage::new(dev_resp, sub_id));
                }
            }
        } else {
            error!("deviceresponse msg extraction failed.");
        }
        device_response_msg
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

    #[test]
    fn test_get_callsign_method() {
        let call_request = DeviceChannelRequest::Call(DeviceCallRequest {
            method: "org.rdk.System.1.testMethod".to_string(),
            params: None,
        });
        let (callsign, method) = call_request.get_callsign_method();
        assert_eq!(callsign, "org.rdk.System");
        assert_eq!(method, "testMethod");

        let call_request = DeviceChannelRequest::Call(DeviceCallRequest {
            method: "org.rdk.System.testMethod".to_string(),
            params: None,
        });
        let (callsign, method) = call_request.get_callsign_method();
        assert_eq!(callsign, "org.rdk.System");
        assert_eq!(method, "testMethod");

        let call_request = DeviceChannelRequest::Call(DeviceCallRequest {
            method: "org.testMethod".to_string(),
            params: None,
        });
        let (callsign, method) = call_request.get_callsign_method();
        assert_eq!(callsign, "org");
        assert_eq!(method, "testMethod");

        // subscribe request
        let subscribe_request = DeviceChannelRequest::Subscribe(DeviceSubscribeRequest {
            module: "org.rdk.System.1".to_string(),
            event_name: "onPowerStateEvent".to_string(),
            params: None,
            sub_id: None,
        });
        let (callsign, method) = subscribe_request.get_callsign_method();
        assert_eq!(callsign, "org.rdk.System");
        assert_eq!(method, "onPowerStateEvent");

        let subscribe_request = DeviceChannelRequest::Subscribe(DeviceSubscribeRequest {
            module: "org.rdk.System".to_string(),
            event_name: "onPowerStateEvent".to_string(),
            params: None,
            sub_id: None,
        });
        let (callsign, method) = subscribe_request.get_callsign_method();
        assert_eq!(callsign, "org.rdk.System");
        assert_eq!(method, "onPowerStateEvent");

        let subscribe_request = DeviceChannelRequest::Subscribe(DeviceSubscribeRequest {
            module: "org".to_string(),
            event_name: "onPowerStateEvent".to_string(),
            params: None,
            sub_id: None,
        });
        let (callsign, method) = subscribe_request.get_callsign_method();
        assert_eq!(callsign, "org");
        assert_eq!(method, "onPowerStateEvent");

        // unsubscribe request
        let unsubscribe_request = DeviceChannelRequest::Unsubscribe(DeviceUnsubscribeRequest {
            module: "org.rdk.AuthService.1".to_string(),
            event_name: "onTestEvent".to_string(),
        });
        let (callsign, method) = unsubscribe_request.get_callsign_method();
        assert_eq!(callsign, "org.rdk.AuthService");
        assert_eq!(method, "onTestEvent");

        let unsubscribe_request = DeviceChannelRequest::Unsubscribe(DeviceUnsubscribeRequest {
            module: "org".to_string(),
            event_name: "onTestEvent".to_string(),
        });
        let (callsign, method) = unsubscribe_request.get_callsign_method();
        assert_eq!(callsign, "org");
        assert_eq!(method, "onTestEvent");
    }
}
