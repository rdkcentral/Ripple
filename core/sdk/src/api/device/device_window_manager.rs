use serde::{Deserialize, Serialize};

use crate::{
    api::apps::Dimensions,
    extn::{
        extn_capability::{ExtnCapability, ExtnClass},
        extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    },
};

use super::device_request::DeviceRequest;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum WindowManagerRequest {
    Visibility(String, bool),
    MoveToFront(String),
    MoveToBack(String),
    Focus(String),
    Dimensions(String, Dimensions),
}

impl WindowManagerRequest {
    pub fn window_name(&self) -> String {
        match self {
            WindowManagerRequest::Visibility(wn, _) => wn,
            WindowManagerRequest::MoveToFront(wn) => wn,
            WindowManagerRequest::MoveToBack(wn) => wn,
            WindowManagerRequest::Focus(wn) => wn,
            WindowManagerRequest::Dimensions(wn, _) => wn,
        }
        .clone()
    }

    pub fn visible(&self) -> Option<bool> {
        if let WindowManagerRequest::Visibility(_, params) = self {
            return Some(if *params { true } else { false });
        }
        None
    }

    pub fn dimensions(&self) -> Option<Dimensions> {
        if let WindowManagerRequest::Dimensions(_, params) = self {
            return Some(params.clone());
        }
        None
    }
}

impl ExtnPayloadProvider for WindowManagerRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::WindowManager(
            self.clone(),
        )))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Request(request) => match request {
                ExtnRequest::Device(r) => match r {
                    DeviceRequest::WindowManager(d) => return Some(d),
                    _ => {}
                },
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn cap() -> ExtnCapability {
        ExtnCapability::new_channel(ExtnClass::Device, "window-manager".into())
    }
}
