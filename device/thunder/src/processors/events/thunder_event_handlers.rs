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

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thunder_ripple_sdk::{
    client::thunder_plugin::ThunderPlugin,
    events::thunder_event_processor::{ThunderEventHandler, ThunderEventHandlerProvider},
    ripple_sdk::{
        api::device::{
            device_events::{
                HDCP_CHANGED_EVENT, HDR_CHANGED_EVENT, NETWORK_CHANGED_EVENT,
                SCREEN_RESOLUTION_CHANGED_EVENT, VIDEO_RESOLUTION_CHANGED_EVENT,
            },
            device_request::{
                NetworkResponse, NetworkState, NetworkType, PowerState, SystemPowerState,
            },
        },
        log::{debug, error},
        serde_json::{self, Value},
        tokio,
    },
    thunder_state::ThunderState,
};

use super::super::thunder_device_info::{
    get_audio, get_dimension_from_resolution, ThunderDeviceInfoRequestProcessor,
};

// -----------------------
// Active Input Changed
pub struct HDCPEventHandler;

impl HDCPEventHandler {
    pub fn handle(state: ThunderState, value: Value) {
        if let Some(v) = value["activeInput"].as_bool() {
            debug!("activeInput {}", v);
        }
        let state_c = state.clone();
        tokio::spawn(async move {
            let map = ThunderDeviceInfoRequestProcessor::get_hdcp_support(state).await;
            ThunderEventHandler::send_app_event(
                state_c,
                Self::get_mapped_event(),
                serde_json::to_value(map).unwrap(),
            )
        });
    }
}

impl ThunderEventHandlerProvider for HDCPEventHandler {
    fn provide(id: String) -> ThunderEventHandler {
        ThunderEventHandler {
            request: Self::get_device_request(),
            handle: Self::handle,
            listeners: vec![id],
            id: Self::get_mapped_event(),
        }
    }

    fn get_mapped_event() -> String {
        HDCP_CHANGED_EVENT.into()
    }

    fn event_name() -> String {
        "activeInputChanged".into()
    }

    fn module() -> String {
        ThunderPlugin::DisplaySettings.callsign_string()
    }
}

// -----------------------
// HDR Changed

pub struct HDREventHandler;

impl HDREventHandler {
    pub fn handle(state: ThunderState, value: Value) {
        if let Some(v) = value["activeInput"].as_bool() {
            debug!("activeInput {}", v);
        }
        let state_c = state.clone();
        tokio::spawn(async move {
            let map = ThunderDeviceInfoRequestProcessor::get_hdr(state).await;
            ThunderEventHandler::send_app_event(
                state_c,
                Self::get_mapped_event(),
                serde_json::to_value(map).unwrap(),
            )
        });
    }
}

impl ThunderEventHandlerProvider for HDREventHandler {
    fn provide(id: String) -> ThunderEventHandler {
        ThunderEventHandler {
            request: Self::get_device_request(),
            handle: Self::handle,
            listeners: vec![id],
            id: Self::get_mapped_event(),
        }
    }

    fn get_mapped_event() -> String {
        HDR_CHANGED_EVENT.into()
    }

    fn event_name() -> String {
        "activeInputChanged".into()
    }

    fn module() -> String {
        ThunderPlugin::DisplaySettings.callsign_string()
    }
}

// -----------------------
// ScreenResolution Changed

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionChangedEvent {
    pub width: i32,
    pub height: i32,
    pub video_display_type: String,
    pub resolution: String,
}

pub struct ScreenResolutionEventHandler;

impl ScreenResolutionEventHandler {
    pub fn handle(state: ThunderState, value: Value) {
        if let Ok(v) = serde_json::from_value::<ResolutionChangedEvent>(value) {
            let value: Vec<i32> = vec![v.width, v.height];
            ThunderEventHandler::send_app_event(
                state,
                Self::get_mapped_event(),
                serde_json::to_value(value).unwrap(),
            )
        }
    }
}

impl ThunderEventHandlerProvider for ScreenResolutionEventHandler {
    fn provide(id: String) -> ThunderEventHandler {
        ThunderEventHandler {
            request: Self::get_device_request(),
            handle: Self::handle,
            listeners: vec![id],
            id: Self::get_mapped_event(),
        }
    }

    fn event_name() -> String {
        "resolutionChanged".into()
    }

    fn get_mapped_event() -> String {
        SCREEN_RESOLUTION_CHANGED_EVENT.into()
    }

    fn module() -> String {
        ThunderPlugin::DisplaySettings.callsign_string()
    }
}

// -----------------------
// VideoResolution Changed
pub struct VideoResolutionEventHandler;

impl VideoResolutionEventHandler {
    pub fn handle(state: ThunderState, value: Value) {
        if let Ok(v) = serde_json::from_value::<ResolutionChangedEvent>(value) {
            let value = get_dimension_from_resolution(&v.resolution);
            ThunderEventHandler::send_app_event(
                state,
                Self::get_mapped_event(),
                serde_json::to_value(value).unwrap(),
            )
        }
    }
}

impl ThunderEventHandlerProvider for VideoResolutionEventHandler {
    fn provide(id: String) -> ThunderEventHandler {
        ThunderEventHandler {
            request: Self::get_device_request(),
            handle: Self::handle,
            listeners: vec![id],
            id: Self::get_mapped_event(),
        }
    }

    fn event_name() -> String {
        "resolutionChanged".into()
    }

    fn get_mapped_event() -> String {
        VIDEO_RESOLUTION_CHANGED_EVENT.into()
    }

    fn module() -> String {
        ThunderPlugin::DisplaySettings.callsign_string()
    }
}

// -----------------------
// Network Changed

pub struct NetworkEventHandler;

impl NetworkEventHandler {
    pub fn handle(state: ThunderState, value: Value) {
        if let Some(v) = value["interface"].as_str() {
            if let Ok(network_type) = NetworkType::from_str(v) {
                if let Some(v) = value["status"].as_str() {
                    if let Ok(network_status) = NetworkState::from_str(v) {
                        let response = NetworkResponse {
                            _type: network_type,
                            state: network_status,
                        };
                        ThunderEventHandler::send_app_event(
                            state,
                            Self::get_mapped_event(),
                            serde_json::to_value(response).unwrap(),
                        )
                    }
                }
            }
        }
    }
}

impl ThunderEventHandlerProvider for NetworkEventHandler {
    fn provide(id: String) -> ThunderEventHandler {
        ThunderEventHandler {
            request: Self::get_device_request(),
            handle: Self::handle,
            listeners: vec![id],
            id: Self::get_mapped_event(),
        }
    }

    fn event_name() -> String {
        "onConnectionStatusChanged".into()
    }

    fn get_mapped_event() -> String {
        NETWORK_CHANGED_EVENT.into()
    }

    fn module() -> String {
        ThunderPlugin::Network.callsign_string()
    }
}

// -----------------------
// SystemPower Changed

pub struct SystemPowerStateChangeEventHandler;

impl SystemPowerStateChangeEventHandler {
    pub fn handle(state: ThunderState, value: Value) {
        if let Some(v) = value["powerState"].as_str() {
            if let Ok(power_state) = PowerState::from_str(v) {
                if let Some(v) = value["currentPowerState"].as_str() {
                    if let Ok(current_power_state) = PowerState::from_str(v) {
                        let response = SystemPowerState {
                            power_state,
                            current_power_state,
                        };
                        if let Err(_) = state.get_client().request_transient(response) {
                            error!("Error sending system power state");
                        }
                    }
                }
            }
        }
    }
}

impl ThunderEventHandlerProvider for SystemPowerStateChangeEventHandler {
    fn provide(id: String) -> ThunderEventHandler {
        ThunderEventHandler {
            request: Self::get_device_request(),
            handle: Self::handle,
            listeners: vec![id],
            id: Self::get_mapped_event(),
        }
    }

    fn event_name() -> String {
        "onSystemPowerStateChanged".into()
    }

    fn get_mapped_event() -> String {
        "device.onPowerStateChanged".into()
    }

    fn module() -> String {
        ThunderPlugin::System.callsign_string()
    }
}

// -----------------------
// VoiceGuidance Changed

pub struct VoiceGuidanceEnabledChangedEventHandler;

impl VoiceGuidanceEnabledChangedEventHandler {
    pub fn handle(state: ThunderState, value: Value) {
        let response = Value::Bool(value["state"].as_bool().unwrap_or(false));
        ThunderEventHandler::send_app_event(
            state,
            Self::get_mapped_event(),
            serde_json::to_value(response).unwrap(),
        )
    }
}

impl ThunderEventHandlerProvider for VoiceGuidanceEnabledChangedEventHandler {
    fn provide(id: String) -> ThunderEventHandler {
        ThunderEventHandler {
            request: Self::get_device_request(),
            handle: Self::handle,
            listeners: vec![id],
            id: Self::get_mapped_event(),
        }
    }

    fn event_name() -> String {
        "onttsstatechanged".into()
    }

    fn get_mapped_event() -> String {
        "device.onVoiceGuidanceSettingsChanged".into()
    }

    fn module() -> String {
        ThunderPlugin::TextToSpeech.callsign_string()
    }
}

pub struct AudioChangedEvent;

impl AudioChangedEvent {
    pub fn handle(state: ThunderState, value: Value) {
        let value = get_audio(value);
        ThunderEventHandler::send_app_event(
            state,
            Self::get_mapped_event(),
            serde_json::to_value(value).unwrap(),
        )
    }
}

impl ThunderEventHandlerProvider for AudioChangedEvent {
    fn provide(id: String) -> ThunderEventHandler {
        ThunderEventHandler {
            request: Self::get_device_request(),
            handle: Self::handle,
            listeners: vec![id],
            id: Self::get_mapped_event(),
        }
    }

    fn event_name() -> String {
        "audioFormatChanged".into()
    }

    fn get_mapped_event() -> String {
        "device.onAudioChanged".into()
    }

    fn module() -> String {
        ThunderPlugin::DisplaySettings.callsign_string()
    }
}
