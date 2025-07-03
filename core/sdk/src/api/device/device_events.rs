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
    api::session::EventAdjective,
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};
use serde::{Deserialize, Serialize};

pub trait DeviceEventProvider {
    fn get_name() -> String;
}

pub const HDCP_CHANGED_EVENT: &str = "device.onHdcpChanged";
pub const AUDIO_CHANGED_EVENT: &str = "device.onAudioChanged";
pub const VOICE_GUIDANCE_SETTINGS_CHANGED: &str = "accessibility.onVoiceGuidanceSettingsChanged";
pub const VOICE_GUIDANCE_ENABLED_CHANGED: &str = "voiceguidance.onEnabledChanged";
pub const VOICE_GUIDANCE_SPEED_CHANGED: &str = "voiceguidance.onSpeedChanged";

// Is this from the device to thunder event handler???
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum DeviceEvent {
    InputChanged,
    AudioChanged,
}

impl FromStr for DeviceEvent {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "device.onHdcpChanged" => Ok(Self::InputChanged),
            "device.onAudioChanged" => Ok(Self::AudioChanged),
            _ => Err(()),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum DeviceEventCallback {
    FireboltAppEvent(String),
    ExtnEvent,
}

impl DeviceEventCallback {
    pub fn get_id(&self) -> String {
        match self {
            DeviceEventCallback::FireboltAppEvent(id) => id.clone(),
            DeviceEventCallback::ExtnEvent => "internal".to_owned(),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct DeviceEventRequest {
    pub event: DeviceEvent,
    pub subscribe: bool,
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
    fn get_contract(&self) -> RippleContract {
        match self.event {
            DeviceEvent::InputChanged => RippleContract::DeviceEvents(EventAdjective::Input),
            DeviceEvent::AudioChanged => RippleContract::DeviceEvents(EventAdjective::Audio),
        }
    }

    fn contract() -> RippleContract {
        RippleContract::DeviceEvents(EventAdjective::Input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils::test_extn_payload_provider;
    use rstest::rstest;

    #[rstest(input, expected,
            case("device.onHdcpChanged", Ok(DeviceEvent::InputChanged)),
            case("invalid_event", Err(())),
        )]
    fn test_from_str(input: &str, expected: Result<DeviceEvent, ()>) {
        assert_eq!(DeviceEvent::from_str(input), expected);
    }

    #[rstest]
    #[case(
        DeviceEventCallback::FireboltAppEvent("app_event".to_string()),
        "app_event"
    )]
    #[case(DeviceEventCallback::ExtnEvent, "internal")]
    fn test_get_id(#[case] callback: DeviceEventCallback, #[case] expected_id: &str) {
        assert_eq!(callback.get_id(), expected_id);
    }

    #[test]
    fn test_extn_request_device_event() {
        let device_event_request = DeviceEventRequest {
            event: DeviceEvent::InputChanged,
            subscribe: true,
            callback_type: DeviceEventCallback::FireboltAppEvent("id".to_string()),
        };
        let contract_type: RippleContract = RippleContract::DeviceEvents(EventAdjective::Input);
        test_extn_payload_provider(device_event_request, contract_type);
    }
}
