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

use std::hash::{Hash, Hasher};

use ripple_sdk::api::manifest::device_manifest::GrantStep;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GrantPolicy {
    pub options: Vec<GrantRequirements>,
    pub scope: GrantScope,
    pub lifespan: GrantLifespan,
    pub overridable: bool,
    pub lifespan_ttl: Option<u64>,
    pub privacy_setting: Option<GrantPrivacySetting>,
}

impl Default for GrantPolicy {
    fn default() -> Self {
        GrantPolicy {
            options: Default::default(),
            scope: GrantScope::Device,
            lifespan: GrantLifespan::Once,
            overridable: true,
            lifespan_ttl: None,
            privacy_setting: None,
        }
    }
}

#[derive(Eq, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum GrantLifespan {
    Once,
    Forever,
    AppActive,
    PowerActive,
    Seconds,
}

impl GrantLifespan {
    pub fn as_string(&self) -> &'static str {
        match self {
            GrantLifespan::Once => "once",
            GrantLifespan::Forever => "forever",
            GrantLifespan::AppActive => "appActive",
            GrantLifespan::PowerActive => "powerActive",
            GrantLifespan::Seconds => "seconds",
        }
    }
}

impl Hash for GrantLifespan {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u8(match self {
            GrantLifespan::Once => 0,
            GrantLifespan::Forever => 1,
            GrantLifespan::AppActive => 2,
            GrantLifespan::PowerActive => 3,
            GrantLifespan::Seconds => 4,
        });
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GrantRequirements {
    pub steps: Vec<GrantStep>,
}

#[derive(Eq, PartialEq, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum GrantScope {
    App,
    Device,
}

impl Hash for GrantScope {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u8(match self {
            GrantScope::App => 0,
            GrantScope::Device => 1,
        });
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GrantPrivacySetting {
    pub property: String,
    pub auto_apply_policy: AutoApplyPolicy,
    pub update_property: bool,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum AutoApplyPolicy {
    Always,
    Allowed,
    Disallowed,
    Never,
}
