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

use std::collections::{HashMap, HashSet};

use log::{debug, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::fb_capabilities::{
    CapRequestRpcRequest, CapabilityRole, DenyReason, DenyReasonWithCap, FireboltCap,
    FireboltPermission,
};

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct FireboltSemanticVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub readable: String,
}

impl FireboltSemanticVersion {
    pub fn new(major: u32, minor: u32, patch: u32, str: String) -> FireboltSemanticVersion {
        FireboltSemanticVersion {
            major,
            minor,
            patch,
            readable: str,
        }
    }
}

impl Default for FireboltSemanticVersion {
    fn default() -> Self {
        Self {
            major: 0,
            minor: 0,
            patch: 0,
            readable: String::from("DEFAULT_VALUE"),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct FireboltInfo {
    pub title: String,
    pub version: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct FireboltVersionManifest {
    #[allow(unused)]
    pub capabilities: HashMap<String, CapabilityPolicy>,
    apis: HashMap<String, OpenRPCParser>,
}

impl FireboltVersionManifest {
    pub fn get_latest_rpc(&self) -> Option<OpenRPCParser> {
        if self.apis.is_empty() {
            return None;
        }
        let mut max_api_version = self.apis.keys().next().unwrap().clone();
        for k in self.apis.keys() {
            if k.cmp(&max_api_version).is_gt() {
                max_api_version = k.clone();
            }
        }
        self.apis.get(&max_api_version).cloned()
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct CapabilityPolicy {
    pub level: CapabilitySupportLevel,
    #[serde(rename = "use")]
    pub use_role: Option<PermissionPolicy>,
    pub manage: Option<PermissionPolicy>,
    pub provide: Option<PermissionPolicy>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum CapabilitySupportLevel {
    Must,
    Should,
    Could,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PermissionPolicy {
    pub public: bool,
    pub negotiable: bool,
}

#[derive(Deserialize, Debug, Clone)]
pub struct OpenRPCParser {
    pub openrpc: String,
    pub info: FireboltInfo,
    pub methods: Vec<FireboltOpenRpcMethod>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FireboltOpenRpc {
    pub openrpc: String,
    pub info: FireboltSemanticVersion,
    pub methods: Vec<FireboltOpenRpcMethod>,
}

impl From<OpenRPCParser> for FireboltOpenRpc {
    fn from(value: OpenRPCParser) -> Self {
        let version = value.info.version.split('.');
        let version_vec: Vec<&str> = version.collect();
        let patch: String = version_vec[2]
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect();
        FireboltOpenRpc {
            openrpc: value.openrpc,
            info: FireboltSemanticVersion::new(
                version_vec[0].parse::<u32>().unwrap(),
                version_vec[1].parse::<u32>().unwrap(),
                patch.parse::<u32>().unwrap(),
                "".to_string(),
            ),
            methods: value.methods,
        }
    }
}

impl Default for FireboltOpenRpc {
    fn default() -> Self {
        Self {
            openrpc: "0.0.0".to_string(),
            info: FireboltSemanticVersion {
                major: 0,
                minor: 0,
                patch: 0,
                readable: String::from("Firebolt API v0.0.0"),
            },
            methods: Vec::new(),
        }
    }
}

impl From<FireboltVersionManifest> for FireboltOpenRpc {
    fn from(version_manifest: FireboltVersionManifest) -> Self {
        // TODO only use the latest rpc version, in future we should support multiple versions
        // If the spec file has no rpc versions, this will panic. But we cannot start ripple without an rpc version
        let parser = version_manifest.get_latest_rpc().unwrap();

        // Parse the version into a FireboltSemanticVersion
        let version = parser.info.version.split('.');
        let version_vec: Vec<&str> = version.collect();
        let patch: String = version_vec[2]
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect();
        let mut api = FireboltSemanticVersion::new(
            version_vec[0].parse::<u32>().unwrap(),
            version_vec[1].parse::<u32>().unwrap(),
            patch.parse::<u32>().unwrap(),
            "".to_string(),
        );
        api.readable = format!("Firebolt API v{}.{}.{}", api.major, api.minor, api.patch);

        FireboltOpenRpc {
            methods: parser.methods,
            openrpc: parser.openrpc,
            info: api,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub enum CapType {
    Available,
    Supported,
}

pub struct Cap {
    pub urn: String,
    pub cap_type: CapType,
}

impl Cap {
    pub fn from_str(s: String, support_cap_roster: Vec<String>) -> Cap {
        Cap {
            urn: s.clone(),
            cap_type: if support_cap_roster.contains(&s) {
                CapType::Supported
            } else {
                CapType::Available
            },
        }
    }
}

impl FireboltOpenRpc {
    pub fn get_methods_caps(&self) -> HashMap<String, CapabilitySet> {
        let mut r = HashMap::default();
        for method in &self.methods {
            let method_name = FireboltOpenRpcMethod::name_with_lowercase_module(&method.name);
            let method_tags = &method.tags;
            if let Some(tags) = method_tags {
                for tag in tags {
                    if tag.name == "capabilities" {
                        r.insert(
                            method_name.clone(),
                            CapabilitySet {
                                use_caps: tag.get_uses_caps(),
                                provide_cap: tag.get_provides(),
                                manage_caps: tag.get_manages_caps(),
                            },
                        );
                    }
                }
            }
        }
        r
    }

    pub fn get_setter_method_for_getter(
        &self,
        getter_method: &str,
    ) -> Option<FireboltOpenRpcMethod> {
        let tokens: Vec<&str> = getter_method.split_terminator('.').collect();

        if tokens.len() != 2 {
            return None;
        }

        let setter = self.get_setter_method_for_property(tokens[1]);
        setter.filter(|method| {
            if let Some(suffix) = method
                .name
                .to_lowercase()
                .strip_prefix(tokens[0].to_lowercase().as_str())
            {
                !suffix.is_empty() && suffix.starts_with('.')
            } else {
                false
            }
        })
    }

    pub fn get_setter_method_for_property(&self, property: &str) -> Option<FireboltOpenRpcMethod> {
        self.methods
            .iter()
            .find(|method| {
                method.tags.as_ref().map_or(false, |tags| {
                    tags.iter().any(|tag| {
                        (tag.name == "rpc-only" || tag.name == "setter")
                            && tag.setter_for.as_deref() == Some(property)
                    })
                })
            })
            .cloned()
    }

    /// Ripple Developers can use this method to create an extension open rpc based on Firebolt Schema
    /// and pass this to the main application for capability resolution
    pub fn load_additional_methods(rpc: &mut FireboltOpenRpc, file_contents: &'static str) {
        let addl_rpc = serde_json::from_str::<OpenRPCParser>(file_contents);
        if addl_rpc.is_err() {
            warn!("Could not read additional RPC file");
            return;
        }

        for m in addl_rpc.unwrap().methods {
            rpc.methods.push(m.clone());
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct FireboltOpenRpcTagDeprecated {
    pub name: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct FireboltOpenRpcCapabilities {
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FireboltOpenRpcTag {
    pub name: String,
    #[serde(rename = "x-uses")]
    pub uses: Option<Vec<String>>,
    #[serde(rename = "x-manages")]
    pub manages: Option<Vec<String>>,
    #[serde(rename = "x-provides")]
    pub provides: Option<String>,
    #[serde(rename = "x-provided-by")]
    pub provided_by: Option<String>,
    #[serde(rename = "x-alternative")]
    pub alternative: Option<String>,
    #[serde(rename = "x-since")]
    pub since: Option<String>,
    #[serde(rename = "x-allow-value")]
    pub allow_value: Option<bool>,
    #[serde(rename = "x-setter-for")]
    pub setter_for: Option<String>,
    #[serde(rename = "x-response")]
    pub response: Option<Value>,
    #[serde(rename = "x-response-for")]
    pub response_for: Option<String>,
    #[serde(rename = "x-error")]
    pub error: Option<Value>,
    #[serde(rename = "x-error-for")]
    pub error_for: Option<String>,
    #[serde(rename = "x-allow-focus")]
    pub allow_focus: Option<bool>,
    #[serde(rename = "x-allow-focus-for")]
    pub allow_focus_for: Option<String>,
}

impl FireboltOpenRpcTag {
    fn get_uses_caps(&self) -> Option<Vec<FireboltCap>> {
        if let Some(caps) = self.uses.clone() {
            return Some(caps.iter().map(|x| FireboltCap::Full(x.clone())).collect());
        }
        None
    }

    fn get_manages_caps(&self) -> Option<Vec<FireboltCap>> {
        if let Some(caps) = self.manages.clone() {
            return Some(caps.iter().map(|x| FireboltCap::Full(x.clone())).collect());
        }
        None
    }

    pub fn get_provides(&self) -> Option<FireboltCap> {
        if let Some(caps) = self.provides.clone() {
            return Some(FireboltCap::Full(caps));
        }
        None
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FireboltOpenRpcMethod {
    pub name: String,
    pub tags: Option<Vec<FireboltOpenRpcTag>>,
}

impl FireboltOpenRpcMethod {
    pub fn get_allow_value(&self) -> Option<bool> {
        self.tags.as_ref()?;
        let allow_tag_opt = self
            .tags
            .as_ref()
            .unwrap()
            .iter()
            .find(|x| x.name == "property" && x.allow_value.is_some());

        allow_tag_opt.map(|openrpc_tag| openrpc_tag.allow_value.unwrap())
    }

    pub fn name_with_lowercase_module(method: &str) -> String {
        // Check if delimter ('.') is present in method name.
        // If found, idx will be the index of the delimiter
        if let Some((idx, _)) = method.chars().enumerate().find(|&(_, c)| c == '.') {
            // Split method to module and method name at the delimiter
            let (module, method_name) = method.split_at(idx);
            // convert module to lowercase and concatenate with method name
            format!("{}.{}", module.to_lowercase(), &method_name[1..])
        } else {
            // No delimiter found, return method as is
            method.to_string()
        }
    }

    pub fn is_named(&self, method_name: &str) -> bool {
        FireboltOpenRpcMethod::name_with_lowercase_module(&self.name)
            == FireboltOpenRpcMethod::name_with_lowercase_module(method_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CapabilitySet {
    pub use_caps: Option<Vec<FireboltCap>>,
    pub provide_cap: Option<FireboltCap>,
    pub manage_caps: Option<Vec<FireboltCap>>,
}

impl From<CapRequestRpcRequest> for CapabilitySet {
    fn from(c: CapRequestRpcRequest) -> Self {
        let mut use_caps_vec = Vec::new();
        let mut provide_cap = None;
        let mut manage_caps_vec = Vec::new();

        for cap in c.grants {
            let capability = cap.clone().capability;
            if let Some(role) = cap.clone().role {
                match role {
                    CapabilityRole::Use => use_caps_vec.push(capability),
                    CapabilityRole::Provide => {
                        let _ = provide_cap.insert(capability);
                    }
                    CapabilityRole::Manage => manage_caps_vec.push(capability),
                }
            }
        }

        let use_caps = if !use_caps_vec.is_empty() {
            Some(use_caps_vec)
        } else {
            None
        };

        let manage_caps = if !manage_caps_vec.is_empty() {
            Some(manage_caps_vec)
        } else {
            None
        };

        CapabilitySet {
            use_caps,
            provide_cap,
            manage_caps,
        }
    }
}

impl From<Vec<FireboltPermission>> for CapabilitySet {
    fn from(permissions: Vec<FireboltPermission>) -> Self {
        let mut use_caps = Vec::new();
        let mut provide_caps = None;
        let mut manage_caps = Vec::new();
        for permission in permissions {
            match permission.role {
                CapabilityRole::Use => use_caps.push(permission.cap.clone()),
                CapabilityRole::Manage => manage_caps.push(permission.cap.clone()),
                CapabilityRole::Provide => {
                    let _ = provide_caps.insert(permission.cap.clone());
                }
            }
        }
        let use_caps = if use_caps.is_empty() {
            None
        } else {
            Some(use_caps)
        };

        let manage_caps = if manage_caps.is_empty() {
            None
        } else {
            Some(manage_caps)
        };
        CapabilitySet {
            use_caps,
            provide_cap: provide_caps,
            manage_caps,
        }
    }
}

impl CapabilitySet {
    pub fn into_firebolt_permissions_vec(&self) -> Vec<FireboltPermission> {
        let mut permission_vec = Vec::new();
        if let Some(use_caps) = self.use_caps.as_ref() {
            for caps in use_caps {
                permission_vec.push(FireboltPermission {
                    cap: caps.clone(),
                    role: CapabilityRole::Use,
                });
            }
        }
        if let Some(manage_caps) = self.manage_caps.as_ref() {
            for caps in manage_caps {
                permission_vec.push(FireboltPermission {
                    cap: caps.clone(),
                    role: CapabilityRole::Manage,
                });
            }
        }
        if let Some(provide_caps) = self.provide_cap.as_ref() {
            permission_vec.push(FireboltPermission {
                cap: provide_caps.clone(),
                role: CapabilityRole::Provide,
            });
        }
        permission_vec
    }

    /// TODO: Make this more role driven in future
    pub fn get_caps(&self) -> Vec<FireboltCap> {
        let mut caps = HashSet::new();
        if let Some(c) = self.use_caps.clone() {
            c.into_iter().for_each(|x| {
                caps.insert(x);
            });
        }
        if let Some(c) = self.provide_cap.clone() {
            caps.insert(c);
        }
        if let Some(c) = self.manage_caps.clone() {
            c.into_iter().for_each(|x| {
                caps.insert(x);
            });
        }

        caps.into_iter().collect()
    }

    fn get_first_perm_from_vec(
        vec: Option<Vec<FireboltCap>>,
        role: CapabilityRole,
    ) -> Option<FireboltPermission> {
        if let Some(c) = vec {
            if let Some(cap) = c.first() {
                return Some(FireboltPermission {
                    cap: cap.clone(),
                    role,
                });
            }
        }

        None
    }

    ///
    /// Gets the first role and capability that is in this request
    /// Should probably try and support multiple capabilities and requests that may
    /// require multiple roles in the future
    pub fn get_first_permission(self) -> Option<FireboltPermission> {
        if let Some(p) = CapabilitySet::get_first_perm_from_vec(self.use_caps, CapabilityRole::Use)
        {
            return Some(p);
        }
        if let Some(p) =
            CapabilitySet::get_first_perm_from_vec(self.manage_caps, CapabilityRole::Manage)
        {
            return Some(p);
        }
        if let Some(c) = self.provide_cap {
            return Some(FireboltPermission {
                cap: c,
                role: CapabilityRole::Provide,
            });
        }

        None
    }

    pub fn has_permissions(
        &self,
        permissions: &[FireboltPermission],
    ) -> Result<(), DenyReasonWithCap> {
        let mut result = true;
        let mut caps_not_permitted: Vec<FireboltCap> = Default::default();
        if let Some(use_caps) = &self.use_caps {
            use_caps.iter().for_each(|cap| {
                let perm = FireboltPermission {
                    cap: cap.to_owned(),
                    role: CapabilityRole::Use,
                };
                if !permissions.contains(&perm) {
                    result = false;
                    debug!("use caps not present: {:?}", perm);
                    caps_not_permitted.push(cap.to_owned());
                }
            });
        }
        if result && self.provide_cap.is_some() {
            let provided = self.provide_cap.as_ref().unwrap();
            let perm = FireboltPermission {
                cap: provided.to_owned(),
                role: CapabilityRole::Provide,
            };
            result = permissions.contains(&perm);
            if !result {
                debug!("provide caps not present: {:?}", perm);
                caps_not_permitted.push(provided.to_owned());
            }
        }
        if result && self.manage_caps.is_some() {
            let manage_caps = self.manage_caps.as_ref().unwrap();
            manage_caps.iter().for_each(|cap| {
                let perm = FireboltPermission {
                    cap: cap.to_owned(),
                    role: CapabilityRole::Use,
                };
                if !permissions.contains(&perm) {
                    result = false;
                    debug!("manage caps not present: {:?}", perm);
                    caps_not_permitted.push(cap.to_owned());
                }
            });
        }
        if result {
            Ok(())
        } else {
            Err(DenyReasonWithCap::new(
                DenyReason::Unpermitted,
                caps_not_permitted,
            ))
        }
    }

    pub fn check(&self, cap_set: CapabilitySet) -> Result<(), DenyReasonWithCap> {
        let mut caps_not_permitted = Vec::new();
        if let Some(use_caps) = cap_set.use_caps {
            if let Some(self_use_caps) = self.use_caps.clone() {
                for cap in use_caps {
                    if !self_use_caps.contains(&cap) {
                        caps_not_permitted.push(cap.clone());
                    }
                }
            } else {
                caps_not_permitted.extend(use_caps);
            }
        }

        if let Some(manage_caps) = cap_set.manage_caps {
            if let Some(self_manage_caps) = self.manage_caps.clone() {
                for cap in manage_caps {
                    if !self_manage_caps.contains(&cap) {
                        caps_not_permitted.push(cap.clone());
                    }
                }
            } else {
                caps_not_permitted.extend(manage_caps);
            }
        }

        if let Some(provide_cap) = cap_set.provide_cap {
            if let Some(self_provide_cap) = self.provide_cap.clone() {
                if !self_provide_cap.eq(&provide_cap) {
                    caps_not_permitted.push(provide_cap)
                }
            }
        }

        if !caps_not_permitted.is_empty() {
            return Err(DenyReasonWithCap::new(
                DenyReason::Unpermitted,
                caps_not_permitted,
            ));
        }

        Ok(())
    }

    pub fn get_from_role(caps: Vec<FireboltCap>, role: Option<CapabilityRole>) -> CapabilitySet {
        if let Some(role) = role {
            match role {
                CapabilityRole::Use => CapabilitySet {
                    use_caps: Some(caps),
                    provide_cap: None,
                    manage_caps: None,
                },
                CapabilityRole::Manage => CapabilitySet {
                    use_caps: None,
                    provide_cap: Some(caps.first().cloned().unwrap()),
                    manage_caps: None,
                },
                CapabilityRole::Provide => CapabilitySet {
                    use_caps: None,
                    provide_cap: None,
                    manage_caps: Some(caps),
                },
            }
        } else {
            CapabilitySet {
                use_caps: Some(caps),
                provide_cap: None,
                manage_caps: None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::api::firebolt::{
        fb_capabilities::{
            CapRequestRpcRequest, CapabilityRole, FireboltCap, FireboltPermission, RoleInfo,
        },
        fb_openrpc::{
            Cap, CapType, CapabilitySet, FireboltInfo, FireboltOpenRpcMethod, FireboltOpenRpcTag,
            FireboltVersionManifest, OpenRPCParser,
        },
    };

    #[test]
    fn test_get_latest_rpc_empty() {
        let manifest = FireboltVersionManifest {
            capabilities: HashMap::new(),
            apis: HashMap::new(),
        };
        assert!(manifest.get_latest_rpc().is_none());
    }

    #[test]
    fn test_get_latest_rpc_single() {
        let mut apis = HashMap::new();
        let parser = OpenRPCParser {
            openrpc: "1.0.0".to_string(),
            info: FireboltInfo {
                title: "Firebolt".to_string(),
                version: "1.0.0".to_string(),
            },
            methods: Vec::new(),
        };
        apis.insert("v1".to_string(), parser);

        let manifest = FireboltVersionManifest {
            capabilities: HashMap::new(),
            apis,
        };

        let m = manifest.get_latest_rpc().unwrap();
        assert_eq!(m.openrpc, "1.0.0");
        assert_eq!(m.info.version, "1.0.0");
    }

    #[test]
    fn test_get_latest_rpc_multiple() {
        let mut apis = HashMap::new();
        let parser_v1 = OpenRPCParser {
            openrpc: "1.0.0".to_string(),
            info: FireboltInfo {
                title: "Firebolt".to_string(),
                version: "1.0.0".to_string(),
            },
            methods: Vec::new(),
        };
        let parser_v2 = OpenRPCParser {
            openrpc: "1.1.0".to_string(),
            info: FireboltInfo {
                title: "Firebolt".to_string(),
                version: "1.1.0".to_string(),
            },
            methods: Vec::new(),
        };
        apis.insert("v1".to_string(), parser_v1);
        apis.insert("v2".to_string(), parser_v2);

        let manifest = FireboltVersionManifest {
            capabilities: HashMap::new(),
            apis,
        };

        let m = manifest.get_latest_rpc().unwrap();
        assert_eq!(m.openrpc, "1.1.0");
        assert_eq!(m.info.version, "1.1.0");
    }

    #[test]
    fn test_cap_from_str() {
        let support_cap_roster = vec!["cap1".to_string(), "cap2".to_string()];
        let cap = Cap::from_str("cap1".to_string(), support_cap_roster.clone());
        assert_eq!(cap.cap_type, CapType::Supported);

        let cap = Cap::from_str("cap".to_string(), support_cap_roster);
        assert_eq!(cap.cap_type, CapType::Available);
    }

    #[test]
    fn test_name_with_lowercase_module() {
        assert_eq!(
            FireboltOpenRpcMethod::name_with_lowercase_module("example.method"),
            "example.method"
        );

        assert_eq!(
            FireboltOpenRpcMethod::name_with_lowercase_module("example"),
            "example"
        );
    }

    #[test]
    fn test_firebolt_open_rpc_tag_impl() {
        let tag = FireboltOpenRpcTag {
            name: "property".to_string(),
            allow_value: Some(true),
            uses: Some(vec!["cap1".to_string(), "cap2".to_string()]),
            manages: Some(vec!["cap3".to_string(), "cap4".to_string()]),
            provides: Some("cap5".to_string()),
            alternative: Some("cap6".to_string()),
            since: Some("1.0.0".to_string()),
            setter_for: Some("example_property".to_string()),
            response: None,
            response_for: None,
            error: None,
            error_for: None,
            allow_focus: None,
            allow_focus_for: None,
            provided_by: None,
        };

        assert_eq!(
            tag.get_uses_caps(),
            Some(vec![
                FireboltCap::Full("cap1".to_string()),
                FireboltCap::Full("cap2".to_string())
            ])
        );

        assert_eq!(
            tag.get_manages_caps(),
            Some(vec![
                FireboltCap::Full("cap3".to_string()),
                FireboltCap::Full("cap4".to_string())
            ])
        );

        assert_eq!(
            tag.get_provides(),
            Some(FireboltCap::Full("cap5".to_string()))
        );
    }

    #[test]
    fn test_get_allow_value() {
        let mut method = FireboltOpenRpcMethod {
            name: "test_method".to_string(),
            tags: Some(vec![FireboltOpenRpcTag {
                name: "property".to_string(),
                allow_value: Some(true),
                uses: Some(vec!["cap1".to_string(), "cap2".to_string()]),
                manages: Some(vec!["cap3".to_string(), "cap4".to_string()]),
                provides: Some("cap5".to_string()),
                alternative: Some("cap6".to_string()),
                since: Some("1.0.0".to_string()),
                setter_for: Some("example_property".to_string()),
                response: None,
                response_for: None,
                error: None,
                error_for: None,
                allow_focus: None,
                allow_focus_for: None,
                provided_by: None,
            }]),
        };

        assert_eq!(method.get_allow_value(), Some(true));

        method.tags = Some(vec![FireboltOpenRpcTag {
            name: "other_tag".to_string(),
            allow_value: None,
            uses: Some(vec!["cap1".to_string(), "cap2".to_string()]),
            manages: Some(vec!["cap3".to_string(), "cap4".to_string()]),
            provides: Some("cap5".to_string()),
            alternative: Some("cap6".to_string()),
            since: Some("1.0.0".to_string()),
            setter_for: Some("example_property".to_string()),
            response: None,
            response_for: None,
            error: None,
            error_for: None,
            allow_focus: None,
            allow_focus_for: None,
            provided_by: None,
        }]);

        assert_eq!(method.get_allow_value(), None);
    }

    #[test]
    fn test_is_named() {
        let method = FireboltOpenRpcMethod {
            name: "module.method".to_string(),
            tags: None,
        };

        assert!(method.is_named("module.method"));
        assert!(method.is_named("Module.method"));
        assert!(!method.is_named("other_module.method"));
        assert!(!method.is_named("module.other_method"));
    }

    #[test]
    fn test_from_cap_request_rpc_request() {
        let cap_request = CapRequestRpcRequest {
            grants: vec![
                RoleInfo {
                    capability: FireboltCap::Short("use_cap".to_string()),
                    role: Some(CapabilityRole::Use),
                },
                RoleInfo {
                    capability: FireboltCap::Short("provide_cap".to_string()),
                    role: Some(CapabilityRole::Provide),
                },
                RoleInfo {
                    capability: FireboltCap::Short("manage_cap".to_string()),
                    role: Some(CapabilityRole::Manage),
                },
            ],
        };

        let capability_set = CapabilitySet::from(cap_request);

        assert_eq!(
            capability_set.use_caps,
            Some(vec![FireboltCap::Short("use_cap".to_string())])
        );
        assert_eq!(
            capability_set.provide_cap,
            Some(FireboltCap::Short("provide_cap".to_string()))
        );
        assert_eq!(
            capability_set.manage_caps,
            Some(vec![FireboltCap::Short("manage_cap".to_string())])
        );
    }

    #[test]
    fn test_from_firebolt_permission_vec() {
        let permissions = vec![
            FireboltPermission {
                cap: FireboltCap::Short("use_cap".to_string()),
                role: CapabilityRole::Use,
            },
            FireboltPermission {
                cap: FireboltCap::Short("manage_cap".to_string()),
                role: CapabilityRole::Manage,
            },
            FireboltPermission {
                cap: FireboltCap::Short("provide_cap".to_string()),
                role: CapabilityRole::Provide,
            },
        ];

        let capability_set = CapabilitySet::from(permissions);

        assert_eq!(
            capability_set.use_caps,
            Some(vec![FireboltCap::Short("use_cap".to_string())])
        );
        assert_eq!(
            capability_set.provide_cap,
            Some(FireboltCap::Short("provide_cap".to_string()))
        );
        assert_eq!(
            capability_set.manage_caps,
            Some(vec![FireboltCap::Short("manage_cap".to_string())])
        );
    }

    #[test]
    fn test_into_firebolt_permissions_vec() {
        let capability_set = CapabilitySet {
            use_caps: Some(vec![FireboltCap::Short("use_cap".to_string())]),
            provide_cap: Some(FireboltCap::Short("provide_cap".to_string())),
            manage_caps: Some(vec![FireboltCap::Short("manage_cap".to_string())]),
        };

        let permissions = capability_set.into_firebolt_permissions_vec();

        assert_eq!(
            permissions,
            vec![
                FireboltPermission {
                    cap: FireboltCap::Short("use_cap".to_string()),
                    role: CapabilityRole::Use,
                },
                FireboltPermission {
                    cap: FireboltCap::Short("manage_cap".to_string()),
                    role: CapabilityRole::Manage,
                },
                FireboltPermission {
                    cap: FireboltCap::Short("provide_cap".to_string()),
                    role: CapabilityRole::Provide,
                },
            ]
        );
    }

    #[test]
    pub fn test_firebolt_method_casing() {
        let m1 = FireboltOpenRpcMethod {
            name: String::from("SecureStorage.get"),
            tags: None,
        };
        let m2 = FireboltOpenRpcMethod {
            name: String::from("SecureStorage.getItem"),
            tags: None,
        };
        let m3 = FireboltOpenRpcMethod {
            name: String::from("SecureStorage.get.item"),
            tags: None,
        };
        let m4 = FireboltOpenRpcMethod {
            name: String::from("get"),
            tags: None,
        };

        let m5 = FireboltOpenRpcMethod {
            name: String::from("*"),
            tags: None,
        };

        let m6 = FireboltOpenRpcMethod {
            name: String::from("secureStorage.get"),
            tags: None,
        };
        let m7 = FireboltOpenRpcMethod {
            name: String::from("secureStorage.getItem"),
            tags: None,
        };
        let m8 = FireboltOpenRpcMethod {
            name: String::from("secureStorage.get.item"),
            tags: None,
        };

        assert!(m1.is_named("securestorage.get"));
        assert!(m2.is_named("secureStorage.getItem"));
        assert!(!m2.is_named("secureStorage.getitem"));
        assert!(m3.is_named("Securestorage.get.item"));
        assert!(m4.is_named("get"));
        assert!(!m4.is_named("Get"));
        assert!(!m4.is_named("SecureStorage.get"));
        assert!(m5.is_named("*"));
        assert!(m6.is_named("securestorage.get"));
        assert!(!m6.is_named("securestorage.Get"));
        assert!(!m7.is_named("securestorage.getitem"));
        assert!(m7.is_named("securestorage.getItem"));
        assert!(m8.is_named("securestorage.get.item"));
        assert!(!m8.is_named("securestorage.get.Item"));

        assert_eq!(
            FireboltOpenRpcMethod::name_with_lowercase_module(&m1.name),
            "securestorage.get"
        );
        assert_eq!(
            FireboltOpenRpcMethod::name_with_lowercase_module(&m2.name),
            "securestorage.getItem"
        );
        assert_eq!(
            FireboltOpenRpcMethod::name_with_lowercase_module(&m3.name),
            "securestorage.get.item"
        );
        assert_eq!(
            FireboltOpenRpcMethod::name_with_lowercase_module(&m4.name),
            "get"
        );
        assert_eq!(
            FireboltOpenRpcMethod::name_with_lowercase_module(&m5.name),
            "*"
        );
    }
}
