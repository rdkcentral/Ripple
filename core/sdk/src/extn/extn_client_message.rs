use std::collections::HashMap;

use crossbeam::channel::Sender as CSender;
use log::error;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    api::{config::Config, gateway::rpc_gateway_api::RpcRequest},
    utils::error::RippleError,
};

use super::{extn_capability::ExtnCapability, ffi::ffi_message::CExtnMessage};

/// Default Message enum for the Communication Channel
/// Message would be either a request or response or event
///
/// Contains the Requester information
/// Unique UUID <optional for Events>
///
/// Here are some examples
/// GatewayMessage(ServiceStatusRequest) -> sends the status of the extension
/// GatewayMessage(RPCRequest) --> Sends a RPC request
/// DeviceMessage(LaunchAppRequest) --> Launches the app
/// RpcMessage(RPCExtnRequest) // contains Extn capability for checking

#[derive(Debug, Clone)]
pub struct ExtnMessage {
    pub id: String,
    pub requestor: ExtnCapability,
    pub target: ExtnCapability,
    pub payload: ExtnPayload,
    pub callback: Option<CSender<CExtnMessage>>,
}

impl ExtnMessage {
    pub fn is_request(&self) -> bool {
        match self.payload {
            ExtnPayload::Request(_) => true,
            _ => false,
        }
    }

    pub fn is_response(&self) -> bool {
        match self.payload {
            ExtnPayload::Response(_) => true,
            _ => false,
        }
    }

    pub fn is_event(&self) -> bool {
        match self.payload {
            ExtnPayload::Event(_) => true,
            _ => false,
        }
    }

    pub fn get_response(&self, response: ExtnResponse) -> Result<ExtnMessage, RippleError> {
        match self.clone().payload {
            ExtnPayload::Request(_) => Ok(ExtnMessage {
                callback: self.callback.clone(),
                id: self.id.clone(),
                payload: ExtnPayload::Response(response),
                requestor: self.target.clone(),
                target: self.requestor.clone(),
            }),
            _ => {
                error!("can only respond for a request message");
                Err(RippleError::InvalidInput)
            }
        }
    }
}

impl TryFrom<String> for ExtnPayload {
    type Error = RippleError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if let Ok(r) = serde_json::from_str(&value) {
            return Ok(r);
        }
        Err(RippleError::ParseError)
    }
}

impl Into<String> for ExtnPayload {
    fn into(self) -> String {
        serde_json::to_string(&self).unwrap()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExtnPayload {
    Request(ExtnRequest),
    Response(ExtnResponse),
    Event(ExtnEvent),
}

impl ExtnPayload {
    pub fn extract<T: ExtnPayloadProvider>(&self) -> Option<T> {
        T::get_from_payload(self.clone())
    }
}

pub trait ExtnPayloadProvider: Clone + Send + Sync
where
    Self: Sized,
{
    fn get_extn_payload(&self) -> ExtnPayload;
    fn get_from_payload(payload: ExtnPayload) -> Option<Self>;
    fn get_capability(&self) -> ExtnCapability {
        Self::cap()
    }
    fn cap() -> ExtnCapability;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExtnRequest {
    Config(Config),
    Rpc(RpcRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExtnResponse {
    String(String),
    Boolean(bool),
    Number(u32),
    Value(Value),
    StringMap(HashMap<String, String>),
    List(Vec<String>),
    Error(RippleError),
}

impl ExtnPayloadProvider for ExtnResponse {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(self.clone())
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Response(r) => return Some(r),
            _ => {}
        }
        None
    }

    fn cap() -> ExtnCapability {
        ExtnCapability::get_main_target("response".into())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExtnEvent {
    String(String),
}

impl ExtnPayloadProvider for ExtnEvent {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Event(self.clone())
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Event(r) => return Some(r),
            _ => {}
        }
        None
    }

    fn cap() -> ExtnCapability {
        ExtnCapability::get_main_target("event".into())
    }
}
