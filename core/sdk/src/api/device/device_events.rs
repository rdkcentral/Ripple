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

use std::str::FromStr;

use crate::{
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};
use serde::{Deserialize, Serialize};

pub trait DeviceEventProvider {
    fn get_name() -> String;
}

pub const HDCP_CHANGED_EVENT: &str = "device.onHdcpChanged";
pub const HDR_CHANGED_EVENT: &str = "device.onHdrChanged";
pub const SCREEN_RESOLUTION_CHANGED_EVENT: &str = "device.onScreenResolutionChanged";
pub const VIDEO_RESOLUTION_CHANGED_EVENT: &str = "device.onVideoResolutionChanged";
pub const NETWORK_CHANGED_EVENT: &str = "device.onNetworkChanged";
pub const AUDIO_CHANGED_EVENT: &str = "device.onAudioChanged";
pub const VOICE_GUIDANCE_CHANGED: &str = "accessibility.onVoiceGuidanceSettingsChanged";
pub const POWER_STATE_CHANGED: &str = "device.onPowerStateChanged";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceEvent {
    InputChanged,
    HdrChanged,
    ScreenResolutionChanged,
    VideoResolutionChanged,
    VoiceGuidanceChanged,
    NetworkChanged,
    AudioChanged,
    SystemPowerStateChanged,
}

impl FromStr for DeviceEvent {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "device.onHdcpChanged" => Ok(Self::InputChanged),
            "device.onHdrChanged" => Ok(Self::HdrChanged),
            "device.onScreenResolutionChanged" => Ok(Self::ScreenResolutionChanged),
            "device.onVideoResolutionChanged" => Ok(Self::VideoResolutionChanged),
            "accessibility.onVoiceGuidanceSettingsChanged" => Ok(Self::VoiceGuidanceChanged),
            "device.onNetworkChanged" => Ok(Self::NetworkChanged),
            "device.onAudioChanged" => Ok(Self::AudioChanged),
            "device.onPowerStateChanged" => Ok(Self::SystemPowerStateChanged),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceEventCallback {
    FireboltAppEvent,
    ExtnEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceEventRequest {
    pub event: DeviceEvent,
    pub subscribe: bool,
    pub id: String,
    pub callback_type: DeviceEventCallback,
}

impl ExtnPayloadProvider for DeviceEventRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::DeviceEvent(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::DeviceEvent(d)) = payload {
            return Some(d);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::DeviceEvents
    }
}
