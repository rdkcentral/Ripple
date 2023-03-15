use std::{collections::HashMap, str::ParseBoolError};

use ripple_sdk::{
    api::{
        api::device::device_operator::DeviceCallRequest,
        firebolt::fb_hdcp::HdcpProfile,
    },
    export_thunder_extn_builder,
    extn::ffi::ffi_library::{CDeviceRequest, DeviceExtn, ThunderExtnBuilder},
    utils::{error::RippleError, logger::init_logger},
};
use serde_json::Value;

pub fn get_request(_: Value) -> CDeviceRequest {
    ThunderRequest::Call(DeviceCallRequest {
        method: "org.rdk.HdcpProfile.1.getSettopHDCPSupport".into(),
        params: None,
    })
    .into()
}

pub fn process(response: Value) -> Result<Value, RippleError> {
    let hdcp_version = response["supportedHDCPVersion"].to_string();
    let result: Result<bool, ParseBoolError> = response["isHDCPSupported"].to_string().parse();
    if let Ok(is_hdcp_supported) = result {
        let mut hdcp_response = HashMap::new();
        if hdcp_version.contains("1.4") {
            hdcp_response.insert(HdcpProfile::Hdcp1_4, is_hdcp_supported);
        }
        if hdcp_version.contains("2.2") {
            hdcp_response.insert(HdcpProfile::Hdcp2_2, is_hdcp_supported);
        }
        return Ok(serde_json::to_value(hdcp_response).unwrap());
    }

    Err(RippleError::ParseError)
}

pub fn get_thunder_extn(cap_str: &'static str) -> Option<DeviceExtn> {
    let _ = init_logger(format!("device_extn_{}", cap_str.clone()));
    Some(DeviceExtn {
        service: "hdcp-support".into(),
        get_request: get_request,
        process: process,
    })
}

fn init_thunder_builder() -> ThunderExtnBuilder {
    ThunderExtnBuilder {
        build: get_thunder_extn,
        caps: vec!["hdcp-support"],
    }
}

export_thunder_extn_builder!(ThunderExtnBuilder, init_thunder_builder);
