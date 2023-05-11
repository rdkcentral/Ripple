use std::str::FromStr;

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

use crate::{
    api::firebolt::fb_openrpc::FireboltSemanticVersion,
    extn::extn_client_message::{ExtnEvent, ExtnPayload, ExtnPayloadProvider},
    framework::ripple_contract::RippleContract,
};

use super::{
    device_accessory::RemoteAccessoryRequest, device_browser::BrowserRequest,
    device_info_request::DeviceInfoRequest, device_storage::StorageRequest,
    device_wifi::WifiRequest, device_window_manager::WindowManagerRequest,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceRequest {
    DeviceInfo(DeviceInfoRequest),
    Browser(BrowserRequest),
    WindowManager(WindowManagerRequest),
    Storage(StorageRequest),
    Wifi(WifiRequest),
    Accessory(RemoteAccessoryRequest),
}

#[derive(Hash, Eq, PartialEq, Debug, Serialize, Deserialize, Clone)]
pub enum HdcpProfile {
    #[serde(rename = "hdcp1.4")]
    Hdcp1_4,
    #[serde(rename = "hdcp2.2")]
    Hdcp2_2,
}

#[derive(Hash, Eq, PartialEq, Debug, Serialize, Deserialize, Clone)]
pub enum HdrProfile {
    #[serde(rename = "HDR10")]
    Hdr10,
    #[serde(rename = "HDR10+")]
    Hdr10plus,
    #[serde(rename = "HLG")]
    Hlg,
    #[serde(rename = "Dolby Vision")]
    DolbyVision,
    #[serde(rename = "Technicolor")]
    Technicolor,
}

#[derive(Hash, Eq, PartialEq, Debug, Serialize, Deserialize, Clone)]
pub enum AudioProfile {
    #[serde(rename = "stereo")]
    Stereo,
    #[serde(rename = "dolbyDigital5.1")]
    DolbyDigital5_1,
    #[serde(rename = "dolbyDigital7.1")]
    DolbyDigital7_1,
    #[serde(rename = "dolbyDigital5.1+")]
    DolbyDigital5_1Plus,
    #[serde(rename = "dolbyDigital7.1+")]
    DolbyDigital7_1Plus,
    #[serde(rename = "dolbyAtmos")]
    DolbyAtmos,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceVersionResponse {
    pub api: FireboltSemanticVersion,
    pub firmware: FireboltSemanticVersion,
    pub os: FireboltSemanticVersion,
    pub debug: String,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkResponse {
    pub state: NetworkState,
    #[serde(rename = "type")]
    pub _type: NetworkType,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NetworkState {
    Connected,
    Disconnected,
}

impl FromStr for NetworkState {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "CONNECTED" => Ok(NetworkState::Connected),
            "DISCONNECTED" => Ok(NetworkState::Disconnected),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Hash, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NetworkType {
    Wifi,
    Ethernet,
    Hybrid,
}

impl FromStr for NetworkType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "WIFI" => Ok(NetworkType::Wifi),
            "ETHERNET" => Ok(NetworkType::Ethernet),
            "HYBRID" => Ok(NetworkType::Hybrid),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HDCPStatus {
    pub is_connected: bool,
    #[serde(rename = "isHDCPCompliant")]
    pub is_hdcp_compliant: bool,
    #[serde(rename = "isHDCPEnabled")]
    pub is_hdcp_enabled: bool,
    pub hdcp_reason: u32,
    #[serde(rename = "supportedHDCPVersion")]
    pub supported_hdcp_version: String,
    #[serde(rename = "receiverHDCPVersion")]
    pub receiver_hdcp_version: String,
    #[serde(rename = "currentHDCPVersion")]
    pub current_hdcp_version: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Hash, Eq)]
#[serde(rename_all = "lowercase")]

pub enum Resolution {
    Resolution480,
    Resolution576,
    Resolution540,
    Resolution720,
    Resolution1080,
    Resolution2160,
    Resolution4k,
    ResolutionDefault,
}

impl Resolution {
    pub fn dimension(&self) -> Vec<i32> {
        match self {
            Resolution::Resolution480 => vec![720, 480],
            Resolution::Resolution576 => vec![720, 576],
            Resolution::Resolution540 => vec![960, 540],
            Resolution::Resolution720 => vec![1280, 720],
            Resolution::Resolution1080 => vec![1920, 1080],
            Resolution::Resolution2160 => vec![3840, 2160],
            Resolution::Resolution4k => vec![3840, 2160],
            Resolution::ResolutionDefault => vec![1920, 1080],
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OnInternetConnectedRequest {
    pub timeout: u64,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct LanguageProperty {
    //#[serde(with = "language_code_serde")]
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct TimezoneProperty {
    //#[serde(with = "timezone_serde")]
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum PowerState {
    Standby,
    DeepSleep,
    LightSleep,
    On,
}
impl FromStr for PowerState {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "STANDBY" => Ok(PowerState::Standby),
            "DEEP_SLEEP" => Ok(PowerState::DeepSleep),
            "LIGHT_SLEEP" => Ok(PowerState::LightSleep),
            "ON" => Ok(PowerState::On),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemPowerState {
    pub power_state: PowerState,
    pub current_power_state: PowerState,
}

impl ExtnPayloadProvider for SystemPowerState {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Event(ExtnEvent::PowerState(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<SystemPowerState> {
        match payload {
            ExtnPayload::Event(request) => match request {
                ExtnEvent::PowerState(r) => return Some(r),
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn contract() -> RippleContract {
        RippleContract::PowerStateEvent
    }
}
