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
use crate::utils::error::RippleError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt::Debug;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Id {
    Number(i64),
    String(String),
    Null,
}
impl Id {
    pub fn is_null(&self) -> bool {
        matches!(self, Id::Null)
    }
    pub fn get_number(&self) -> Option<i64> {
        if let Id::Number(n) = self {
            Some(*n)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    pub id: Id,
}
// implment fmt for JsonRpcRequest
impl std::fmt::Display for JsonRpcRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "JsonRpcRequest {{ jsonrpc: {}, method: {}, params: {:?}, id: {:?} }}",
            self.jsonrpc, self.method, self.params, self.id
        )
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcSuccess {
    pub jsonrpc: String,
    pub result: Value,
    pub id: Id,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcErrorDetails {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub jsonrpc: String,
    pub error: JsonRpcErrorDetails,
    pub id: Id,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcMessage {
    Request(JsonRpcRequest),
    Notification(JsonRpcNotification),
    Success(JsonRpcSuccess),
    Error(JsonRpcError),
}

// convert JsonRpcMessage to String
impl From<JsonRpcMessage> for String {
    fn from(val: JsonRpcMessage) -> Self {
        serde_json::to_string(&val).unwrap()
    }
}

// set the id for JsonRpcMessage
impl JsonRpcMessage {
    pub fn set_id(&mut self, id: Id) {
        match self {
            JsonRpcMessage::Request(req) => req.id = id,
            JsonRpcMessage::Notification(_) => {}
            JsonRpcMessage::Success(success) => success.id = id,
            JsonRpcMessage::Error(err) => err.id = id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceMessage {
    // #[serde(flatten)] Enable this once we stop supporting ExtnMessage
    pub message: JsonRpcMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,
}

// implement fmt for ServiceMessage
impl std::fmt::Display for ServiceMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ServiceMessage {{ message: {:?}, context: {:?} }}",
            self.message, self.context
        )
    }
}

// Custom deserializer for ServiceMessage is required due to the flattern field
// this helps to distinguish between request and notification messages
// Enable this once we stop supporting ExtnMessage
/*
impl<'de> Deserialize<'de> for ServiceMessage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // parse the input into a map
        let mut map: Map<String, Value> = Deserialize::deserialize(deserializer)?;

        // extract the context if it exists
        let context = map.remove("context");

        let json_value = Value::Object(map.clone());

        // extract the message type
        let message = if map.contains_key("result") {
            serde_json::from_value::<JsonRpcSuccess>(json_value).map(JsonRpcMessage::Success)
        } else if map.contains_key("error") {
            serde_json::from_value::<JsonRpcError>(json_value).map(JsonRpcMessage::Error)
        } else if map.contains_key("id") {
            serde_json::from_value::<JsonRpcRequest>(json_value).map(JsonRpcMessage::Request)
        } else {
            serde_json::from_value::<JsonRpcNotification>(json_value)
                .map(JsonRpcMessage::Notification)
        }
        .map_err(|_| serde::de::Error::custom("Failed to parse JsonRpcMessage"))?;

        Ok(ServiceMessage { message, context })
    }
}
*/
impl TryFrom<&str> for ServiceMessage {
    type Error = RippleError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        serde_json::from_str(value).map_err(|_| RippleError::ParseError)
    }
}

impl From<ServiceMessage> for String {
    fn from(val: ServiceMessage) -> Self {
        serde_json::to_string(&val).unwrap()
    }
}

impl ServiceMessage {
    pub fn new_request(method: String, params: Option<Value>, id: Id) -> Self {
        ServiceMessage {
            message: JsonRpcMessage::Request(JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                method,
                params,
                id,
            }),
            context: None,
        }
    }

    pub fn new_notification(method: String, params: Option<Value>) -> Self {
        ServiceMessage {
            message: JsonRpcMessage::Notification(JsonRpcNotification {
                jsonrpc: "2.0".to_string(),
                method,
                params,
            }),
            context: None,
        }
    }

    pub fn new_success(result: Value, id: Id) -> Self {
        ServiceMessage {
            message: JsonRpcMessage::Success(JsonRpcSuccess {
                jsonrpc: "2.0".to_string(),
                result,
                id,
            }),
            context: None,
        }
    }

    pub fn new_error(code: i64, message: String, data: Option<Value>, id: Id) -> Self {
        ServiceMessage {
            message: JsonRpcMessage::Error(JsonRpcError {
                jsonrpc: "2.0".to_string(),
                error: JsonRpcErrorDetails {
                    code,
                    message,
                    data,
                },
                id,
            }),
            context: None,
        }
    }

    pub fn set_context(&mut self, context: Option<Value>) {
        self.context = context;
    }

    // get the request id from the message
    pub fn get_request_id(&self) -> u64 {
        match &self.message {
            JsonRpcMessage::Request(req) => {
                if let Id::Number(id) = req.id {
                    id as u64
                } else {
                    0
                }
            }
            JsonRpcMessage::Notification(_) => 0,
            JsonRpcMessage::Success(success) => {
                if let Id::Number(id) = success.id {
                    id as u64
                } else {
                    0
                }
            }
            JsonRpcMessage::Error(err) => {
                if let Id::Number(id) = err.id {
                    id as u64
                } else {
                    0
                }
            }
        }
    }
}
