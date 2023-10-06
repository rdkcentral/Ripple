use std::{collections::HashMap, fmt::Display};

use ripple_sdk::{
    api::mock_server::{PayloadType, RequestPayload, ResponsePayload},
    log::error,
};
use serde_hashkey::{to_key, Key};
use serde_json::Value;

pub type MockData = HashMap<Key, (MockDataMessage, Vec<MockDataMessage>)>;

#[derive(Clone, Debug)]
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
    pub fn key(&self) -> Result<Key, MockDataError> {
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

pub fn json_key(value: &Value) -> Result<Key, MockDataError> {
    let key = to_key(value);
    if let Ok(key) = key {
        return Ok(key);
    }

    error!("Failed to create key from data {value:?}");
    Err(MockDataError::FailedToCreateKey(value.clone()))
}

pub fn jsonrpc_key(value: &Value) -> Result<Key, MockDataError> {
    let mut new_value = value.clone();
    new_value
        .as_object_mut()
        .and_then(|payload| payload.remove("id"));

    json_key(&new_value)
}
