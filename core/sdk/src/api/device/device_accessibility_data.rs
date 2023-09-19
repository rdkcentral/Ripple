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

use serde::{Deserialize, Serialize, Serializer};

#[derive(Serialize, Deserialize)]
pub struct ClosedCaptionsSettings {
    pub enabled: bool,
    pub styles: ClosedCaptionStyle,
}

pub const FONT_FAMILY_LIST: [&str; 5] = ["sans-serif", "serif", "monospace", "cursive", "fantasy"];
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClosedCaptionStyle {
    pub font_family: String,
    pub font_size: f32,
    pub font_color: String,
    pub font_edge: String,
    pub font_edge_color: String,
    pub font_opacity: u32,
    pub background_color: String,
    pub background_opacity: u32,
    pub text_align: String,
    pub text_align_vertical: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VoiceGuidanceSettings {
    pub enabled: bool,
    #[serde(serialize_with = "speed_serializer")]
    pub speed: f32,
}

fn speed_serializer<S>(speed: &f32, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let s = (*speed as f64 * 100.0).ceil() / 100.0;
    serializer.serialize_f64(s)
}

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct VoiceGuidanceEnabledChangedEventData {
    pub state: bool,
}
