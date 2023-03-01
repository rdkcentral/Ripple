use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;

/// Generic Operator trait used for Device Communications
/// Each Device platform should implement this trait based on the underlying service
#[async_trait]
pub trait DeviceOperator: Clone {
    async fn call(&self, request: DeviceCallRequest) -> DeviceResponseMessage;

    async fn subscribe(
        &self,
        request: DeviceSubsribeRequest,
        handler: mpsc::Sender<DeviceResponseMessage>,
    ) -> DeviceResponseMessage;

    async fn unsubscribe(&self, request: DeviceUnsubscribeRequest);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceChannelRequest {
    Call(DeviceCallRequest),
    Subscribe(DeviceSubsribeRequest),
    Unsubscribe(DeviceUnsubscribeRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCallRequest {
    pub method: String,
    pub params: Option<DeviceChannelParams>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceSubsribeRequest {
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
