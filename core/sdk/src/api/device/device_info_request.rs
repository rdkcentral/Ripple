// If not stated otherwise in this file or this component's license file the
// following copyright and licenses apply:
//
// Copyright 2023 RDK Management
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
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    api::firebolt::fb_openrpc::FireboltSemanticVersion,
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest, ExtnResponse},
    framework::ripple_contract::RippleContract,
};

use super::device_request::{
    AudioProfile, DeviceRequest, HDCPStatus, HdcpProfile, HdrProfile, OnInternetConnectedRequest,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceInfoRequest {
    MacAddress,
    Model,
    Make,
    Name,
    Version,
    HdcpSupport,
    HdcpStatus,
    Hdr,
    Audio,
    Sku,
    ScreenResolution,
    VideoResolution,
    AvailableMemory,
    Network,
    OnInternetConnected(OnInternetConnectedRequest),
    SetTimezone(String),
    GetTimezone,
    GetAvailableTimezones,
    VoiceGuidanceEnabled,
    SetVoiceGuidanceEnabled(bool),
    VoiceGuidanceSpeed,
    SetVoiceGuidanceSpeed(f32),
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

    fn contract() -> RippleContract {
        RippleContract::DeviceInfo
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceResponse {
    CustomError(String),
    AudioProfileResponse(HashMap<AudioProfile, bool>),
    HdcpSupportResponse(HashMap<HdcpProfile, bool>),
    HdcpStatusResponse(HDCPStatus),
    HdrResponse(HashMap<HdrProfile, bool>),
    FirmwareInfo(FireboltSemanticVersion),
    ScreenResolutionResponse(Vec<i32>),
    VideoResolutionResponse(Vec<i32>),
}

impl ExtnPayloadProvider for DeviceResponse {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::Value(
            serde_json::to_value(self.clone()).unwrap(),
        ))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Response(response) => match response {
                ExtnResponse::Value(value) => {
                    if let Ok(v) = serde_json::from_value(value) {
                        return Some(v);
                    }
                }
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn contract() -> RippleContract {
        RippleContract::DeviceInfo
    }
}
