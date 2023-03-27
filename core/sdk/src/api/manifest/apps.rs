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

use crate::api::device::device_browser::BrowserProps;

const X_DEFAULT: u32 = 0;
const Y_DEFAULT: u32 = 0;
const W_DEFAULT: u32 = 1920;
const H_DEFAULT: u32 = 1080;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Capability {
    pub required: Vec<String>,
    pub optional: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AppCapabilities {
    pub used: Capability,
    pub managed: Capability,
    pub provided: Capability,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppProperties {
    pub user_agent: Option<String>,
    pub http_cookie_accept_policy: Option<String>,
    pub local_storage_enabled: Option<bool>,
    pub languages: Option<String>,
    pub headers: Option<String>,
}

impl AppProperties {
    pub fn get_browser_props(self) -> BrowserProps {
        BrowserProps {
            headers: self.headers,
            http_cookie_accept_policy: self.http_cookie_accept_policy,
            languages: self.languages,
            local_storage_enabled: self.local_storage_enabled,
            user_agent: self.user_agent,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppManifest {
    pub app_key: String,
    pub name: String,
    pub start_page: String,
    pub content_catalog: Option<String>,
    pub runtime: String,
    #[serde(default = "x_default")]
    pub x: u32,
    #[serde(default = "y_default")]
    pub y: u32,
    #[serde(default = "w_default")]
    pub w: u32,
    #[serde(default = "h_default")]
    pub h: u32,
    pub capabilities: AppCapabilities,
    pub properties: Option<AppProperties>,
}

impl AppManifest {
    pub fn requires_capability(&self, cap: &'static str) -> bool {
        self.capabilities.used.required.contains(&String::from(cap))
    }
}

fn x_default() -> u32 {
    X_DEFAULT
}

fn y_default() -> u32 {
    Y_DEFAULT
}

fn w_default() -> u32 {
    W_DEFAULT
}

fn h_default() -> u32 {
    H_DEFAULT
}
