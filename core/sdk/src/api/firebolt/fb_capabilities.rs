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

use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::fb_openrpc::CapabilitySet;
use crate::api::gateway::rpc_error::RpcError;

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

impl Serialize for FireboltCap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.as_str())
    }
}

impl<'de> Deserialize<'de> for FireboltCap {
    fn deserialize<D>(deserializer: D) -> Result<FireboltCap, D::Error>
    where
        D: Deserializer<'de>,
    {
        let cap = String::deserialize(deserializer)?;
        if let Some(fc) = FireboltCap::parse(cap.clone()) {
            Ok(fc)
        } else {
            Err(serde::de::Error::custom(format!(
                "Invalid capability: {}",
                cap
            )))
        }
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
    // TODO: refactor this to use ToString trait instead of confusingly named function
    pub fn as_str(&self) -> String {
        let prefix = "xrn:firebolt:capability:";
        match self {
            Self::Full(s) => s.clone(),
            Self::Short(s) => format!("{}{}", prefix, s).to_lowercase(),
        }
    }

    pub fn parse(cap: String) -> Option<FireboltCap> {
        let mut caps = cap.clone();
        if !cap.starts_with("xrn:firebolt:capability") {
            caps = "xrn:firebolt:capability:".to_string() + cap.as_str();
        }
        FireboltCap::parse_long(caps)
    }

    pub fn parse_long(cap: String) -> Option<FireboltCap> {
        let pattern = r"^xrn:firebolt:capability:([a-z0-9\\-]+)((:[a-z0-9\\-]+)?)$";
        if !Regex::new(pattern).unwrap().is_match(cap.as_str()) {
            return None;
        }

        let prefix = ["xrn", "firebolt", "capability"];
        let c_a = cap.split(':');
        let mut cap_vec = Vec::<String>::new();
        for c in c_a.into_iter() {
            if !prefix.contains(&c) {
                cap_vec.push(String::from(c));
            }
        }
        Some(FireboltCap::Short(cap_vec.join(":")))
    }

    pub fn from_vec_string(cap_strings: Vec<String>) -> Vec<FireboltCap> {
        cap_strings
            .into_iter()
            .filter(|x| FireboltCap::parse(x.clone()).is_some())
            .map(FireboltCap::Full)
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
            cap: role_info.capability.to_owned(),
            role: role_info.role.unwrap_or(CapabilityRole::Use),
        }
    }
}

impl From<FireboltCap> for FireboltPermission {
    fn from(value: FireboltCap) -> Self {
        FireboltPermission {
            cap: value,
            role: CapabilityRole::Use,
        }
    }
}

impl From<CapRequestRpcRequest> for Vec<FireboltPermission> {
    fn from(value: CapRequestRpcRequest) -> Self {
        value
            .grants
            .iter()
            .map(|role_info| FireboltPermission {
                cap: role_info.capability.to_owned(),
                role: role_info.role.unwrap_or(CapabilityRole::Use),
            })
            .collect()
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
                cap: provide_cap,
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

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "contract_tests", derive(Serialize))]
pub struct FireboltPermissions {
    pub capabilities: Vec<FireboltPermission>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolePermission {
    pub permitted: bool,
    pub granted: Option<bool>,
}

impl RolePermission {
    pub fn new(permitted: bool, granted: Option<bool>) -> RolePermission {
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
        let (mut supported, mut available, mut permitted, mut granted) =
            (true, true, true, Some(true));
        let mut details = None;
        if let Some(r) = reason {
            details = Some(vec![r.clone()]);
            match r {
                DenyReason::Unsupported => {
                    supported = false;
                    available = false;
                    permitted = false;
                    granted = None;
                }
                DenyReason::Unavailable => {
                    available = false;
                    permitted = false;
                    granted = None;
                }
                DenyReason::Unpermitted => {
                    permitted = false;
                    granted = None;
                }
                DenyReason::Ungranted => {
                    granted = None;
                }
                DenyReason::GrantDenied => {
                    granted = Some(false);
                }
                _ => {}
            }
        }
        CapabilityInfo {
            capability: cap,
            supported,
            available,
            _use: RolePermission::new(permitted, granted),
            manage: RolePermission::new(permitted, granted),
            provide: RolePermission::new(permitted, granted),
            details,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    AppNotInActiveState,
}
impl std::fmt::Display for DenyReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DenyReason::NotFound => write!(f, "NotFound"),
            DenyReason::Unpermitted => write!(f, "Unpermitted"),
            DenyReason::Unsupported => write!(f, "Unsupported"),
            DenyReason::Disabled => write!(f, "Disabled"),
            DenyReason::Unavailable => write!(f, "Unavailable"),
            DenyReason::GrantDenied => write!(f, "GrantDenied"),
            DenyReason::Ungranted => write!(f, "Ungranted"),
            DenyReason::GrantProviderMissing => write!(f, "GrantProviderMissing"),
            DenyReason::AppNotInActiveState => write!(f, "AppNotInActiveState"),
        }
    }
}

pub const CAPABILITY_NOT_AVAILABLE: i32 = -50300;

pub const CAPABILITY_NOT_SUPPORTED: i32 = -50100;

pub const CAPABILITY_GET_ERROR: i32 = -50200;

pub const CAPABILITY_NOT_PERMITTED: i32 = -40300;

pub const JSON_RPC_STANDARD_ERROR_INVALID_PARAMS: i32 = -32602;

pub const JSON_RPC_STANDARD_ERROR_METHOD_NOT_FOUND: i32 = -32601;

pub const CAPABILITY_GRANT_DENIED: i32 = -40400;

pub const CAPABILITY_UNGRANTED: i32 = -40401;

pub const CAPABILITY_APP_NOT_IN_ACTIVE_STATE: i32 = -40402;

pub const CAPABILITY_GRANT_PROVIDER_MISSING: i32 = -40403;

impl RpcError for DenyReason {
    type E = Vec<String>;
    fn get_rpc_error_code(&self) -> i32 {
        match self {
            Self::Unavailable => CAPABILITY_NOT_AVAILABLE,
            Self::Unsupported => CAPABILITY_NOT_SUPPORTED,
            Self::GrantDenied => CAPABILITY_NOT_PERMITTED,
            Self::Unpermitted => CAPABILITY_NOT_PERMITTED,
            Self::Ungranted => CAPABILITY_NOT_PERMITTED,
            Self::NotFound => JSON_RPC_STANDARD_ERROR_METHOD_NOT_FOUND,
            Self::AppNotInActiveState => CAPABILITY_NOT_PERMITTED,
            Self::GrantProviderMissing => CAPABILITY_GRANT_PROVIDER_MISSING,
            _ => CAPABILITY_GET_ERROR,
        }
    }

    fn get_rpc_error_message(&self, caps: Vec<String>) -> String {
        let caps_disp = caps.join(",");
        match self {
            Self::Unavailable => format!("{} is not available", caps_disp),
            Self::Unsupported => format!("{} is not supported", caps_disp),
            Self::GrantDenied => format!("The user denied access to {}", caps_disp),
            Self::Unpermitted => format!("{} is not permitted", caps_disp),
            Self::Ungranted => format!("The user did not make a grant decision for {}", caps_disp),
            Self::NotFound => "Method not Found".to_string(),
            Self::AppNotInActiveState => {
                "Capability cannot be used when app is not in foreground state due to requiring a user grant".to_string()
            }
            Self::GrantProviderMissing => format!("Grant provider is missing for {}", caps_disp),
            _ => format!("Error with {}", caps_disp),
        }
    }

    fn get_observability_error_code(&self) -> i32 {
        match self {
            Self::Unavailable => CAPABILITY_NOT_AVAILABLE,
            Self::Unsupported => CAPABILITY_NOT_SUPPORTED,
            Self::GrantDenied => CAPABILITY_GRANT_DENIED,
            Self::Unpermitted => CAPABILITY_NOT_PERMITTED,
            Self::Ungranted => CAPABILITY_UNGRANTED,
            Self::NotFound => JSON_RPC_STANDARD_ERROR_METHOD_NOT_FOUND,
            Self::AppNotInActiveState => CAPABILITY_APP_NOT_IN_ACTIVE_STATE,
            Self::GrantProviderMissing => CAPABILITY_GRANT_PROVIDER_MISSING,
            _ => CAPABILITY_GET_ERROR,
        }
    }
}

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct RoleInfo {
    pub role: Option<CapabilityRole>,
    pub capability: FireboltCap,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CapInfoRpcRequest {
    pub capabilities: Vec<FireboltCap>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CapRPCRequest {
    pub capability: FireboltCap,
    pub options: Option<CapabilityOption>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CapabilityOption {
    pub role: CapabilityRole,
}

impl Default for CapabilityOption {
    fn default() -> Self {
        Self {
            role: CapabilityRole::Use,
        }
    }
}

impl From<CapRPCRequest> for RoleInfo {
    fn from(value: CapRPCRequest) -> Self {
        RoleInfo {
            role: Some(value.options.unwrap_or_default().role),
            capability: value.capability,
        }
    }
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
        let variant_name = match self {
            CapEvent::OnAvailable => "onAvailable",
            CapEvent::OnUnavailable => "onUnavailable",
            CapEvent::OnGranted => "onGranted",
            CapEvent::OnRevoked => "onRevoked",
        };
        variant_name.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_firebolt_cap_short() {
        let cap = FireboltCap::short("account:session");
        assert_eq!(cap.as_str(), "xrn:firebolt:capability:account:session");
    }

    #[test]
    fn test_firebolt_cap_full() {
        let cap = FireboltCap::parse_long("xrn:firebolt:capability:account:session".to_string());
        assert_eq!(cap, Some(FireboltCap::Short("account:session".to_string())));
    }

    #[test]
    fn test_firebolt_cap_parse() {
        let cap = FireboltCap::parse("xrn:firebolt:capability:account:session".to_string());
        assert_eq!(cap, Some(FireboltCap::Short("account:session".to_string())));
    }

    #[test]
    fn test_firebolt_cap_parse_long() {
        let cap = FireboltCap::parse_long("xrn:firebolt:capability:account:session".to_string());
        assert_eq!(cap, Some(FireboltCap::Short("account:session".to_string())));
    }

    #[test]
    fn test_firebolt_cap_from_vec_string() {
        let cap_list = vec!["account:session".to_string()];
        let cap = FireboltCap::from_vec_string(cap_list);
        assert_eq!(cap, vec![FireboltCap::Full("account:session".to_string())]);
    }

    #[test]
    fn test_capability_role_as_string() {
        let role = CapabilityRole::Use;
        assert_eq!(role.as_string(), "use");
    }

    #[test]
    fn test_firebolt_permission_from_role_info() {
        let role = RoleInfo {
            role: Some(CapabilityRole::Use),
            capability: FireboltCap::short("account:session"),
        };
        let perm = FireboltPermission::from(role);
        assert_eq!(
            perm,
            FireboltPermission::from(FireboltCap::short("account:session"))
        );
    }

    #[test]
    fn test_firebolt_permission_from_firebolt_cap() {
        let cap = FireboltCap::short("account:session");
        let perm = FireboltPermission::from(cap);
        assert_eq!(
            perm,
            FireboltPermission::from(FireboltCap::short("account:session"))
        );
    }

    #[test]
    fn test_firebolt_permission_from_cap_request_rpc_request() {
        let cap_req = CapRequestRpcRequest {
            grants: vec![RoleInfo {
                role: Some(CapabilityRole::Use),
                capability: FireboltCap::short("account:session"),
            }],
        };
        let perm = Vec::<FireboltPermission>::from(cap_req);
        assert_eq!(
            perm,
            vec![FireboltPermission::from(FireboltCap::short(
                "account:session"
            ))]
        );
    }

    #[test]
    fn test_firebolt_permission_from_capability_set() {
        let cap_set = CapabilitySet {
            use_caps: Some(vec![FireboltCap::short("account:session")]),
            manage_caps: None,
            provide_cap: None,
        };
        let perm = Vec::<FireboltPermission>::from(cap_set);
        assert_eq!(
            perm,
            vec![FireboltPermission::from(FireboltCap::short(
                "account:session"
            ))]
        );
    }

    #[test]
    fn test_firebolt_permission_serialize() {
        let perm = FireboltPermission {
            cap: FireboltCap::short("account:session"),
            role: CapabilityRole::Use,
        };
        let serialized = serde_json::to_string(&perm).unwrap();
        assert_eq!(serialized, "\"xrn:firebolt:capability:account:session\"");
    }

    #[test]
    fn test_firebolt_permission_deserialize() {
        let perm = "\"xrn:firebolt:capability:account:session\"";
        let deserialized: FireboltPermission = serde_json::from_str(perm).unwrap();
        assert_eq!(
            deserialized,
            FireboltPermission {
                cap: FireboltCap::short("account:session"),
                role: CapabilityRole::Use
            }
        );
    }

    #[test]
    fn test_deny_reason_get_rpc_error_message() {
        let caps = vec!["xrn:firebolt:capability:account:session".to_string()];
        assert_eq!(
            DenyReason::Unavailable.get_rpc_error_message(caps.clone()),
            "xrn:firebolt:capability:account:session is not available"
        );
        assert_eq!(
            DenyReason::Unsupported.get_rpc_error_message(caps.clone()),
            "xrn:firebolt:capability:account:session is not supported"
        );
        assert_eq!(
            DenyReason::GrantDenied.get_rpc_error_message(caps.clone()),
            "The user denied access to xrn:firebolt:capability:account:session"
        );
        assert_eq!(
            DenyReason::Unpermitted.get_rpc_error_message(caps.clone()),
            "xrn:firebolt:capability:account:session is not permitted"
        );
        assert_eq!(
            DenyReason::Ungranted.get_rpc_error_message(caps.clone()),
            "The user did not make a grant decision for xrn:firebolt:capability:account:session"
        );
        assert_eq!(
            DenyReason::NotFound.get_rpc_error_message(caps.clone()),
            "Method not Found"
        );
        assert_eq!(
            DenyReason::AppNotInActiveState.get_rpc_error_message(caps.clone()),
            "Capability cannot be used when app is not in foreground state due to requiring a user grant"
        );
        assert_eq!(
            DenyReason::GrantProviderMissing.get_rpc_error_message(caps),
            "Grant provider is missing for xrn:firebolt:capability:account:session"
        );
    }
    #[test]
    fn test_deny_reason_get_rpc_error_code() {
        assert_eq!(
            DenyReason::Unavailable.get_rpc_error_code(),
            CAPABILITY_NOT_AVAILABLE
        );
        assert_eq!(
            DenyReason::Unsupported.get_rpc_error_code(),
            CAPABILITY_NOT_SUPPORTED
        );
        assert_eq!(
            DenyReason::GrantDenied.get_rpc_error_code(),
            CAPABILITY_NOT_PERMITTED
        );
        assert_eq!(
            DenyReason::Unpermitted.get_rpc_error_code(),
            CAPABILITY_NOT_PERMITTED
        );
        assert_eq!(
            DenyReason::Ungranted.get_rpc_error_code(),
            CAPABILITY_NOT_PERMITTED
        );
        assert_eq!(
            DenyReason::NotFound.get_rpc_error_code(),
            JSON_RPC_STANDARD_ERROR_METHOD_NOT_FOUND
        );
        assert_eq!(
            DenyReason::AppNotInActiveState.get_rpc_error_code(),
            CAPABILITY_NOT_PERMITTED
        );
        assert_eq!(
            DenyReason::GrantProviderMissing.get_rpc_error_code(),
            CAPABILITY_GRANT_PROVIDER_MISSING
        );
    }
    #[test]
    fn test_deny_reason_get_observability_error_code() {
        assert_eq!(
            DenyReason::Unavailable.get_observability_error_code(),
            CAPABILITY_NOT_AVAILABLE
        );
        assert_eq!(
            DenyReason::Unsupported.get_observability_error_code(),
            CAPABILITY_NOT_SUPPORTED
        );
        assert_eq!(
            DenyReason::GrantDenied.get_observability_error_code(),
            CAPABILITY_GRANT_DENIED
        );
        assert_eq!(
            DenyReason::Unpermitted.get_observability_error_code(),
            CAPABILITY_NOT_PERMITTED
        );
        assert_eq!(
            DenyReason::Ungranted.get_observability_error_code(),
            CAPABILITY_UNGRANTED
        );
        assert_eq!(
            DenyReason::NotFound.get_observability_error_code(),
            JSON_RPC_STANDARD_ERROR_METHOD_NOT_FOUND
        );
        assert_eq!(
            DenyReason::AppNotInActiveState.get_observability_error_code(),
            CAPABILITY_APP_NOT_IN_ACTIVE_STATE
        );
        assert_eq!(
            DenyReason::GrantProviderMissing.get_observability_error_code(),
            CAPABILITY_GRANT_PROVIDER_MISSING
        );
    }
}
