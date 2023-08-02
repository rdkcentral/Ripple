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

use std::hash::{Hash, Hasher};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::api::gateway::rpc_error::RpcError;

use super::fb_openrpc::CapabilitySet;

/// There are many types of Firebolt Cap enums
/// 1. Short: `device:model` becomes = `xrn:firebolt:capability:account:session` its just a handy cap which helps us write less code
/// 2. Full: Contains the full string for capability typically loaded from Manifest and Firebolt SDK which contains the full string
#[derive(Debug, Clone)]
pub enum FireboltCap {
    Short(String),
    Full(String),
}

impl FireboltCap {
    pub fn short<S>(s: S) -> FireboltCap
    where
        S: Into<String>,
    {
        FireboltCap::Short(s.into())
    }
}

impl Eq for FireboltCap {}

impl PartialEq for FireboltCap {
    fn eq(&self, other: &Self) -> bool {
        self.as_str().eq(&other.as_str())
    }
}

impl Hash for FireboltCap {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl FireboltCap {
    pub fn as_str(&self) -> String {
        let prefix = "xrn:firebolt:capability:";
        match self {
            Self::Full(s) => s.clone(),
            Self::Short(s) => format!("{}{}", prefix, s).to_lowercase(),
        }
    }

    pub fn parse(cap: String) -> Option<FireboltCap> {
        let prefix = vec!["xrn", "firebolt", "capability"];
        let c_a = cap.split(":");
        if c_a.count() > 1 {
            let c_a = cap.split(":");
            let mut cap_vec = Vec::<String>::new();
            for c in c_a.into_iter() {
                if !prefix.contains(&c) {
                    cap_vec.push(String::from(c));
                    if cap_vec.len() == 2 {
                        return Some(FireboltCap::Short(cap_vec.join(":")));
                    }
                }
            }
        }

        None
    }

    pub fn from_vec_string(cap_strings: Vec<String>) -> Vec<FireboltCap> {
        cap_strings
            .into_iter()
            .filter(|x| FireboltCap::parse(x.clone()).is_some())
            .map(|x| FireboltCap::Full(x.clone()))
            .collect()
    }
}

#[derive(Eq, Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CapabilityRole {
    Use,
    Manage,
    Provide,
}

impl CapabilityRole {
    pub fn as_string(&self) -> &'static str {
        match self {
            CapabilityRole::Use => "use",
            CapabilityRole::Manage => "manage",
            CapabilityRole::Provide => "provide",
        }
    }
}

impl Hash for CapabilityRole {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u8(match self {
            CapabilityRole::Use => 0,
            CapabilityRole::Manage => 1,
            CapabilityRole::Provide => 2,
        });
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FireboltPermission {
    pub cap: FireboltCap,
    pub role: CapabilityRole,
}

impl From<RoleInfo> for FireboltPermission {
    fn from(role_info: RoleInfo) -> Self {
        FireboltPermission {
            cap: FireboltCap::Full(role_info.capability.to_owned()),
            role: role_info.role.unwrap_or(CapabilityRole::Use),
        }
    }
}
impl From<CapabilitySet> for Vec<FireboltPermission> {
    fn from(cap_set: CapabilitySet) -> Self {
        let mut fb_perm_list = Vec::new();
        if let Some(use_caps) = cap_set.use_caps {
            for cap in use_caps {
                fb_perm_list.push(FireboltPermission {
                    cap: cap.clone(),
                    role: CapabilityRole::Use,
                });
            }
        }
        if let Some(manage_caps) = cap_set.manage_caps {
            for cap in manage_caps {
                fb_perm_list.push(FireboltPermission {
                    cap: cap.clone(),
                    role: CapabilityRole::Manage,
                });
            }
        }
        if let Some(provide_cap) = cap_set.provide_cap {
            fb_perm_list.push(FireboltPermission {
                cap: provide_cap.clone(),
                role: CapabilityRole::Provide,
            });
        }
        fb_perm_list
    }
}

impl Serialize for FireboltPermission {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = self.cap.as_str();
        let suffix = match self.role {
            CapabilityRole::Use => "",
            CapabilityRole::Manage => "[manage]",
            CapabilityRole::Provide => "[provide]",
        };
        serializer.serialize_str(&format!("{}{}", s, suffix))
    }
}

impl<'de> Deserialize<'de> for FireboltPermission {
    fn deserialize<D>(deserializer: D) -> Result<FireboltPermission, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut str = String::deserialize(deserializer)?;
        let mut role = CapabilityRole::Use;
        let mut cap = str.clone();
        if str.ends_with("[manage]") {
            role = CapabilityRole::Manage;
            str.truncate(str.len() - "[manage]".len());
            cap = str;
        } else if str.ends_with("[provide]") {
            role = CapabilityRole::Provide;
            str.truncate(str.len() - "[provide]".len());
            cap = str;
        }
        Ok(FireboltPermission {
            cap: FireboltCap::Full(cap),
            role,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolePermission {
    pub permitted: bool,
    pub granted: bool,
}

impl RolePermission {
    pub fn get(permitted: bool, granted: bool) -> RolePermission {
        RolePermission { permitted, granted }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityInfo {
    pub capability: String,
    pub supported: bool,
    pub available: bool,
    #[serde(rename = "use")]
    pub _use: RolePermission,
    pub manage: RolePermission,
    pub provide: RolePermission,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Vec<DenyReason>>,
}

impl CapabilityInfo {
    pub fn get(cap: String, reason: Option<DenyReason>) -> CapabilityInfo {
        let (mut supported, mut available, mut permitted, mut granted) = (true, true, true, true);
        let mut details = None;
        if let Some(r) = reason.clone() {
            details = Some(vec![r.clone()]);
            match r {
                DenyReason::Unsupported => {
                    supported = false;
                    available = false;
                    permitted = false;
                    granted = false;
                }
                DenyReason::Unavailable => {
                    available = false;
                    permitted = false;
                    granted = false;
                }
                DenyReason::Unpermitted => {
                    permitted = false;
                    granted = false;
                }
                DenyReason::Ungranted => {
                    granted = false;
                }
                DenyReason::GrantDenied => {
                    granted = false;
                }
                _ => {}
            }
        }
        CapabilityInfo {
            capability: cap,
            supported: supported,
            available: available,
            _use: RolePermission::get(permitted, granted),
            manage: RolePermission::get(permitted, granted),
            provide: RolePermission::get(permitted, granted),
            details,
        }
    }

    pub fn update_ungranted(&mut self, role: &CapabilityRole, granted: bool) {
        if let Some(details) = self.details.as_mut() {
            if let Some(index) = details.iter().position(|x| x == &DenyReason::Ungranted) {
                details.remove(index);
                if !granted {
                    details.push(DenyReason::GrantDenied)
                } else {
                    match role {
                        CapabilityRole::Use => self._use.granted = granted,
                        CapabilityRole::Manage => self.manage.granted = granted,
                        CapabilityRole::Provide => self.provide.granted = granted,
                    }
                }
            }
            if details.len() == 0 {
                self.details = None;
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum DenyReason {
    NotFound,
    Unpermitted,
    Unsupported,
    Disabled,
    Unavailable,
    GrantDenied,
    Ungranted,
    GrantProviderMissing,
}

pub const CAPABILITY_NOT_AVAILABLE: i32 = -50300;

pub const CAPABILITY_NOT_SUPPORTED: i32 = -50100;

pub const CAPABILITY_GET_ERROR: i32 = -50200;

pub const CAPABILITY_NOT_PERMITTED: i32 = -40300;

pub const JSON_RPC_STANDARD_ERROR_INVALID_PARAMS: i32 = -32602;

pub const JSON_RPC_STANDARD_ERROR_METHOD_NOT_FOUND: i32 = -32601;

impl RpcError for DenyReason {
    type E = Vec<String>;
    fn get_rpc_error_code(&self) -> i32 {
        match self {
            Self::Unavailable => CAPABILITY_NOT_AVAILABLE,
            Self::Unsupported => CAPABILITY_NOT_SUPPORTED,
            Self::GrantDenied => CAPABILITY_NOT_PERMITTED,
            Self::Unpermitted => CAPABILITY_NOT_PERMITTED,
            Self::NotFound => JSON_RPC_STANDARD_ERROR_METHOD_NOT_FOUND,
            _ => CAPABILITY_GET_ERROR,
        }
    }

    fn get_rpc_error_message(&self, caps: Vec<String>) -> String {
        let caps_disp = caps.clone().join(",");
        match self {
            Self::Unavailable => format!("{} is not available", caps_disp),
            Self::Unsupported => format!("{} is not supported", caps_disp),
            Self::GrantDenied => format!("The user denied access to {}", caps_disp),
            Self::Unpermitted => format!("{} is not permitted", caps_disp),
            Self::NotFound => format!("Method not Found"),
            _ => format!("Error with {}", caps_disp),
        }
    }
}

#[derive(Debug)]
pub struct DenyReasonWithCap {
    pub reason: DenyReason,
    pub caps: Vec<FireboltCap>,
}

impl DenyReasonWithCap {
    pub fn new(reason: DenyReason, caps: Vec<FireboltCap>) -> DenyReasonWithCap {
        DenyReasonWithCap { reason, caps }
    }

    pub fn add_caps(&mut self, caps: Vec<FireboltCap>) {
        self.caps.extend(caps)
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct CapRequestRpcRequest {
    pub grants: Vec<RoleInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RoleInfo {
    pub role: Option<CapabilityRole>,
    pub capability: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CapInfoRpcRequest {
    pub capabilities: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CapListenRPCRequest {
    pub capability: String,
    pub listen: bool,
    pub role: Option<CapabilityRole>,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CapEvent {
    OnAvailable,
    OnUnavailable,
    OnGranted,
    OnRevoked,
}

impl CapEvent {
    pub fn as_str(self) -> String {
        serde_json::to_string(&self).unwrap()
    }
}
