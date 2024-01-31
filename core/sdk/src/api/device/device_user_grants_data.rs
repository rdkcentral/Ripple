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

use crate::api::firebolt::fb_capabilities::{
    CapabilityRole, DenyReason, FireboltCap, FireboltPermission,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
    time::{Duration, SystemTime},
};

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GrantStep {
    pub capability: String,
    pub configuration: Option<Value>,
}

impl GrantStep {
    pub fn capability_as_fb_cap(&self) -> FireboltCap {
        FireboltCap::Full(self.capability.clone())
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GrantRequirements {
    pub steps: Vec<GrantStep>,
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

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AutoApplyPolicy {
    Always,
    Allowed,
    Disallowed,
    Never,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GrantPrivacySetting {
    pub property: String,
    pub auto_apply_policy: AutoApplyPolicy,
    pub update_property: bool,
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

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PolicyPersistenceType {
    Account,
    Device,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum EvaluateAt {
    Invocation,
    ActiveSession,
    LoadedSession,
}
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GrantPolicy {
    #[serde(default = "default_evaluate_at")]
    pub evaluate_at: Vec<EvaluateAt>,
    pub options: Vec<GrantRequirements>,
    pub scope: GrantScope,
    pub lifespan: GrantLifespan,
    pub overridable: bool,
    pub lifespan_ttl: Option<u64>,
    pub privacy_setting: Option<GrantPrivacySetting>,
    #[serde(default = "default_policy_persistence_type")]
    pub persistence: PolicyPersistenceType,
}
pub fn default_evaluate_at() -> Vec<EvaluateAt> {
    vec![EvaluateAt::Invocation]
}

pub fn default_policy_persistence_type() -> PolicyPersistenceType {
    PolicyPersistenceType::Device
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
            persistence: PolicyPersistenceType::Device,
            evaluate_at: vec![EvaluateAt::Invocation],
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct GrantPolicies {
    #[serde(rename = "use")]
    pub use_: Option<GrantPolicy>,
    pub manage: Option<GrantPolicy>,
    pub provide: Option<GrantPolicy>,
}

impl GrantPolicies {
    pub fn get_policy(&self, permission: &FireboltPermission) -> Option<GrantPolicy> {
        match permission.role {
            CapabilityRole::Use => {
                if let Some(value) = &self.use_ {
                    return Some(value.clone());
                }
            }
            CapabilityRole::Manage => {
                if let Some(manage) = &self.manage {
                    return Some(manage.clone());
                }
            }
            CapabilityRole::Provide => {
                if let Some(provide) = &self.provide {
                    return Some(provide.clone());
                }
            }
        }
        None
    }
}

impl GrantPolicy {
    pub fn get_steps_without_grant(&self) -> Option<Vec<GrantStep>> {
        let mut grant_steps = Vec::new();
        for grant_requirements in &self.options {
            for step in &grant_requirements.steps {
                if !step
                    .capability
                    .starts_with("xrn:firebolt:capability:usergrant:")
                {
                    grant_steps.push(step.clone());
                }
            }
        }
        if !grant_steps.is_empty() {
            return Some(grant_steps);
        }
        None
    }
}
#[derive(Deserialize, Debug, Clone, Default)]
pub struct GrantExclusionFilter {
    pub capability: Option<String>,
    pub id: Option<String>,
    pub catalog: Option<String>,
}

#[derive(Eq, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GrantStatus {
    Allowed,
    Denied,
}

impl GrantStatus {
    pub fn as_string(&self) -> &'static str {
        match self {
            GrantStatus::Allowed => "granted",
            GrantStatus::Denied => "denied",
        }
    }
}

impl From<GrantStatus> for Result<(), DenyReason> {
    fn from(grant_status: GrantStatus) -> Self {
        match grant_status {
            GrantStatus::Allowed => Ok(()),
            GrantStatus::Denied => Err(DenyReason::GrantDenied),
        }
    }
}

impl Hash for GrantStatus {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u8(match self {
            GrantStatus::Allowed => 0,
            GrantStatus::Denied => 1,
        });
    }
}

#[derive(Eq, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GrantStateModify {
    Grant,
    Deny,
    Clear,
}

#[derive(Eq, Clone, Debug, Serialize, Deserialize)]
pub struct GrantEntry {
    pub role: CapabilityRole,
    pub capability: String,
    pub status: Option<GrantStatus>,
    pub lifespan: Option<GrantLifespan>,
    pub last_modified_time: Duration,
    pub lifespan_ttl_in_secs: Option<u64>,
}

impl PartialEq for GrantEntry {
    fn eq(&self, other: &Self) -> bool {
        self.role == other.role && self.capability == other.capability
    }
}

impl Hash for GrantEntry {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.role.hash(state);
        self.capability.hash(state);
    }
}

impl GrantEntry {
    pub fn get(role: CapabilityRole, capability: String) -> GrantEntry {
        GrantEntry {
            role,
            capability,
            status: None,
            lifespan: None,
            last_modified_time: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap(),
            lifespan_ttl_in_secs: None,
        }
    }

    pub fn has_expired(&self) -> bool {
        match self.lifespan {
            Some(GrantLifespan::Seconds) => match self.lifespan_ttl_in_secs {
                None => true,
                Some(ttl) => {
                    let elapsed_time = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .checked_sub(self.last_modified_time)
                        .unwrap_or(Duration::from_secs(0));

                    elapsed_time > Duration::from_secs(ttl)
                }
            },
            Some(GrantLifespan::Once) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum GrantActiveState {
    ActiveGrant(Result<(), DenyReason>),
    PendingGrant,
}

#[derive(Debug, Default)]
pub struct GrantErrors {
    pub ungranted: HashSet<FireboltCap>,
    pub denied: HashSet<FireboltCap>,
}

impl GrantErrors {
    pub fn add_ungranted(&mut self, cap: FireboltCap) {
        self.ungranted.insert(cap);
    }

    pub fn add_denied(&mut self, cap: FireboltCap) {
        self.denied.insert(cap);
    }

    pub fn has_errors(&self) -> bool {
        !self.ungranted.is_empty() || !self.denied.is_empty()
    }

    pub fn get_reason(&self, cap: &FireboltCap) -> Option<DenyReason> {
        if self.ungranted.contains(cap) {
            Some(DenyReason::Ungranted)
        } else if self.denied.contains(cap) {
            Some(DenyReason::GrantDenied)
        } else {
            None
        }
    }
}
