// Copyright 2023 Comcast Cable Communications Management, LLC
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
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::{
    api::firebolt::fb_openrpc::FireboltSemanticVersion,
    extn::{
        client::extn_client::ExtnClient,
        extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest, ExtnResponse},
    },
    framework::ripple_contract::RippleContract,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::device_request::{
    AudioProfile, DeviceRequest, HDCPStatus, HdcpProfile, HdrProfile, InternetConnectionStatus,
    PowerState, TimeZone,
};

pub const DEVICE_INFO_AUTHORIZED: &str = "device_info_authorized";
pub const DEVICE_SKU_AUTHORIZED: &str = "device_sku_authorized";
pub const DEVICE_MAKE_MODEL_AUTHORIZED: &str = "device_make_model_authorized";
pub const DEVICE_NETWORK_STATUS_AUTHORIZED: &str = "network_status_authorized";

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum DeviceInfoRequest {
    Model,
    FirmwareInfo,
    HdcpSupport,
    HdcpStatus,
    Audio,
    AvailableMemory,
    InternetConnectionStatus,
    VoiceGuidanceEnabled,
    SetVoiceGuidanceEnabled(bool),
    VoiceGuidanceSpeed,
    SetVoiceGuidanceSpeed(f32),
    PowerState,
    PlatformBuildInfo,
}

impl ExtnPayloadProvider for DeviceInfoRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::DeviceInfo(self.clone())))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::DeviceInfo(d))) = payload {
            return Some(d);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::DeviceInfo
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCapabilities {
    pub video_resolution: Option<Vec<i32>>,
    pub screen_resolution: Option<Vec<i32>>,
    pub firmware_info: Option<FireboltSemanticVersion>,
    pub hdr: Option<HashMap<HdrProfile, bool>>,
    pub hdcp: Option<HDCPStatus>,
    pub model: Option<String>,
    pub make: Option<String>,
    pub audio: Option<HashMap<AudioProfile, bool>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PlatformBuildInfo {
    pub name: String,
    pub device_model: String,
    pub branch: Option<String>,
    pub release_version: Option<String>,
    pub debug: bool,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FirmwareInfo {
    pub name: String,
    pub version: FireboltSemanticVersion,
}

impl From<FireboltSemanticVersion> for FirmwareInfo {
    fn from(version: FireboltSemanticVersion) -> Self {
        FirmwareInfo {
            name: "rdk".into(),
            version,
        }
    }
}
impl Default for FirmwareInfo {
    fn default() -> Self {
        FirmwareInfo {
            name: "rdk".into(),
            version: FireboltSemanticVersion::default(),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum DeviceResponse {
    CustomError(String),
    AudioProfileResponse(HashMap<AudioProfile, bool>),
    HdcpSupportResponse(HashMap<HdcpProfile, bool>),
    HdcpStatusResponse(HDCPStatus),
    FirmwareInfo(FirmwareInfo),
    InternetConnectionStatus(InternetConnectionStatus),
    PowerState(PowerState),
    TimeZone(TimeZone),
    PlatformBuildInfo(PlatformBuildInfo),
}

impl ExtnPayloadProvider for DeviceResponse {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::Value(
            serde_json::to_value(self.clone()).unwrap(),
        ))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Response(ExtnResponse::Value(value)) = payload {
            if let Ok(v) = serde_json::from_value(value) {
                return Some(v);
            }
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::DeviceInfo
    }
}

pub struct DeviceInfo {}

impl DeviceInfo {
    ///
    /// Checks if the device is a debug device
    /// If any error happens then assume it is NOT a debug device
    pub async fn is_debug(extn_client: &mut ExtnClient) -> bool {
        let resp_res = extn_client
            .request(DeviceInfoRequest::PlatformBuildInfo)
            .await;
        if resp_res.is_err() {
            return false;
        }
        let device_resp = resp_res.unwrap().payload.extract::<DeviceResponse>();
        if device_resp.is_none() {
            return false;
        }
        match device_resp.unwrap() {
            DeviceResponse::PlatformBuildInfo(pbi) => pbi.debug,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils::test_extn_payload_provider;

    #[test]
    fn test_extn_request_device_info_request() {
        let contract_type: RippleContract = RippleContract::DeviceInfo;
        test_extn_payload_provider(DeviceInfoRequest::Model, contract_type);
    }

    #[test]
    fn test_extn_payload_provider_for_device_response() {
        let device_response = DeviceResponse::PowerState(PowerState::Standby);

        let contract_type: RippleContract = RippleContract::DeviceInfo;
        test_extn_payload_provider(device_response, contract_type);
    }
}
