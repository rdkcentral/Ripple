use serde::{Deserialize, Serialize};

use crate::extn::{
    extn_capability::{ExtnCapability, ExtnClass},
    extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
};

use super::device_request::DeviceRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceInfoRequest {
    MacAddress,
    Model,
    Make,
    Version,
    HdcpSupport,
    HdcpStatus,
    Hdr,
    Audio,
    ScreenResolution,
    VideoResolution,
    AvailableMemory,
}

impl ExtnPayloadProvider for DeviceInfoRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::DeviceInfo(self.clone())))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Request(request) => match request {
                ExtnRequest::Device(r) => match r {
                    DeviceRequest::DeviceInfo(d) => return Some(d),
                    _ => {}
                },
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn cap() -> ExtnCapability {
        ExtnCapability::new_channel(ExtnClass::Device, "info".into())
    }
}
