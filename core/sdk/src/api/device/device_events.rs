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

use crate::{
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};
use serde::{Deserialize, Serialize};

pub trait DeviceEventProvider {
    fn get_name() -> String;
}

pub const HDCP_CHANGED_EVENT: &'static str = "device.onHdcpChanged";
pub const HDR_CHANGED_EVENT: &'static str = "device.onHdrChanged";
pub const SCREEN_RESOLUTION_CHANGED_EVENT: &'static str = "device.onScreenResolutionChanged";
pub const VIDEO_RESOLUTION_CHANGED_EVENT: &'static str = "device.onVideoResolutionChanged";
pub const NETWORK_CHANGED_EVENT: &'static str = "device.onNetworkChanged";
pub const AUDIO_CHANGED_EVENT: &'static str = "device.onAudioChanged";
pub const VOICE_GUIDANCE_CHANGED: &'static str = "accessibility.onVoiceGuidanceSettingsChanged";

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceEventRequest {
    pub event: DeviceEvent,
    pub subscribe: bool,
    pub id: String,
}

impl ExtnPayloadProvider for DeviceEventRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::DeviceEvent(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Request(response) => match response {
                ExtnRequest::DeviceEvent(d) => return Some(d),
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn contract() -> RippleContract {
        RippleContract::DeviceEvents
    }
}
