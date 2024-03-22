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

use std::fmt::Display;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::mock_data::MockData;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum PayloadTypeError {
    InvalidMessageType,
}

impl Display for PayloadTypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidMessageType => {
                f.write_str("Invalid message type. Possible values are: json, jsonrpc")
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum PayloadType {
    #[serde(rename = "json")]
    Json,
    #[serde(rename = "jsonrpc")]
    JsonRpc,
}

impl Display for PayloadType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&String::from(self))
    }
}

impl From<&PayloadType> for String {
    fn from(val: &PayloadType) -> Self {
        match val {
            PayloadType::Json => "json".to_string(),
            PayloadType::JsonRpc => "jsonrpc".to_string(),
        }
    }
}

impl TryFrom<&str> for PayloadType {
    type Error = PayloadTypeError;

    fn try_from(val: &str) -> Result<Self, Self::Error> {
        match val {
            "json" => Ok(PayloadType::Json),
            "jsonrpc" => Ok(PayloadType::JsonRpc),
            _ => Err(PayloadTypeError::InvalidMessageType),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MessagePayload {
    /// The type of payload data
    #[serde(rename = "type")]
    pub payload_type: PayloadType,
    /// The body of the request
    pub body: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EventPayload {
    // TODO: wrap around MessagePayload
    /// The body of the event
    pub body: Value,
    /// The number of msecs before the event should be emitted
    pub delay: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MockServerRequest {
    EmitEvent(EmitEventParams),
    AddRequestResponse(MockData),
    RemoveRequestResponse(MockData),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum MockServerResponse {
    AddRequestResponse(AddRequestResponseResponse),
    EmitEvent(EmitEventResponse),
    RemoveRequestResponse(RemoveRequestResponse),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AddRequestResponseParams {
    pub request: MessagePayload,
    pub responses: Vec<MessagePayload>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct AddRequestResponseResponse {
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RemoveRequestParams {
    pub request: MessagePayload,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct RemoveRequestResponse {
    pub success: bool,
    pub error: Option<String>,
}

// TODO: add a clear all mock data request

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EmitEventParams {
    pub event: EventPayload,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct EmitEventResponse {
    pub success: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_type_try_from_str_json() {
        assert_eq!(PayloadType::try_from("json"), Ok(PayloadType::Json));
    }

    #[test]
    fn test_message_type_try_from_str_jsonrpc() {
        assert_eq!(PayloadType::try_from("jsonrpc"), Ok(PayloadType::JsonRpc));
    }

    #[test]
    fn test_message_type_try_from_str_err() {
        assert_eq!(
            PayloadType::try_from("unknown"),
            Err(PayloadTypeError::InvalidMessageType)
        );
    }

    #[test]
    fn test_message_type_to_string_json() {
        assert_eq!(PayloadType::Json.to_string(), "json".to_owned());
        assert_eq!(String::from(&PayloadType::Json), "json".to_owned());
    }

    #[test]
    fn test_message_type_to_string_jsonrpc() {
        assert_eq!(PayloadType::JsonRpc.to_string(), "jsonrpc".to_owned());
        assert_eq!(String::from(&PayloadType::JsonRpc), "jsonrpc".to_owned());
    }
}
