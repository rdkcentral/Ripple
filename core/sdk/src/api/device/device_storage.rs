use crate::{
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};
use chrono::Utc;
use jsonrpsee_core::Serialize;
use serde::Deserialize;
use serde_json::Value;

use super::device_request::DeviceRequest;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StorageData {
    pub value: Value,
    pub update_time: String, // ISO 8601/RFC3339 format
}

impl StorageData {
    pub fn new(value: Value) -> StorageData {
        StorageData {
            value: value.clone(),
            update_time: Utc::now().to_rfc3339(),
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct SetBoolProperty {
    pub value: bool,
}

#[derive(Deserialize, Debug)]
pub struct SetStringProperty {
    pub value: String,
}

#[derive(Deserialize, Debug)]
pub struct SetU32Property {
    pub value: u32,
}

#[derive(Deserialize, Debug)]
pub struct SetF32Property {
    pub value: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GetStorageProperty {
    pub namespace: String,
    pub key: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SetStorageProperty {
    pub namespace: String,
    pub key: String,
    pub data: StorageData,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum StorageRequest {
    Get(GetStorageProperty),
    Set(SetStorageProperty),
}

impl ExtnPayloadProvider for StorageRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Storage(self.clone())))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Request(request) => match request {
                ExtnRequest::Device(r) => match r {
                    DeviceRequest::Storage(d) => return Some(d),
                    _ => {}
                },
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn contract() -> RippleContract {
        RippleContract::Storage
    }
}
