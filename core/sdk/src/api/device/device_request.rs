use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::extn::{
    extn_capability::{ExtnCapability, ExtnClass},
    extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
};

use super::device_info_request::DeviceInfoRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceRequest {
    DeviceInfo(DeviceInfoRequest),
    Extn(BaseDeviceRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseDeviceRequest {
    pub params: Option<Value>,
    pub method: String,
    pub module: String,
}

impl ExtnPayloadProvider for BaseDeviceRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Extn(self.clone())))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Request(request) => match request {
                ExtnRequest::Device(r) => match r {
                    DeviceRequest::Extn(d) => return Some(d),
                    _ => {}
                },
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn get_capability(&self) -> ExtnCapability {
        ExtnCapability::new_channel(ExtnClass::Device, self.method.clone())
    }

    fn cap() -> ExtnCapability {
        ExtnCapability::new_channel(ExtnClass::Device, "info".into())
    }
}
