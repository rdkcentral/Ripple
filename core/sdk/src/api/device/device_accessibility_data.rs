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
use log::info;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};

use super::device_request::DeviceRequest;

pub const VOICE_GUIDANCE_SETTINGS_CHANGED_EVENT: &'static str =
    "accessibility.onVoiceGuidanceSettingsChanged";
pub const VOICE_GUIDANCE_ENABLED_CHANGED_EVENT: &'static str = "voiceguidance.onEnabledChanged";
pub const VOICE_GUIDANCE_SPEED_CHANGED_EVENT: &'static str = "voiceguidance.onSpeedChanged";

pub const CLOSED_CAPTION_SETTING_CHANGED_EVENT: &'static str =
    "accessibility.onClosedCaptionsSettingsChanged";
pub const CLOSED_CAPTION_ENABLED_CHANGED_EVENT: &'static str =
    "accessibility.onClosedCaptionsSettingsEnabledChanged";
pub const CLOSED_CAPTION_FONT_FAMILY_CHANGED_EVENT: &'static str =
    "accessibility.onClosedCaptionsSettingsFontFamilyChanged";
pub const CLOSED_CAPTION_FONT_SIZE_CHANGED_EVENT: &'static str =
    "accessibility.onClosedCaptionsSettingsFontSizeChanged";
pub const CLOSED_CAPTION_FONT_COLOR_CHANGED_EVENT: &'static str =
    "accessibility.onClosedCaptionsSettingsFontColorChanged";
pub const CLOSED_CAPTION_FONT_EDGE_CHANGED_EVENT: &'static str =
    "accessibility.onClosedCaptionsSettingsFontEdgeChanged";
pub const CLOSED_CAPTION_FONT_EDGE_COLOR_CHANGED_EVENT: &'static str =
    "accessibility.onClosedCaptionsSettingsFontEdgeColorChanged";
pub const CLOSED_CAPTION_FONT_OPACITY_CHANGED_EVENT: &'static str =
    "accessibility.onClosedCaptionsSettingsFontOpacityChanged";
pub const CLOSED_CAPTION_BACKGROUND_COLOR_CHANGED_EVENT: &'static str =
    "accessibility.onClosedCaptionsSettingsBackgroundColorChanged";
pub const CLOSED_CAPTION_BACKGROUND_OPACITY_CHANGED_EVENT: &'static str =
    "accessibility.onClosedCaptionsSettingsBackgroundOpacityChanged";
pub const CLOSED_CAPTION_TEXT_ALIGN_CHANGED_EVENT: &'static str =
    "accessibility.onClosedCaptionsSettingsTextAlignChanged";
pub const CLOSED_CAPTION_TEXT_ALIGN_VERTICAL_CHANGED_EVENT: &'static str =
    "accessibility.onClosedCaptionsSettingsTextAlignVerticalChanged";

#[derive(Deserialize, Debug)]
pub struct SetBoolProperty {
    pub value: bool,
}

#[derive(Deserialize, Debug)]
pub struct SetStringProperty {
    pub value: String,
}

#[derive(Deserialize, Debug)]
pub struct SetU32Property {
    pub value: u32,
}

#[derive(Deserialize, Debug)]
pub struct SetF32Property {
    pub value: f32,
}

#[derive(Deserialize, Debug)]
pub struct OpacityProperty {
    // #[serde(with = "opacity_serde")]
    pub value: u32,
}

#[derive(Serialize, Deserialize)]
pub struct ClosedCaptionsSettings {
    pub enabled: bool,
    pub styles: Vec<ClosedCaptionStyle>,
}

pub const FONT_FAMILY_LIST: [&str; 5] = ["sans-serif", "serif", "monospace", "cursive", "fantasy"];
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClosedCaptionStyle {
    pub font_family: String,
    pub font_size: u32,
    pub font_color: String,
    pub font_edge: String,
    pub font_edge_color: String,
    pub font_opacity: u32,
    pub background_color: String,
    pub background_opacity: u32,
    pub text_align: String,
    pub text_align_vertical: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceGuidanceSettings {
    enabled: bool,
    speed: f32,
}

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct VoiceGuidanceEnabledChangedEventData {
    pub state: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GetStorageProperty {
    pub namespace: String,
    pub key: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SetStorageProperty {
    pub namespace: String,
    pub key: String,
    pub value: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum StorageRequest {
    Get(GetStorageProperty),
    Set(SetStorageProperty),
}

impl ExtnPayloadProvider for StorageRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        info!("inside etn paylod");
        ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Storage(self.clone())))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Request(request) => match request {
                ExtnRequest::Device(r) => match r {
                    DeviceRequest::Storage(d) => return Some(d),
                    _ => {}
                },
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn contract() -> RippleContract {
        RippleContract::Storage
    }
}
