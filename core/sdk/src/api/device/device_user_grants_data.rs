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
use crate::api::firebolt::fb_capabilities::{CapabilityRole, FireboltPermission};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::hash::{Hash, Hasher};

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GrantStep {
    pub capability: String,
    pub configuration: Option<Value>,
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

#[derive(Deserialize, Debug, Clone, PartialEq)]
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

#[derive(Deserialize, Debug, Clone)]
pub struct GrantPolicies {
    #[serde(rename = "use")]
    pub _use: Option<GrantPolicy>,
    pub manage: Option<GrantPolicy>,
    pub provide: Option<GrantPolicy>,
}

impl GrantPolicies {
    pub fn get_policy(&self, permission: &FireboltPermission) -> Option<GrantPolicy> {
        match permission.role {
            CapabilityRole::Use => {
                if self._use.is_some() {
                    return Some(self._use.clone().unwrap());
                }
            }
            CapabilityRole::Manage => {
                if self.manage.is_some() {
                    return Some(self.manage.clone().unwrap());
                }
            }
            CapabilityRole::Provide => {
                if self.manage.is_some() {
                    return Some(self.manage.clone().unwrap());
                }
            }
        }
        None
    }
}

// impl GrantRequirements {
//     /*
//      * We can execute a grant requirement only if the caps defined in its
//      * steps are supported and available. This functions checks if all the
//      * steps in the requirements are supported and available.
//      */
//     async fn has_steps_for_execution(
//         &self,
//         platform_state: &PlatformState,
//         call_ctx: &CallContext,
//     ) -> bool {
//         let mut supported_and_available = true;
//         for step in &self.steps {
//             let firebolt_cap = FireboltCap::Full(step.capability.to_owned());
//             debug!(
//                 "checking if the cap is supported & available: {:?}",
//                 firebolt_cap.as_str()
//             );
//             if platform_state
//                 .services
//                 .is_cap_supported(firebolt_cap.clone())
//                 .await
//                 && platform_state
//                     .services
//                     .is_cap_available(firebolt_cap.clone())
//                     .await
//             {
//                 supported_and_available = true;
//             } else {
//                 supported_and_available = false;
//                 break;
//             }
//         }
//         supported_and_available
//     }

//     async fn execute(
//         &self,
//         platform_state: &PlatformState,
//         call_ctx: &CallContext,
//     ) -> Result<(), DenyReason> {
//         for step in &self.steps {
//             step.execute(platform_state, call_ctx).await?
//         }
//         Ok(())
//     }
// }

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
        if grant_steps.len() > 0 {
            return Some(grant_steps);
        }
        None
    }
}

// impl GrantPolicy {
//     async fn evaluate_options(
//         &self,
//         platform_state: &PlatformState,
//         call_ctx: &CallContext,
//     ) -> Result<(), DenyReason> {
//         let mut result: Result<(), DenyReason> = Err(DenyReason::GrantDenied);
//         for grant_requirements in &self.options {
//             if grant_requirements
//                 .has_steps_for_execution(platform_state, call_ctx)
//                 .await
//             {
//                 result = grant_requirements.execute(platform_state, call_ctx).await;
//                 break;
//             }
//         }
//         result
//     }

//     pub async fn execute(
//         &self,
//         platform_state: &PlatformState,
//         call_ctx: &CallContext,
//     ) -> Result<(), DenyReason> {
//         let mut result: Result<(), DenyReason>;
//         if let Some(_privacy_settings) = &self.privacy_setting {
//             result = Ok(());
//         } else {
//             result = self.evaluate_options(platform_state, call_ctx).await;
//         }
//         result
//     }

//     pub async fn override_policy(&mut self, plaform_state: &PlatformState, call_ctx: &CallContext) {
//     }

//     pub async fn is_valid(&self, platform_state: &PlatformState, call_ctx: &CallContext) -> bool {
//         let mut result = false;
//         if let Some(privacy_settings) = &self.privacy_setting {
//             // Checking if the property is present in firebolt-open-rpc spec
//             if let Some(method) = &platform_state
//                 .firebolt_open_rpc
//                 .methods
//                 .iter()
//                 .find(|x| x.name == privacy_settings.property)
//             {
//                 // Checking if the property tag is havin x-allow-value extension.
//                 if let Some(tags) = &method.tags {
//                     result = tags
//                         .iter()
//                         .find(|x| x.name.starts_with("property:") && x.allow_value.is_some())
//                         .map_or(false, |_| true);
//                 }
//             }
//         } else {
//             // Checking if we have at least one Grant Requirements.
//             result = true;
//             if self.options.len() > 0 {
//                 // Check if all the Granting capabilities are of the format "xrn:firebolt:capability:usergrant:* and have entry in firebolt-spec"
//                 'requirements: for grant_requirements in &self.options {
//                     for step in &grant_requirements.steps {
//                         if !step
//                             .capability
//                             .starts_with("xrn:firebolt:capability:usergrant:")
//                             && CapabilityResolver::get(None)
//                                 .cap_policies
//                                 .contains_key(&step.capability)
//                         {
//                             result = false;
//                             break 'requirements;
//                         }
//                     }
//                 }
//             } else {
//                 result = false;
//             }
//         }
//         result
//     }
// }
