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

use serde::{Deserialize, Serialize};

use crate::api::device::device_browser::BrowserProps;

const X_DEFAULT: u32 = 0;
const Y_DEFAULT: u32 = 0;
const W_DEFAULT: u32 = 1920;
const H_DEFAULT: u32 = 1080;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Default)]
pub struct Capability {
    pub required: Vec<String>,
    pub optional: Vec<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Default)]
pub struct AppCapabilities {
    pub used: Capability,
    pub managed: Capability,
    pub provided: Capability,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
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

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
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

impl Default for AppManifest {
    fn default() -> Self {
        AppManifest {
            app_key: String::from("xrn:firebolt:application:test"),
            name: String::from("test"),
            start_page: String::from("https://firecertapp.firecert.comcast.com/prod/index.html"),
            content_catalog: None,
            runtime: String::from("Web"),
            x: x_default(),
            y: y_default(),
            w: w_default(),
            h: h_default(),
            capabilities: AppCapabilities {
                used: Capability {
                    required: [].to_vec(),
                    optional: [].to_vec(),
                },
                managed: Capability {
                    required: [].to_vec(),
                    optional: [].to_vec(),
                },
                provided: Capability {
                    required: [].to_vec(),
                    optional: [].to_vec(),
                },
            },
            properties: None,
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_browser_props() {
        let app_props = AppProperties {
            user_agent: Some(String::from("Mozilla/5.0")),
            http_cookie_accept_policy: Some(String::from("strict")),
            local_storage_enabled: Some(true),
            languages: Some(String::from("en-US")),
            headers: Some(String::from("Content-Type: application/json")),
        };

        let browser_props = app_props.get_browser_props();

        assert_eq!(browser_props.user_agent, Some(String::from("Mozilla/5.0")));
        assert_eq!(
            browser_props.http_cookie_accept_policy,
            Some(String::from("strict"))
        );
        assert_eq!(browser_props.local_storage_enabled, Some(true));
        assert_eq!(browser_props.languages, Some(String::from("en-US")));
        assert_eq!(
            browser_props.headers,
            Some(String::from("Content-Type: application/json"))
        );
    }

    #[test]
    fn test_requires_capability() {
        let app_manifest = AppManifest {
            app_key: String::from("xrn:firebolt:application:test"),
            name: String::from("test"),
            start_page: String::from("https://firecertapp.firecert.comcast.com/prod/index.html"),
            content_catalog: None,
            runtime: String::from("Web"),
            x: X_DEFAULT,
            y: Y_DEFAULT,
            w: W_DEFAULT,
            h: H_DEFAULT,
            capabilities: AppCapabilities {
                used: Capability {
                    required: vec![String::from("capability1"), String::from("capability2")],
                    optional: vec![String::from("capability3")],
                },
                managed: Capability::default(),
                provided: Capability::default(),
            },
            properties: None,
        };

        assert!(app_manifest.requires_capability("capability1"));
        assert!(app_manifest.requires_capability("capability2"));
        assert!(!app_manifest.requires_capability("capability3"));
        assert!(!app_manifest.requires_capability("capability4"));
    }
}
