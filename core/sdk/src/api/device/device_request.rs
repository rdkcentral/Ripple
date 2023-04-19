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

use crate::api::firebolt::fb_openrpc::FireboltSemanticVersion;

use super::{
    device_accessibility_data::StorageRequest, device_accessory::RemoteAccessoryRequest,
    device_browser::BrowserRequest, device_info_request::DeviceInfoRequest,
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
