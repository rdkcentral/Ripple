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
    client::thunder_plugin::ThunderPlugin,
    events::thunder_event_processor::{
        ThunderEventHandler, ThunderEventHandlerProvider, ThunderEventMessage,
    },
    ripple_sdk::{
        api::device::{
            device_events::{DeviceEventCallback, HDCP_CHANGED_EVENT},
            device_request::{AudioProfile, HdcpProfile},
        },
        log::debug,
        tokio,
    },
    thunder_state::ThunderState,
};
use std::collections::HashMap;

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
