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

use serde::{Deserialize, Serialize};

use super::fb_discovery::NavigationIntent;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SecondScreenEvent {
    #[serde(rename = "type")]
    pub _type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
pub struct AppInitParameters {
    pub us_privacy: Option<String>,
    pub lmt: Option<u16>,
    pub discovery: Option<DiscoveryEvent>,
    #[serde(rename = "secondScreen", skip_serializing_if = "Option::is_none")]
    pub second_screen: Option<SecondScreenEvent>,
}

#[derive(Serialize, Debug, Clone)]
pub struct DiscoveryEvent {
    #[serde(rename = "navigateTo")]
    pub navigate_to: NavigationIntent,
}
