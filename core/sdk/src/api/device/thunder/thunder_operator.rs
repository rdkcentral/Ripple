use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;

#[async_trait]
pub trait ThunderOperator: Clone {
    async fn call_thunder(&self, request: ThunderCallRequest) -> ThunderResponseMessage;

    async fn subscribe(
        &self,
        request: ThunderSubsribeRequest,
        handler: mpsc::Sender<ThunderResponseMessage>,
    ) -> ThunderResponseMessage;

    async fn unsubscribe(&self, request: ThunderUnsubsribeRequest);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ThunderRequest {
    Call(ThunderCallRequest),
    Subscribe(ThunderSubsribeRequest),
    Unsubscribe(ThunderUnsubsribeRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThunderCallRequest {
    pub method: String,
    pub params: Option<ThunderParams>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThunderSubsribeRequest {
    pub module: String,
    pub event_name: String,
    pub params: Option<String>,
    pub sub_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThunderUnsubsribeRequest {
    pub module: String,
    pub event_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ThunderParams {
    Json(String),
    Literal(String),
    Bool(bool),
}

impl ThunderParams {
    pub fn as_params(&self) -> String {
        match self {
            ThunderParams::Json(json) => json.clone(),
            ThunderParams::Literal(lit) => lit.clone(),
            ThunderParams::Bool(_) => String::from(""),
        }
    }

    pub fn as_value(&self) -> Option<Value> {
        match self {
            ThunderParams::Bool(b) => Some(Value::Bool(*b)),
            _ => None,
        }
    }

    pub fn is_json(&self) -> bool {
        match self {
            ThunderParams::Json(_) => true,
            ThunderParams::Literal(_) => false,
            ThunderParams::Bool(_) => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThunderResponseMessage {
    pub message: Value,
    pub sub_id: Option<String>,
}

impl ThunderResponseMessage {
    pub fn call(message: Value) -> ThunderResponseMessage {
        ThunderResponseMessage {
            message,
            sub_id: None,
        }
    }

    pub fn sub(message: Value, sub_id: String) -> ThunderResponseMessage {
        ThunderResponseMessage {
            message,
            sub_id: Some(sub_id),
        }
    }
}
