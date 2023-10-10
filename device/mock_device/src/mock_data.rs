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

use std::{collections::HashMap, fmt::Display};

use ripple_sdk::{
    api::mock_server::{PayloadType, RequestPayload, ResponsePayload},
    log::error,
};
use serde_hashkey::{to_key_with_ordered_float, Key, OrderedFloatPolicy};
use serde_json::Value;

pub type MockDataKey = Key<OrderedFloatPolicy>;
pub type MockData = HashMap<MockDataKey, (MockDataMessage, Vec<MockDataMessage>)>;

#[derive(Clone, Debug, PartialEq)]
pub enum MockDataError {
    NotAnObject,
    MissingTypeProperty,
    MissingBodyProperty,
    InvalidMessageType,
    MissingRequestField,
    MissingResponseField,
    FailedToCreateKey(Value),
}

impl Display for MockDataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingTypeProperty => {
                f.write_fmt(format_args!("Message must have a type property."))
            }
            Self::MissingBodyProperty => {
                f.write_fmt(format_args!("Message must have a body property."))
            }
            Self::InvalidMessageType => f.write_fmt(format_args!(
                "Message type not recognised. Valid types: json, jsonrpc"
            )),
            Self::FailedToCreateKey(body) => f.write_fmt(format_args!(
                "Unable to create a key for message. Message body: {body}"
            )),
            Self::MissingRequestField => f.write_str("The request field is missing."),
            Self::MissingResponseField => f.write_str("The response field is missing."),
            Self::NotAnObject => f.write_str("Payload must be an object."),
        }
    }
}

#[derive(Clone, Debug)]
pub enum MessageType {
    Json,
    JsonRpc,
}

impl From<MessageType> for String {
    fn from(val: MessageType) -> Self {
        match val {
            MessageType::Json => "json".to_string(),
            MessageType::JsonRpc => "jsonrpc".to_string(),
        }
    }
}

impl TryFrom<&str> for MessageType {
    type Error = MockDataError;

    fn try_from(val: &str) -> Result<Self, Self::Error> {
        match val {
            "json" => Ok(MessageType::Json),
            "jsonrpc" => Ok(MessageType::JsonRpc),
            _ => Err(MockDataError::InvalidMessageType),
        }
    }
}

#[derive(Clone, Debug)]
pub struct MockDataMessage {
    pub message_type: MessageType,

    pub body: Value,
}

impl From<RequestPayload> for MockDataMessage {
    fn from(value: RequestPayload) -> Self {
        Self {
            message_type: match value.payload_type {
                PayloadType::Json => MessageType::Json,
                PayloadType::JsonRpc => MessageType::JsonRpc,
            },
            body: value.body,
        }
    }
}

impl From<ResponsePayload> for MockDataMessage {
    fn from(value: ResponsePayload) -> Self {
        Self {
            message_type: match value.payload_type {
                PayloadType::Json => MessageType::Json,
                PayloadType::JsonRpc => MessageType::JsonRpc,
            },
            body: value.body,
        }
    }
}

impl TryFrom<&Value> for MockDataMessage {
    type Error = MockDataError;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let message_type = value
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or(MockDataError::MissingTypeProperty)?;
        let message_body = value
            .get("body")
            .ok_or(MockDataError::MissingBodyProperty)?;

        Ok(MockDataMessage {
            message_type: message_type.try_into()?,
            body: message_body.clone(),
        })
    }
}

impl MockDataMessage {
    pub fn key(&self) -> Result<MockDataKey, MockDataError> {
        match self.message_type {
            MessageType::Json => json_key(&self.body),
            MessageType::JsonRpc => jsonrpc_key(&self.body),
        }
    }

    pub fn is_json(&self) -> bool {
        matches!(self.message_type, MessageType::Json)
    }

    pub fn is_json_rpc(&self) -> bool {
        matches!(self.message_type, MessageType::JsonRpc)
    }
}

pub fn json_key(value: &Value) -> Result<MockDataKey, MockDataError> {
    to_key_with_ordered_float(value).map_err(|_| {
        error!("Failed to create key from data {value:?}");
        MockDataError::FailedToCreateKey(value.clone())
    })
}

pub fn jsonrpc_key(value: &Value) -> Result<MockDataKey, MockDataError> {
    let mut new_value = value.clone();
    new_value
        .as_object_mut()
        .and_then(|payload| payload.remove("id"));

    json_key(&new_value)
}

#[cfg(test)]
mod tests {
    use serde_hashkey::{Float, OrderedFloat};
    use serde_json::json;

    use super::*;

    #[test]
    fn test_json_key_ok() {
        let value = json!({"key": "value"});

        assert_eq!(
            json_key(&value),
            Ok(MockDataKey::Map(Box::new([(
                MockDataKey::String("key".into()),
                MockDataKey::String("value".into())
            )])))
        );
    }

    #[test]
    fn test_json_key_f64_ok() {
        let value = json!({"key": 32.1});

        assert_eq!(
            json_key(&value),
            Ok(MockDataKey::Map(Box::new([(
                MockDataKey::String("key".into()),
                MockDataKey::Float(Float::F64(OrderedFloat(32.1)))
            )])))
        );
    }

    #[test]
    fn test_jsonrpc_key() {
        let value =
            json!({"jsonrpc": "2.0", "id": 1, "method": "someAction", "params": {"key": "value"}});

        assert_eq!(
            jsonrpc_key(&value),
            Ok(MockDataKey::Map(Box::new([
                (
                    MockDataKey::String("jsonrpc".into()),
                    MockDataKey::String("2.0".into())
                ),
                (
                    MockDataKey::String("method".into()),
                    MockDataKey::String("someAction".into())
                ),
                (
                    MockDataKey::String("params".into()),
                    MockDataKey::Map(Box::new([(
                        MockDataKey::String("key".into()),
                        MockDataKey::String("value".into())
                    )]))
                )
            ])))
        );
    }

    mod mock_data_message {
        use serde_json::json;

        use crate::mock_data::{MessageType, MockDataMessage};

        #[test]
        fn test_mock_message_is_json() {
            assert!(MockDataMessage {
                message_type: MessageType::Json,
                body: json!({})
            }
            .is_json())
        }

        #[test]
        fn test_mock_message_is_json_rpc() {
            assert!(MockDataMessage {
                message_type: MessageType::JsonRpc,
                body: json!({})
            }
            .is_json_rpc())
        }
    }
}
