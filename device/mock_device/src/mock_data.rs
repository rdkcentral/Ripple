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

use ripple_sdk::log::error;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashMap, fmt::Display};

use crate::{
    errors::{LoadMockDataError, MockDeviceError},
    mock_server::{MessagePayload, PayloadType, PayloadTypeError},
};

pub type MockData = HashMap<String, Vec<ParamResponse>>;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ParamResponse {
    pub params: Option<Value>,
    pub result: Option<Value>,
    pub error: Option<Value>,
    pub events: Option<Vec<EventValue>>,
}

#[derive(Debug)]
pub struct ResponseSink {
    pub delay: u64,
    pub data: Value,
}

impl ParamResponse {
    pub fn get_key(&self, key: &Value) -> Option<Self> {
        match &self.params {
            Some(v) => {
                if v.eq(key) {
                    return Some(self.clone());
                }
                None
            }
            None => None,
        }
    }
    pub fn get_notification_id(&self) -> Option<String> {
        if let Some(params) = &self.params {
            if let Some(event) = params.get("event") {
                if let Some(id) = params.get("id") {
                    return Some(format!(
                        "{}.{}",
                        id.to_string().replace('\"', ""),
                        event.to_string().replace('\"', "")
                    ));
                }
            }
        }
        None
    }

    pub fn get_all(&self, id: Option<u64>) -> Vec<ResponseSink> {
        let mut sink_responses = Vec::new();
        if let Some(v) = self.result.clone() {
            sink_responses.push(ResponseSink {
                delay: 0,
                data: json!({"jsonrpc": "2.0", "id": id, "result": v}),
            });

            if let Some(events) = &self.events {
                let notif_id = self.get_notification_id();
                error!("Getting notif id {:?}", notif_id);
                for event in events {
                    sink_responses.push(ResponseSink {
                        delay: event.delay.unwrap_or(0),
                        data: json!({"jsonrpc": "2.0", "method": notif_id, "params": event.data.clone()})
                    })
                }
            }
        }
        if let Some(e) = self.error.clone() {
            sink_responses.push(ResponseSink {
                delay: 0,
                data: json!({"jsonrpc": "2.0", "id": id, "error": [e]}),
            });
        }
        sink_responses
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EventValue {
    pub delay: Option<u64>,
    pub data: Value,
}

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
    pub fn is_json(&self) -> bool {
        matches!(self.message_type, PayloadType::Json)
    }

    pub fn is_jsonrpc(&self) -> bool {
        matches!(self.message_type, PayloadType::JsonRpc)
    }
}

// #[cfg(test)]
// mod tests {
//     use serde_hashkey::{Float, OrderedFloat};
//     use serde_json::json;

//     use super::*;

//     #[test]
//     fn test_json_key_ok() {
//         let value = json!({"key": "value"});

//         assert_eq!(
//             json_key(&value),
//             Ok(MockDataKey::Map(Box::new([(
//                 MockDataKey::String("key".into()),
//                 MockDataKey::String("value".into())
//             )])))
//         );
//     }

//     #[test]
//     fn test_json_key_f64_ok() {
//         let value = json!({"key": 32.1});

//         assert_eq!(
//             json_key(&value),
//             Ok(MockDataKey::Map(Box::new([(
//                 MockDataKey::String("key".into()),
//                 MockDataKey::Float(Float::F64(OrderedFloat(32.1)))
//             )])))
//         );
//     }

//     #[test]
//     fn test_jsonrpc_key() {
//         let value =
//             json!({"jsonrpc": "2.0", "id": 1, "method": "someAction", "params": {"key": "value"}});

//         assert_eq!(
//             jsonrpc_key(&value),
//             Ok(MockDataKey::Map(Box::new([
//                 (
//                     MockDataKey::String("jsonrpc".into()),
//                     MockDataKey::String("2.0".into())
//                 ),
//                 (
//                     MockDataKey::String("method".into()),
//                     MockDataKey::String("someAction".into())
//                 ),
//                 (
//                     MockDataKey::String("params".into()),
//                     MockDataKey::Map(Box::new([(
//                         MockDataKey::String("key".into()),
//                         MockDataKey::String("value".into())
//                     )]))
//                 )
//             ])))
//         );
//     }

//     #[test]
//     fn test_json_key_ne_jsonrpc_key() {
//         let value =
//             json!({"jsonrpc": "2.0", "id": 1, "method": "someAction", "params": {"key": "value"}});

//         assert_ne!(jsonrpc_key(&value), json_key(&value));
//     }

//     mod mock_data_message {
//         use crate::mock_server::{MessagePayload, PayloadType};
//         use serde_json::json;

//         use crate::mock_data::{json_key, jsonrpc_key, MockDataError, MockDataMessage};

//         #[test]
//         fn test_mock_message_is_json() {
//             assert!(MockDataMessage {
//                 message_type: PayloadType::Json,
//                 body: json!({})
//             }
//             .is_json())
//         }

//         #[test]
//         fn test_mock_message_is_jsonrpc() {
//             assert!(MockDataMessage {
//                 message_type: PayloadType::JsonRpc,
//                 body: json!({})
//             }
//             .is_jsonrpc())
//         }

//         #[test]
//         fn test_mock_message_from_message_payload_json() {
//             let body = json!({"key": "value"});

//             assert_eq!(
//                 MockDataMessage::from(MessagePayload {
//                     payload_type: PayloadType::Json,
//                     body: body.clone()
//                 }),
//                 MockDataMessage {
//                     message_type: PayloadType::Json,
//                     body
//                 }
//             );
//         }

//         #[test]
//         fn test_mock_message_from_message_payload_jsonrpc() {
//             let body = json!({"jsonrpc": "2.0", "id": 2, "method": "someAction"});

//             assert_eq!(
//                 MockDataMessage::from(MessagePayload {
//                     payload_type: PayloadType::JsonRpc,
//                     body: body.clone()
//                 }),
//                 MockDataMessage {
//                     message_type: PayloadType::JsonRpc,
//                     body
//                 }
//             );
//         }

//         #[test]
//         fn test_mock_message_try_from_ok_jsonrpc() {
//             let body = json!({"jsonrpc": "2.0", "id": 2, "method": "someAction"});
//             let value = json!({"type": "jsonrpc", "body": body});

//             assert_eq!(
//                 MockDataMessage::try_from(&value),
//                 Ok(MockDataMessage {
//                     message_type: PayloadType::JsonRpc,
//                     body
//                 })
//             );
//         }

//         #[test]
//         fn test_mock_message_try_from_ok_json() {
//             let body = json!({"jsonrpc": "2.0", "id": 2, "method": "someAction"});
//             let value = json!({"type": "json", "body": body});

//             assert_eq!(
//                 MockDataMessage::try_from(&value),
//                 Ok(MockDataMessage {
//                     message_type: PayloadType::Json,
//                     body
//                 })
//             );
//         }

//         #[test]
//         fn test_mock_message_try_from_err_missing_type() {
//             assert_eq!(
//                 MockDataMessage::try_from(
//                     &json!({"body": {"jsonrpc": "2.0", "id": 2, "method": "someAction"}})
//                 ),
//                 Err(MockDataError::MissingTypeProperty)
//             );
//         }

//         #[test]
//         fn test_mock_message_try_from_err_missing_body() {
//             assert_eq!(
//                 MockDataMessage::try_from(&json!({"type": "jsonrpc"})),
//                 Err(MockDataError::MissingBodyProperty)
//             );
//         }

//         #[test]
//         fn test_mock_message_key_json() {
//             let value = json!({"jsonrpc": "2.0", "id": 1, "method": "someAction"});
//             let key = json_key(&value);
//             assert_eq!(
//                 MockDataMessage {
//                     message_type: PayloadType::Json,
//                     body: value
//                 }
//                 .key(),
//                 key
//             );
//         }

//         #[test]
//         fn test_mock_message_key_jsonrpc() {
//             let value = json!({"jsonrpc": "2.0", "id": 1, "method": "someAction"});
//             let key = jsonrpc_key(&value);
//             assert_eq!(
//                 MockDataMessage {
//                     message_type: PayloadType::JsonRpc,
//                     body: value
//                 }
//                 .key(),
//                 key
//             );
//         }
//     }
// }
