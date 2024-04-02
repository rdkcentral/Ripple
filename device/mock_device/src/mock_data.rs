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

use ripple_sdk::log::{debug, error};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashMap, fmt::Display};

use crate::{
    errors::{LoadMockDataError, MockDeviceError},
    mock_server::{MessagePayload, PayloadType, PayloadTypeError},
    mock_web_socket_server::ThunderRegisterParams,
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
            None => Some(self.clone()),
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

    pub fn get_all(
        &self,
        id: Option<u64>,
        thunder_response: Option<ThunderRegisterParams>,
    ) -> Vec<ResponseSink> {
        let mut sink_responses = Vec::new();
        if let Some(e) = self.error.clone() {
            sink_responses.push(ResponseSink {
                delay: 0,
                data: json!({"jsonrpc": "2.0", "id": id, "error": [e]}),
            });
        } else if let Some(v) = self.result.clone() {
            sink_responses.push(ResponseSink {
                delay: 0,
                data: json!({"jsonrpc": "2.0", "id": id, "result": v}),
            });

            if let Some(events) = &self.events {
                let notif_id = if let Some(t) = thunder_response {
                    Some(format!("{}.{}", t.id, t.event))
                } else {
                    self.get_notification_id()
                };

                error!("Getting notif id {:?}", notif_id);
                for event in events {
                    sink_responses.push(ResponseSink {
                        delay: event.delay.unwrap_or(0),
                        data: json!({"jsonrpc": "2.0", "method": notif_id, "params": event.data.clone()})
                    })
                }
            }
        } else {
            sink_responses.push(ResponseSink {
                delay: 0,
                data: json!({"jsonrpc": "2.0", "id": id, "result": null}),
            });
        }
        debug!("Total sink responses {:?}", sink_responses);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_param_response_get_key() {
        let response = ParamResponse {
            result: None,
            error: None,
            events: None,
            params: None,
        };
        assert!(response.get_key(&Value::Null).is_some());
        let response = ParamResponse {
            result: None,
            error: None,
            events: None,
            params: Some(Value::String("Some".to_owned())),
        };
        assert!(response.get_key(&Value::Null).is_none());
        assert!(response
            .get_key(&Value::String("Some".to_owned()))
            .is_some());
    }

    #[test]
    fn test_param_response_get_notif_id() {
        let response = ParamResponse {
            result: None,
            error: None,
            events: None,
            params: None,
        };
        assert!(response.get_notification_id().is_none());
        let response = ParamResponse {
            result: None,
            error: None,
            events: None,
            params: Some(Value::String("Some".to_owned())),
        };
        assert!(response.get_notification_id().is_none());

        let response = ParamResponse {
            result: None,
            error: None,
            events: None,
            params: Some(json!({
                "event": "SomeEvent",
                "id": "SomeId"
            })),
        };

        assert!(response
            .get_notification_id()
            .unwrap()
            .eq("SomeId.SomeEvent"));
    }

    #[test]
    fn test_get_all() {
        let pr = ParamResponse {
            result: None,
            error: Some(json!({"code": -32010, "message": "Error Message"})),
            events: None,
            params: None,
        };
        let response = pr.get_all(Some(0), None)[0]
            .data
            .get("error")
            .unwrap()
            .as_array()
            .unwrap()[0]
            .get("code")
            .unwrap()
            .as_i64()
            .unwrap();
        assert!(response.eq(&-32010));

        let pr = ParamResponse {
            result: Some(json!({"code": 0})),
            error: None,
            events: Some(vec![EventValue {
                delay: Some(0),
                data: json!({"event": 0}),
            }]),
            params: None,
        };

        let response = pr.get_all(Some(0), None)[0]
            .data
            .get("result")
            .unwrap()
            .get("code")
            .unwrap()
            .as_i64()
            .unwrap();
        assert!(response.eq(&0));

        let event_value = pr.get_all(Some(1), None)[1]
            .data
            .get("params")
            .unwrap()
            .get("event")
            .unwrap()
            .as_i64()
            .unwrap();
        assert!(event_value.eq(&0));
        let params = ThunderRegisterParams {
            event: "SomeEvent".to_owned(),
            id: "SomeId".to_owned(),
        };
        if let Some(v) = pr.get_all(Some(1), Some(params)).get(1) {
            let event_value = v.data.get("method").unwrap().as_str().unwrap();
            assert!(event_value.eq("SomeId.SomeEvent"));
        } else {
            panic!("Failure in get all with thunder register params")
        }
    }
}
