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
    api::mock_server::{MessagePayload, PayloadType, PayloadTypeError},
    log::error,
};
use serde_hashkey::{to_key_with_ordered_float, Key, OrderedFloatPolicy};
use serde_json::Value;

use crate::errors::{LoadMockDataError, MockDeviceError};

pub type MockDataKey = Key<OrderedFloatPolicy>;
pub type MockData = HashMap<MockDataKey, (MockDataMessage, Vec<MockDataMessage>)>;

#[derive(Clone, Debug, PartialEq)]
pub enum MockDataError {
    NotAnObject,
    MissingTypeProperty,
    MissingBodyProperty,
    PayloadTypeError(PayloadTypeError),
    MissingRequestField,
    MissingResponseField,
    FailedToCreateKey(Value),
}

impl std::error::Error for MockDataError {}

impl Display for MockDataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Self::MissingTypeProperty => "Message must have a type property.".to_owned(),
            Self::MissingBodyProperty => "Message must have a body property.".to_owned(),
            Self::PayloadTypeError(err) => format!("{err}"),
            Self::FailedToCreateKey(body) => {
                format!("Unable to create a key for message. Message body: {body}")
            }
            Self::MissingRequestField => "The request field is missing.".to_owned(),
            Self::MissingResponseField => "The response field is missing.".to_owned(),
            Self::NotAnObject => "Payload must be an object.".to_owned(),
        };

        f.write_str(msg.as_str())
    }
}

impl From<MockDataError> for MockDeviceError {
    fn from(err: MockDataError) -> Self {
        MockDeviceError::LoadMockDataFailed(LoadMockDataError::MockDataError(err))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MockDataMessage {
    pub message_type: PayloadType,
    pub body: Value,
}

impl From<MessagePayload> for MockDataMessage {
    fn from(value: MessagePayload) -> Self {
        Self {
            message_type: value.payload_type,
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
            message_type: message_type
                .try_into()
                .map_err(MockDataError::PayloadTypeError)?,
            body: message_body.clone(),
        })
    }
}

// TODO: should MockDataMessage be a trait?
impl MockDataMessage {
    pub fn key(&self) -> Result<MockDataKey, MockDataError> {
        match self.message_type {
            PayloadType::Json => json_key(&self.body),
            PayloadType::JsonRpc => jsonrpc_key(&self.body),
        }
    }

    pub fn is_json(&self) -> bool {
        matches!(self.message_type, PayloadType::Json)
    }

    pub fn is_jsonrpc(&self) -> bool {
        matches!(self.message_type, PayloadType::JsonRpc)
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

    #[test]
    fn test_json_key_ne_jsonrpc_key() {
        let value =
            json!({"jsonrpc": "2.0", "id": 1, "method": "someAction", "params": {"key": "value"}});

        assert_ne!(jsonrpc_key(&value), json_key(&value));
    }

    mod mock_data_message {
        use ripple_sdk::api::mock_server::{MessagePayload, PayloadType};
        use serde_json::json;

        use crate::mock_data::{json_key, jsonrpc_key, MockDataError, MockDataMessage};

        #[test]
        fn test_mock_message_is_json() {
            assert!(MockDataMessage {
                message_type: PayloadType::Json,
                body: json!({})
            }
            .is_json())
        }

        #[test]
        fn test_mock_message_is_jsonrpc() {
            assert!(MockDataMessage {
                message_type: PayloadType::JsonRpc,
                body: json!({})
            }
            .is_jsonrpc())
        }

        #[test]
        fn test_mock_message_from_message_payload_json() {
            let body = json!({"key": "value"});

            assert_eq!(
                MockDataMessage::from(MessagePayload {
                    payload_type: PayloadType::Json,
                    body: body.clone()
                }),
                MockDataMessage {
                    message_type: PayloadType::Json,
                    body
                }
            );
        }

        #[test]
        fn test_mock_message_from_message_payload_jsonrpc() {
            let body = json!({"jsonrpc": "2.0", "id": 2, "method": "someAction"});

            assert_eq!(
                MockDataMessage::from(MessagePayload {
                    payload_type: PayloadType::JsonRpc,
                    body: body.clone()
                }),
                MockDataMessage {
                    message_type: PayloadType::JsonRpc,
                    body
                }
            );
        }

        #[test]
        fn test_mock_message_try_from_ok_jsonrpc() {
            let body = json!({"jsonrpc": "2.0", "id": 2, "method": "someAction"});
            let value = json!({"type": "jsonrpc", "body": body});

            assert_eq!(
                MockDataMessage::try_from(&value),
                Ok(MockDataMessage {
                    message_type: PayloadType::JsonRpc,
                    body
                })
            );
        }

        #[test]
        fn test_mock_message_try_from_ok_json() {
            let body = json!({"jsonrpc": "2.0", "id": 2, "method": "someAction"});
            let value = json!({"type": "json", "body": body});

            assert_eq!(
                MockDataMessage::try_from(&value),
                Ok(MockDataMessage {
                    message_type: PayloadType::Json,
                    body
                })
            );
        }

        #[test]
        fn test_mock_message_try_from_err_missing_type() {
            assert_eq!(
                MockDataMessage::try_from(
                    &json!({"body": {"jsonrpc": "2.0", "id": 2, "method": "someAction"}})
                ),
                Err(MockDataError::MissingTypeProperty)
            );
        }

        #[test]
        fn test_mock_message_try_from_err_missing_body() {
            assert_eq!(
                MockDataMessage::try_from(&json!({"type": "jsonrpc"})),
                Err(MockDataError::MissingBodyProperty)
            );
        }

        #[test]
        fn test_mock_message_key_json() {
            let value = json!({"jsonrpc": "2.0", "id": 1, "method": "someAction"});
            let key = json_key(&value);
            assert_eq!(
                MockDataMessage {
                    message_type: PayloadType::Json,
                    body: value
                }
                .key(),
                key
            );
        }

        #[test]
        fn test_mock_message_key_jsonrpc() {
            let value = json!({"jsonrpc": "2.0", "id": 1, "method": "someAction"});
            let key = jsonrpc_key(&value);
            assert_eq!(
                MockDataMessage {
                    message_type: PayloadType::JsonRpc,
                    body: value
                }
                .key(),
                key
            );
        }
    }
}
