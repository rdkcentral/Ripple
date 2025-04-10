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

use std::collections::HashMap;

use ripple_sdk::api::{
    apps::{AppEvent, AppEventRequest},
    device::{
        device_accessibility_data::VoiceGuidanceSettings,
        device_events::VOICE_GUIDANCE_SETTINGS_CHANGED, device_request::VoiceGuidanceState,
    },
};
use ripple_sdk::serde_json;

use crate::{
    client::thunder_plugin::ThunderPlugin,
    events::thunder_event_processor::{
        ThunderEventHandler, ThunderEventHandlerProvider, ThunderEventMessage,
    },
    ripple_sdk::{
        api::device::{
            device_events::{DeviceEventCallback, HDCP_CHANGED_EVENT},
            device_request::{AudioProfile, HdcpProfile},
        },
        extn::extn_client_message::ExtnEvent,
        log::debug,
        tokio,
        utils::error::RippleError,
    },
    thunder_state::ThunderState,
};

use super::super::thunder_device_info::ThunderDeviceInfoRequestProcessor;

pub fn is_active_input(value: ThunderEventMessage) -> bool {
    if let ThunderEventMessage::ActiveInput(_) = value {
        return true;
    }
    false
}

pub fn is_resolution(value: ThunderEventMessage) -> bool {
    if let ThunderEventMessage::Resolution(_) = value {
        return true;
    }
    false
}

// -----------------------
// Active Input Changed
pub struct HDCPEventHandler;

impl HDCPEventHandler {
    pub fn handle(
        state: ThunderState,
        value: ThunderEventMessage,
        callback_type: DeviceEventCallback,
    ) {
        if let ThunderEventMessage::ActiveInput(input) = value {
            if input.active_input {
                debug!("activeInput changed");
            }
        }
        let state_c = state.clone();
        tokio::spawn(async move {
            let map = ThunderDeviceInfoRequestProcessor::get_hdcp_support(state).await;
            if let Ok(v) = Self::get_extn_event(map, callback_type) {
                ThunderEventHandler::callback_device_event(state_c, Self::get_mapped_event(), v)
            }
        });
    }
}

impl ThunderEventHandlerProvider for HDCPEventHandler {
    type EVENT = HashMap<HdcpProfile, bool>;
    fn provide(id: String, callback_type: DeviceEventCallback) -> ThunderEventHandler {
        ThunderEventHandler {
            request: Self::get_device_request(),
            handle: Self::handle,
            is_valid: is_active_input,
            listeners: vec![id],
            id: Self::get_mapped_event(),
            callback_type,
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
// VoiceGuidance Changed

pub struct VoiceGuidanceEnabledChangedEventHandler;

impl VoiceGuidanceEnabledChangedEventHandler {
    pub fn handle(
        state: ThunderState,
        value: ThunderEventMessage,
        callback_type: DeviceEventCallback,
    ) {
        if let ThunderEventMessage::VoiceGuidance(voice_guidance_state) = value {
            if let Ok(v) = Self::get_extn_event(voice_guidance_state.clone(), callback_type) {
                let thunder_state = state.clone();
                let enabled = voice_guidance_state.state;
                tokio::spawn(async move {
                    if let Ok(speed) = ThunderDeviceInfoRequestProcessor::get_voice_guidance_speed(
                        thunder_state.clone(),
                    )
                    .await
                    {
                        let settings = VoiceGuidanceSettings { enabled, speed };
                        let event = ExtnEvent::AppEvent(AppEventRequest::Emit(AppEvent {
                            event_name: VOICE_GUIDANCE_SETTINGS_CHANGED.to_string(),
                            result: serde_json::to_value(settings).unwrap(),
                            context: None,
                            app_id: None,
                        }));

                        ThunderEventHandler::callback_device_event(
                            thunder_state,
                            VOICE_GUIDANCE_SETTINGS_CHANGED.to_string(),
                            event,
                        );
                    }
                });
                ThunderEventHandler::callback_device_event(state, Self::get_mapped_event(), v)
            }
        }
    }

    pub fn is_valid(value: ThunderEventMessage) -> bool {
        if let ThunderEventMessage::VoiceGuidance(_) = value {
            return true;
        }
        false
    }
}

impl ThunderEventHandlerProvider for VoiceGuidanceEnabledChangedEventHandler {
    type EVENT = VoiceGuidanceState;
    fn provide(id: String, callback_type: DeviceEventCallback) -> ThunderEventHandler {
        ThunderEventHandler {
            request: Self::get_device_request(),
            handle: Self::handle,
            is_valid: Self::is_valid,
            listeners: vec![id],
            id: Self::get_mapped_event(),
            callback_type,
        }
    }

    fn event_name() -> String {
        "onVoiceGuidanceChanged".into()
    }

    fn get_mapped_event() -> String {
        "voiceguidance.onEnabledChanged".into()
    }

    fn module() -> String {
        ThunderPlugin::UserSettings.callsign_string()
    }

    fn get_extn_event(
        r: Self::EVENT,
        callback_type: DeviceEventCallback,
    ) -> Result<ExtnEvent, RippleError> {
        let result = serde_json::to_value(r).unwrap();
        match callback_type {
            DeviceEventCallback::FireboltAppEvent(_) => {
                Ok(ExtnEvent::AppEvent(AppEventRequest::Emit(AppEvent {
                    event_name: Self::get_mapped_event(),
                    context: None,
                    result,
                    app_id: None,
                })))
            }
            DeviceEventCallback::ExtnEvent => Ok(ExtnEvent::Value(result)),
        }
    }
}

pub struct AudioChangedEvent;

impl AudioChangedEvent {
    pub fn handle(
        state: ThunderState,
        value: ThunderEventMessage,
        callback_type: DeviceEventCallback,
    ) {
        if let ThunderEventMessage::Audio(v) = value {
            if let Ok(v) = Self::get_extn_event(v, callback_type) {
                ThunderEventHandler::callback_device_event(state, Self::get_mapped_event(), v)
            }
        }
    }

    pub fn is_valid(value: ThunderEventMessage) -> bool {
        if let ThunderEventMessage::Audio(_) = value {
            return true;
        }
        false
    }
}

impl ThunderEventHandlerProvider for AudioChangedEvent {
    type EVENT = HashMap<AudioProfile, bool>;
    fn provide(id: String, callback_type: DeviceEventCallback) -> ThunderEventHandler {
        ThunderEventHandler {
            request: Self::get_device_request(),
            handle: Self::handle,
            is_valid: Self::is_valid,
            listeners: vec![id],
            id: Self::get_mapped_event(),
            callback_type,
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
