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
#[serde(rename_all = "camelCase")]
pub struct ClosedCaptionsSettings {
    pub enabled: bool,
    pub styles: ClosedCaptionStyle,
    pub preferred_languages: Vec<String>,
}

pub const FONT_FAMILY_LIST: [&str; 7] = [
    "monospaced_serif",
    "proportional_serif",
    "monospaced_sanserif",
    "proportional_sanserif",
    "smallcaps",
    "cursive",
    "casual",
];
pub const FONT_EDGE_LIST: [&str; 6] = [
    "none",
    "raised",
    "depressed",
    "uniform",
    "drop_shadow_left",
    "drop_shadow_right",
];

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClosedCaptionStyle {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_edge: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_edge_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_opacity: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_opacity: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_opacity: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_align: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_align_vertical: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
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

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct AudioDescriptionSettings {
    pub enabled: bool,
}

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct AudioDescriptionSettingsSet {
    pub value: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use serde_json::ser::Serializer;

    #[rstest]
    #[case(2.0, "2.0")]
    #[case(0.50, "0.5")]
    #[case(2.51, "2.51")]
    #[case(0.491, "0.5")]
    fn test_speed_serializer_various_values(
        #[case] input_speed: f32,
        #[case] expected_output: &str,
    ) {
        let mut buf = Vec::new();
        let mut serializer = Serializer::new(&mut buf);
        speed_serializer(&input_speed, &mut serializer).unwrap();
        let serialized_str = String::from_utf8(buf).unwrap();
        assert_eq!(serialized_str, expected_output);
    }
}
