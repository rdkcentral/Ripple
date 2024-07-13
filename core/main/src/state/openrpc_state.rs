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

use ripple_sdk::log::{debug, error};
use ripple_sdk::{api::firebolt::fb_openrpc::CapabilityPolicy, serde_json};
use ripple_sdk::{
    api::{
        firebolt::{
            fb_capabilities::FireboltPermission,
            fb_openrpc::{
                CapabilitySet, FireboltOpenRpc, FireboltOpenRpcMethod, FireboltSemanticVersion,
                FireboltVersionManifest, OpenRPCParser,
            },
            provider::ProviderAttributes,
        },
        manifest::exclusory::{Exclusory, ExclusoryImpl},
    },
    semver::Op,
};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use openrpc_validator::FireboltOpenRpc as FireboltOpenRpcValidator;

#[derive(Debug, Clone)]
pub enum ApiSurface {
    Firebolt,
    Ripple,
}

#[derive(Debug, Clone, Default)]
pub struct ProviderSet {
    pub request: Option<FireboltOpenRpcMethod>,
    pub focus: Option<FireboltOpenRpcMethod>,
    pub response: Option<FireboltOpenRpcMethod>,
    // <pca> 2
    //pub error: Option<FireboltOpenRpcMethod>,
    pub error_for: Option<String>,
    // </pca>
    pub attributes: Option<&'static ProviderAttributes>,
    // <pca>
    pub provides: Option<String>,
    pub provided_by: Option<String>,
    pub uses: Option<Vec<String>>,
    pub event: bool,
    pub provides_to: Option<String>,
    // </pca>
}

impl ProviderSet {
    pub fn new() -> ProviderSet {
        ProviderSet::default()
    }
}

// <pca>
// pub fn build_provider_sets(
//     openrpc_methods: &Vec<FireboltOpenRpcMethod>,
// ) -> HashMap<String, ProviderSet> {
//     let mut provider_sets = HashMap::default();

//     for method in openrpc_methods {
//         let mut has_x_provides = None;

//         // Only build provider sets for AcknowledgeChallenge and PinChallenge methods for now
//         if !method.name.starts_with("AcknowledgeChallenge.")
//             && !method.name.starts_with("PinChallenge.")
//         {
//             continue;
//         }

//         if let Some(tags) = &method.tags {
//             let mut has_event = false;
//             let mut has_caps = false;
//             let mut has_x_allow_focus_for = false;
//             let mut has_x_response_for = false;
//             let mut has_x_error_for = false;

//             for tag in tags {
//                 if tag.name.eq("event") {
//                     has_event = true;
//                 } else if tag.name.eq("capabilities") {
//                     has_caps = true;
//                     has_x_provides = tag.get_provides();
//                     has_x_allow_focus_for |= tag.allow_focus_for.is_some();
//                     has_x_response_for |= tag.response_for.is_some();
//                     has_x_error_for |= tag.error_for.is_some();
//                 }
//             }

//             if let Some(p) = has_x_provides {
//                 let mut provider_set = provider_sets
//                     .get(&p.as_str())
//                     .unwrap_or(&ProviderSet::new())
//                     .clone();

//                 if has_event && has_caps {
//                     provider_set.request = Some(method.clone());
//                 }
//                 if has_x_allow_focus_for {
//                     provider_set.focus = Some(method.clone());
//                 }
//                 if has_x_response_for {
//                     provider_set.response = Some(method.clone());
//                 }
//                 if has_x_error_for {
//                     provider_set.error = Some(method.clone());
//                 }

//                 let module: Vec<&str> = method.name.split('.').collect();
//                 provider_set.attributes = ProviderAttributes::get(module[0]);

//                 provider_sets.insert(p.as_str(), provider_set.to_owned());
//             }
//         }
//     }
//     provider_sets
// }
pub fn build_provider_sets(
    openrpc_methods: &Vec<FireboltOpenRpcMethod>,
) -> HashMap<String, ProviderSet> {
    let mut provider_sets = HashMap::default();

    for method in openrpc_methods {
        let mut has_x_provides = None;

        // Only build provider sets for AcknowledgeChallenge and PinChallenge methods for now
        if !method.name.starts_with("AcknowledgeChallenge.")
            && !method.name.starts_with("PinChallenge.")
            && !method.name.starts_with("Discovery.")
            && !method.name.starts_with("Content.")
        {
            continue;
        }

        if let Some(tags) = &method.tags {
            let mut has_event = false;
            let mut has_caps = false;
            let mut has_x_allow_focus_for = false;
            let mut has_x_response_for = false;
            // <pca> 2
            //let mut has_x_error_for = false;
            let mut x_error_for = None;
            // </pca>
            let mut x_provided_by = None;
            let mut x_provides = None;
            let mut x_uses = None;

            for tag in tags {
                if tag.name.eq("event") {
                    has_event = true;
                } else if tag.name.eq("capabilities") {
                    has_caps = true;
                    has_x_provides = tag.get_provides();
                    has_x_allow_focus_for |= tag.allow_focus_for.is_some();
                    has_x_response_for |= tag.response_for.is_some();
                    // <pca> 2
                    //has_x_error_for |= tag.error_for.is_some();
                    x_error_for = tag.error_for.clone();
                    // </pca>
                    x_provided_by = tag.provided_by.clone();
                    x_provides = tag.provides.clone();
                    x_uses = tag.uses.clone();
                }
            }

            let mut provider_set = provider_sets
                .get(&FireboltOpenRpcMethod::name_with_lowercase_module(
                    &method.name,
                ))
                .unwrap_or(&ProviderSet::new())
                .clone();

            if let Some(_capability) = has_x_provides {
                if has_event && has_caps {
                    provider_set.request = Some(method.clone());
                }
                if has_x_allow_focus_for {
                    provider_set.focus = Some(method.clone());
                }
                if has_x_response_for {
                    provider_set.response = Some(method.clone());
                }
                // <pca> 2
                // if has_x_error_for {
                //     provider_set.error = Some(method.clone());
                // }
                provider_set.error_for = x_error_for;
                // </pca>
                provider_set.provides = x_provides;
            } else {
                // x-provided-by can only be set if x-provides isn't.
                provider_set.provided_by = x_provided_by.clone();
                if let Some(provided_by) = x_provided_by {
                    let mut provided_by_set = provider_sets
                        .get(&provided_by)
                        .unwrap_or(&ProviderSet::new())
                        .clone();

                    provided_by_set.provides_to = Some(method.name.clone());

                    provider_sets.insert(
                        FireboltOpenRpcMethod::name_with_lowercase_module(&provided_by),
                        provided_by_set.to_owned(),
                    );
                }
            }

            provider_set.uses = x_uses;
            provider_set.event = has_event;

            let module: Vec<&str> = method.name.split('.').collect();
            provider_set.attributes = ProviderAttributes::get(module[0]);

            provider_sets.insert(
                FireboltOpenRpcMethod::name_with_lowercase_module(&method.name),
                provider_set.to_owned(),
            );
        }
    }

    provider_sets
}
//  </pca>

#[derive(Debug, Clone)]
pub struct OpenRpcState {
    open_rpc: FireboltOpenRpc,
    exclusory: Option<ExclusoryImpl>,
    firebolt_cap_map: Arc<RwLock<HashMap<String, CapabilitySet>>>,
    ripple_cap_map: Arc<RwLock<HashMap<String, CapabilitySet>>>,
    cap_policies: Arc<RwLock<HashMap<String, CapabilityPolicy>>>,
    extended_rpc: Arc<RwLock<Vec<FireboltOpenRpc>>>,
    provider_map: Arc<RwLock<HashMap<String, ProviderSet>>>,
    openrpc_validator: Arc<RwLock<FireboltOpenRpcValidator>>,
}

impl OpenRpcState {
    fn load_additional_rpc(rpc: &mut FireboltOpenRpc, file_contents: &'static str) {
        match serde_json::from_str::<OpenRPCParser>(file_contents) {
            Ok(addl_rpc) => {
                for m in addl_rpc.methods {
                    rpc.methods.push(m.clone());
                }
            }
            Err(_) => {
                error!("Could not read additional RPC file");
            }
        }
    }

    fn load_firebolt_open_rpc() -> FireboltVersionManifest {
        let mut fb_open_rpc_file = "/etc/ripple/openrpc/firebolt-open-rpc.json".to_string();

        if cfg!(feature = "local_dev") {
            let key = "FIREBOLT_OPEN_RPC";
            let env_var = std::env::var(key);
            if let Ok(path) = env_var {
                fb_open_rpc_file = path;
            };
        }

        let mut content = "".to_string();
        match std::fs::read_to_string(fb_open_rpc_file.clone()) {
            Ok(str) => {
                debug!("loading from {fb_open_rpc_file}");
                content = str
            }
            Err(e) => error!("can't read {fb_open_rpc_file}: {:?}", e),
        };

        let version_manifest: FireboltVersionManifest = match serde_json::from_str(&content) {
            Ok(fvm) => fvm,
            _ => {
                if content.is_empty() {
                    debug!("loading default");
                } else {
                    error!("failed to parse firebolt-open-rpc, loading default");
                };
                serde_json::from_str(std::include_str!("./firebolt-open-rpc.json")).unwrap()
            }
        };

        version_manifest
    }

    pub fn new(exclusory: Option<ExclusoryImpl>) -> OpenRpcState {
        let version_manifest = Self::load_firebolt_open_rpc();

        let firebolt_open_rpc: FireboltOpenRpc = version_manifest.clone().into();
        let ripple_rpc_file = std::include_str!("./ripple-rpc.json");
        let mut ripple_open_rpc: FireboltOpenRpc = FireboltOpenRpc::default();
        Self::load_additional_rpc(&mut ripple_open_rpc, ripple_rpc_file);

        let openrpc_validator: FireboltOpenRpcValidator =
            serde_json::from_str(std::include_str!("./firebolt-open-rpc.json")).unwrap();

        OpenRpcState {
            firebolt_cap_map: Arc::new(RwLock::new(firebolt_open_rpc.get_methods_caps())),
            ripple_cap_map: Arc::new(RwLock::new(ripple_open_rpc.get_methods_caps())),
            exclusory,
            cap_policies: Arc::new(RwLock::new(version_manifest.capabilities)),
            open_rpc: firebolt_open_rpc.clone(),
            extended_rpc: Arc::new(RwLock::new(Vec::new())),
            provider_map: Arc::new(RwLock::new(build_provider_sets(&firebolt_open_rpc.methods))),
            openrpc_validator: Arc::new(RwLock::new(openrpc_validator)),
        }
    }

    pub fn add_open_rpc(&self, open_rpc: FireboltOpenRpc) {
        let cap_map = open_rpc.get_methods_caps();
        self.extend_caps(cap_map);
    }

    pub fn is_app_excluded(&self, app_id: &str) -> bool {
        if let Some(e) = &self.exclusory {
            return e.is_app_all_excluded(app_id);
        }

        false
    }

    pub fn is_excluded(&self, method: String, app_id: String) -> bool {
        if let Some(e) = &self.exclusory {
            if e.is_excluded(app_id, method.clone()) {
                return true;
            }
            if !e.can_resolve(method) {
                return true;
            }
        }
        false
    }

    pub fn get_perms_for_method(
        &self,
        method: &str,
        api_surface: Vec<ApiSurface>,
    ) -> Option<Vec<FireboltPermission>> {
        let mut perm_list: Vec<FireboltPermission>;
        let mut result = None;
        for surface in api_surface {
            let cap_map = match surface {
                ApiSurface::Firebolt => self.firebolt_cap_map.clone(),
                ApiSurface::Ripple => self.ripple_cap_map.clone(),
            };
            let cap_set_opt = { cap_map.read().unwrap().get(method).cloned() };
            if let Some(cap_set) = cap_set_opt {
                perm_list = cap_set.into_firebolt_permissions_vec();
                result = Some(perm_list);
            }
        }
        result
    }

    pub fn get_capability_policy(&self, cap: String) -> Option<CapabilityPolicy> {
        self.cap_policies.read().unwrap().get(&cap).cloned()
    }

    pub fn extend_caps(&self, caps: HashMap<String, CapabilitySet>) {
        let mut cap_map = self.firebolt_cap_map.write().unwrap();
        cap_map.extend(caps);
    }

    pub fn extend_policies(&self, policies: HashMap<String, CapabilityPolicy>) {
        let mut cap_policies = self.cap_policies.write().unwrap();
        cap_policies.extend(policies);
    }

    pub fn check_privacy_property(&self, property: &str) -> bool {
        if let Some(method) = self.open_rpc.methods.iter().find(|x| x.is_named(property)) {
            // Checking if the property tag is havin x-allow-value extension.
            if let Some(tags) = &method.tags {
                if tags
                    .iter()
                    .find(|x| x.name == "property" && x.allow_value.is_some())
                    .map_or(false, |_| true)
                {
                    return true;
                }
            }
        }
        {
            let ext_rpcs = self.extended_rpc.read().unwrap();
            for ext_rpc in ext_rpcs.iter() {
                if let Some(method) = ext_rpc.methods.iter().find(|x| x.is_named(property)) {
                    // Checking if the property tag is havin x-allow-value extension.
                    if let Some(tags) = &method.tags {
                        if tags
                            .iter()
                            .find(|x| x.name == "property" && x.allow_value.is_some())
                            .map_or(false, |_| true)
                        {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    pub fn get_method_with_allow_value_property(
        &self,
        method_name: String,
    ) -> Option<FireboltOpenRpcMethod> {
        if let Some(v) = self
            .open_rpc
            .methods
            .iter()
            .find(|x| {
                x.is_named(&method_name)
                    && x.tags.is_some()
                    && x.tags
                        .as_ref()
                        .unwrap()
                        .iter()
                        .any(|tag| tag.name == "property" && tag.allow_value.is_some())
            })
            .cloned()
        {
            return Some(v);
        }
        {
            let ext_rpcs = self.extended_rpc.read().unwrap();
            for ext_rpc in ext_rpcs.iter() {
                if let Some(v) = ext_rpc
                    .methods
                    .iter()
                    .find(|x| {
                        x.is_named(&method_name)
                            && x.tags.is_some()
                            && x.tags
                                .as_ref()
                                .unwrap()
                                .iter()
                                .any(|tag| tag.name == "property" && tag.allow_value.is_some())
                    })
                    .cloned()
                {
                    return Some(v);
                }
            }
        }

        None
    }

    pub fn get_open_rpc(&self) -> FireboltOpenRpc {
        self.open_rpc.clone()
    }

    pub fn get_provider_map(&self) -> HashMap<String, ProviderSet> {
        self.provider_map.read().unwrap().clone()
    }

    pub fn get_version(&self) -> FireboltSemanticVersion {
        self.open_rpc.info.clone()
    }

    pub fn get_openrpc_validator(&self) -> FireboltOpenRpcValidator {
        self.openrpc_validator.read().unwrap().clone()
    }
}
