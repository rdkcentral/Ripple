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

use jsonrpsee::core::Error;
use ripple_sdk::{
    api::device::{device_operator::DeviceResponseMessage, device_request::AudioProfile},
    serde_json::Value,
};
use serde::Deserialize;

pub fn get_audio_profile_from_value(value: Value) -> HashMap<AudioProfile, bool> {
    let mut hm: HashMap<AudioProfile, bool> = HashMap::new();
    hm.insert(AudioProfile::Stereo, false);
    hm.insert(AudioProfile::DolbyDigital5_1, false);
    hm.insert(AudioProfile::DolbyDigital5_1Plus, false);
    hm.insert(AudioProfile::DolbyDigital7_1, false);
    hm.insert(AudioProfile::DolbyDigital7_1Plus, false);
    hm.insert(AudioProfile::DolbyAtmos, false);

    if value.get("supportedAudioFormat").is_none() {
        return hm;
    }
    let supported_profiles = value["supportedAudioFormat"].as_array().unwrap();
    for profile in supported_profiles {
        let profile_name = profile.as_str().unwrap();
        match profile_name {
            "PCM" => {
                hm.insert(AudioProfile::Stereo, true);
                hm.insert(AudioProfile::DolbyDigital5_1, true);
            }
            "DOLBY AC3" => {
                hm.insert(AudioProfile::Stereo, true);
                hm.insert(AudioProfile::DolbyDigital5_1, true);
            }
            "DOLBY EAC3" => {
                hm.insert(AudioProfile::Stereo, true);
                hm.insert(AudioProfile::DolbyDigital5_1, true);
            }
            "DOLBY AC4" => {
                hm.insert(AudioProfile::Stereo, true);
                hm.insert(AudioProfile::DolbyDigital5_1, true);
                hm.insert(AudioProfile::DolbyDigital7_1, true);
            }
            "DOLBY TRUEHD" => {
                hm.insert(AudioProfile::Stereo, true);
                hm.insert(AudioProfile::DolbyDigital5_1, true);
                hm.insert(AudioProfile::DolbyDigital7_1, true);
            }
            "DOLBY EAC3 ATMOS" => {
                hm.insert(AudioProfile::Stereo, true);
                hm.insert(AudioProfile::DolbyDigital5_1, true);
                hm.insert(AudioProfile::DolbyDigital7_1, true);
                hm.insert(AudioProfile::DolbyAtmos, true);
            }
            "DOLBY TRUEHD ATMOS" => {
                hm.insert(AudioProfile::Stereo, true);
                hm.insert(AudioProfile::DolbyDigital5_1, true);
                hm.insert(AudioProfile::DolbyDigital7_1, true);
                hm.insert(AudioProfile::DolbyAtmos, true);
            }
            "DOLBY AC4 ATMOS" => {
                hm.insert(AudioProfile::Stereo, true);
                hm.insert(AudioProfile::DolbyDigital5_1, true);
                hm.insert(AudioProfile::DolbyDigital7_1, true);
                hm.insert(AudioProfile::DolbyAtmos, true);
            }
            _ => (),
        }
    }
    hm
}

pub fn check_thunder_response_success(response: &DeviceResponseMessage) -> bool {
    let r = response.message.get("success");
    if let Some(r) = r {
        r.as_bool().unwrap_or_default()
    } else {
        false
    }
}

#[derive(Deserialize)]
pub struct ThunderErrorResponse {
    pub error: Value,
}

pub fn get_error_value(error: &Error) -> Value {
    if let jsonrpsee::core::Error::Request(s) = error {
        if let Ok(v) = ripple_sdk::serde_json::from_str::<ThunderErrorResponse>(s) {
            return v.error;
        }
    }
    Value::Null
}
