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
    context::RippleContextUpdateRequest,
    device::{
        device_accessibility_data::VoiceGuidanceSettings,
        device_events::{
            INTERNET_CHANGED_EVENT, TIME_ZONE_CHANGED, VOICE_GUIDANCE_SETTINGS_CHANGED,
        },
        device_request::{InternetConnectionStatus, TimeZone, VoiceGuidanceState},
    },
};
use ripple_sdk::serde_json;

use crate::{
    client::thunder_plugin::ThunderPlugin,
    events::thunder_event_processor::{
        ThunderEventHandler, ThunderEventHandlerProvider, ThunderEventMessage,
    },
    processors::thunder_device_info::CachedState,
    ripple_sdk::{
        api::device::{
            device_events::{
                DeviceEventCallback, HDCP_CHANGED_EVENT, HDR_CHANGED_EVENT, NETWORK_CHANGED_EVENT,
                POWER_STATE_CHANGED, SCREEN_RESOLUTION_CHANGED_EVENT,
                VIDEO_RESOLUTION_CHANGED_EVENT,
            },
            device_request::{
                AudioProfile, HdcpProfile, HdrProfile, NetworkResponse, SystemPowerState,
            },
        },
        extn::extn_client_message::ExtnEvent,
        log::debug,
        tokio,
        utils::error::RippleError,
    },
    thunder_state::ThunderState,
};

use super::super::thunder_device_info::{
    get_dimension_from_resolution, ThunderDeviceInfoRequestProcessor,
};

pub fn is_display_connection_changed(value: ThunderEventMessage) -> bool {
    if let ThunderEventMessage::DisplayConnection(_) = value {
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
// Display Connection Changed
pub struct HDCPEventHandler;

impl HDCPEventHandler {
    pub fn handle(
        state: ThunderState,
        value: ThunderEventMessage,
        callback_type: DeviceEventCallback,
    ) {
        if let ThunderEventMessage::DisplayConnection(_connection) = value {
            debug!("HDCPEventHandler: display connection changed");
        }

        let state_c = state.clone();
        let cached_state = CachedState::new(state.clone());

        tokio::spawn(async move {
            let map = ThunderDeviceInfoRequestProcessor::update_hdcp_cache(cached_state).await;
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
            is_valid: is_display_connection_changed,
            listeners: vec![id],
            id: Self::get_mapped_event(),
            callback_type,
        }
    }

    fn get_mapped_event() -> String {
        HDCP_CHANGED_EVENT.into()
    }

    fn event_name() -> String {
        "onDisplayConnectionChanged".into()
    }

    fn module() -> String {
        ThunderPlugin::DisplaySettings.callsign_string()
    }
}

// -----------------------
// HDR Changed

pub struct HDREventHandler;

impl HDREventHandler {
    pub fn handle(
        state: ThunderState,
        value: ThunderEventMessage,
        callback_type: DeviceEventCallback,
    ) {
        if let ThunderEventMessage::DisplayConnection(_connection) = value {
            debug!("HDREventHandler: display connection changed");
        }

        let state_c = state.clone();
        let cached_state = CachedState::new(state.clone());

        tokio::spawn(async move {
            let map = ThunderDeviceInfoRequestProcessor::get_hdr(cached_state).await;
            if let Ok(v) = Self::get_extn_event(map, callback_type) {
                ThunderEventHandler::callback_device_event(state_c, Self::get_mapped_event(), v)
            }
        });
    }
}

impl ThunderEventHandlerProvider for HDREventHandler {
    type EVENT = HashMap<HdrProfile, bool>;

    fn provide(id: String, callback_type: DeviceEventCallback) -> ThunderEventHandler {
        ThunderEventHandler {
            request: Self::get_device_request(),
            handle: Self::handle,
            is_valid: is_display_connection_changed,
            listeners: vec![id],
            id: Self::get_mapped_event(),
            callback_type,
        }
    }

    fn get_mapped_event() -> String {
        HDR_CHANGED_EVENT.into()
    }

    fn event_name() -> String {
        "onDisplayConnectionChanged".into()
    }

    fn module() -> String {
        ThunderPlugin::DisplaySettings.callsign_string()
    }
}

// -----------------------
// ScreenResolution Changed

pub struct ScreenResolutionEventHandler;

impl ScreenResolutionEventHandler {
    pub fn handle(
        state: ThunderState,
        value: ThunderEventMessage,
        callback_type: DeviceEventCallback,
    ) {
        if let ThunderEventMessage::Resolution(v) = value {
            let value: Vec<i32> = vec![v.width, v.height];
            if let Ok(v) = Self::get_extn_event(value, callback_type) {
                ThunderEventHandler::callback_device_event(state, Self::get_mapped_event(), v)
            }
        }
    }
}

impl ThunderEventHandlerProvider for ScreenResolutionEventHandler {
    type EVENT = Vec<i32>;
    fn provide(id: String, callback_type: DeviceEventCallback) -> ThunderEventHandler {
        ThunderEventHandler {
            request: Self::get_device_request(),
            handle: Self::handle,
            is_valid: is_resolution,
            listeners: vec![id],
            id: Self::get_mapped_event(),
            callback_type,
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
    pub fn handle(
        state: ThunderState,
        value: ThunderEventMessage,
        callback_type: DeviceEventCallback,
    ) {
        if let ThunderEventMessage::Resolution(v) = value {
            let value = get_dimension_from_resolution(&v.resolution);
            if let Ok(v) = Self::get_extn_event(value, callback_type) {
                ThunderEventHandler::callback_device_event(state, Self::get_mapped_event(), v)
            }
        }
    }
}

impl ThunderEventHandlerProvider for VideoResolutionEventHandler {
    type EVENT = Vec<i32>;
    fn provide(id: String, callback_type: DeviceEventCallback) -> ThunderEventHandler {
        ThunderEventHandler {
            request: Self::get_device_request(),
            handle: Self::handle,
            is_valid: is_resolution,
            listeners: vec![id],
            id: Self::get_mapped_event(),
            callback_type,
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
// Internet Changed

pub struct InternetEventHandler;

impl InternetEventHandler {
    pub fn handle(
        state: ThunderState,
        value: ThunderEventMessage,
        _callback_type: DeviceEventCallback,
    ) {
        if let ThunderEventMessage::Internet(v) = value {
            ThunderEventHandler::callback_context_update(
                state,
                RippleContextUpdateRequest::InternetStatus(v),
            )
        }
    }

    pub fn is_valid(message: ThunderEventMessage) -> bool {
        if let ThunderEventMessage::Internet(_) = message {
            return true;
        }
        false
    }
}

impl ThunderEventHandlerProvider for InternetEventHandler {
    type EVENT = InternetConnectionStatus;
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

    // This is the thunder event name
    fn event_name() -> String {
        "onInternetStatusChange".into()
    }

    // This is the event at the application level
    fn get_mapped_event() -> String {
        INTERNET_CHANGED_EVENT.into()
    }

    fn module() -> String {
        ThunderPlugin::Network.callsign_string()
    }
    fn get_extn_event(
        _r: Self::EVENT,
        _callback_type: DeviceEventCallback,
    ) -> Result<ExtnEvent, RippleError> {
        Err(RippleError::InvalidOutput)
    }
}

// -----------------------
// Network Changed

pub struct NetworkEventHandler;

impl NetworkEventHandler {
    pub fn handle(
        state: ThunderState,
        value: ThunderEventMessage,
        callback_type: DeviceEventCallback,
    ) {
        if let ThunderEventMessage::Network(v) = value {
            if let Ok(v) = Self::get_extn_event(v, callback_type) {
                ThunderEventHandler::callback_device_event(state, Self::get_mapped_event(), v)
            }
        }
    }

    pub fn is_valid(message: ThunderEventMessage) -> bool {
        if let ThunderEventMessage::Network(_) = message {
            return true;
        }
        false
    }
}

impl ThunderEventHandlerProvider for NetworkEventHandler {
    type EVENT = NetworkResponse;
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
    pub fn handle(
        state: ThunderState,
        value: ThunderEventMessage,
        _callback_type: DeviceEventCallback,
    ) {
        if let ThunderEventMessage::PowerState(p) = value {
            ThunderEventHandler::callback_context_update(
                state,
                RippleContextUpdateRequest::PowerState(p),
            )
        }
    }

    pub fn is_valid(value: ThunderEventMessage) -> bool {
        if let ThunderEventMessage::PowerState(_) = value {
            return true;
        }
        false
    }
}

impl ThunderEventHandlerProvider for SystemPowerStateChangeEventHandler {
    type EVENT = SystemPowerState;
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
        "onSystemPowerStateChanged".into()
    }

    fn get_mapped_event() -> String {
        POWER_STATE_CHANGED.into()
    }

    fn module() -> String {
        ThunderPlugin::System.callsign_string()
    }

    fn get_extn_event(
        _r: Self::EVENT,
        _callback_type: DeviceEventCallback,
    ) -> Result<ExtnEvent, RippleError> {
        Err(RippleError::InvalidOutput)
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
        "onttsstatechanged".into()
    }

    fn get_mapped_event() -> String {
        "voiceguidance.onEnabledChanged".into()
    }

    fn module() -> String {
        ThunderPlugin::TextToSpeech.callsign_string()
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

pub struct TimezoneChangedEventHandler;

impl TimezoneChangedEventHandler {
    pub fn handle(
        state: ThunderState,
        value: ThunderEventMessage,
        _callback_type: DeviceEventCallback,
    ) {
        if let ThunderEventMessage::TimeZone(_v) = value {
            let cached_state = CachedState::new(state.clone());
            tokio::spawn(async move {
                if let Some(tz) =
                    ThunderDeviceInfoRequestProcessor::get_timezone_and_offset(&cached_state).await
                {
                    let event = ExtnEvent::AppEvent(AppEventRequest::Emit(AppEvent {
                        event_name: TIME_ZONE_CHANGED.to_string(),
                        result: serde_json::to_value(tz.clone().time_zone).unwrap(),
                        context: None,
                        app_id: None,
                    }));

                    ThunderEventHandler::callback_device_event(
                        state.clone(),
                        TIME_ZONE_CHANGED.to_string(),
                        event,
                    );

                    ThunderEventHandler::callback_context_update(
                        state,
                        RippleContextUpdateRequest::TimeZone(tz),
                    );
                }
            });
        }
    }

    pub fn is_valid(value: ThunderEventMessage) -> bool {
        if let ThunderEventMessage::TimeZone(_) = value {
            return true;
        }
        false
    }
}

impl ThunderEventHandlerProvider for TimezoneChangedEventHandler {
    type EVENT = TimeZone;
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
        "onTimeZoneDSTChanged".into()
    }

    fn get_mapped_event() -> String {
        TIME_ZONE_CHANGED.into()
    }

    fn module() -> String {
        ThunderPlugin::System.callsign_string()
    }

    fn get_id(&self) -> String {
        Self::get_mapped_event()
    }

    fn get_extn_event(
        _r: Self::EVENT,
        _callback_type: DeviceEventCallback,
    ) -> Result<ExtnEvent, RippleError> {
        Err(RippleError::InvalidOutput)
    }
}
