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

use super::fb_capabilities::{
    CapRequestRpcRequest, CapabilityRole, DenyReason, DenyReasonWithCap, FireboltCap,
    FireboltPermission,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Deserialize, Debug, Clone)]
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
            if let Some(suffix) = method.name.to_lowercase().strip_prefix(tokens[0]) {
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

#[derive(Deserialize, Debug, Clone)]
pub struct FireboltOpenRpcTag {
    pub name: String,
    #[serde(rename = "x-uses")]
    pub uses: Option<Vec<String>>,
    #[serde(rename = "x-manages")]
    pub manages: Option<Vec<String>>,
    #[serde(rename = "x-provides")]
    pub provides: Option<String>,
    #[serde(rename = "x-alternative")]
    pub alternative: Option<String>,
    #[serde(rename = "x-since")]
    pub since: Option<String>,
    #[serde(rename = "x-allow-value")]
    pub allow_value: Option<bool>,
    #[serde(rename = "x-setter-for")]
    pub setter_for: Option<String>,
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

    fn get_provides(&self) -> Option<FireboltCap> {
        if let Some(caps) = self.provides.clone() {
            return Some(FireboltCap::Full(caps));
        }
        None
    }
}

#[derive(Deserialize, Debug, Clone)]
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
        let mut parts: Vec<&str> = method.split('.').collect();
        if parts.len() < 2 {
            return String::from(method);
        }
        let module = parts.remove(0);
        let method_name = parts.join(".");
        format!("{}.{}", module.to_lowercase(), method_name)
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
                    provide_cap: Some(caps.get(0).cloned().unwrap()),
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
