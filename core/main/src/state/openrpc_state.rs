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

use ripple_sdk::api::{
    firebolt::{
        fb_capabilities::FireboltPermission,
        fb_openrpc::{
            CapabilitySet, FireboltOpenRpc, FireboltOpenRpcMethod, FireboltSemanticVersion,
            FireboltVersionManifest, OpenRPCParser,
        },
        provider::ProviderAttributes,
    },
    manifest::exclusory::{Exclusory, ExclusoryImpl},
};
use ripple_sdk::log::error;
use ripple_sdk::{api::firebolt::fb_openrpc::CapabilityPolicy, serde_json};
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

// <pca>
#[derive(Debug, Clone, Default)]
// pub struct ProviderSet {
//     pub provides: Option<FireboltCap>,
//     // <pca> Will need for request validation
//     //pub request: Option<Value>,
//     // </pca>
//     pub response: Option<Value>,
//     pub response_for: Option<String>,
//     pub error: Option<Value>,
//     pub error_for: Option<String>,
//     pub allow_focus: Option<bool>,
//     pub focus_for: Option<String>,
//     pub attributes: Option<ProviderAttributes>,
// }

// impl ProviderSet {
//     pub fn new() -> ProviderSet {
//         ProviderSet {
//             provides: None,
//             response: None,
//             response_for: None,
//             error: None,
//             error_for: None,
//             allow_focus: None,
//             focus_for: None,
//             attributes: None,
//         }
//     }
// }
pub struct ProviderSet {
    pub request: Option<FireboltOpenRpcMethod>,
    pub focus: Option<FireboltOpenRpcMethod>,
    pub response: Option<FireboltOpenRpcMethod>,
    pub error: Option<FireboltOpenRpcMethod>,
    pub attributes: Option<&'static ProviderAttributes>,
}

impl ProviderSet {
    pub fn new() -> ProviderSet {
        ProviderSet::default()
    }
}

pub fn build_provider_sets(
    openrpc_methods: &Vec<FireboltOpenRpcMethod>,
) -> HashMap<String, ProviderSet> {
    let mut provider_sets = HashMap::default();

    println!("*** _DEBUG: build_provider_sets: entry");

    for method in openrpc_methods {
        let mut has_x_provides = None;

        // <pca> debug
        // Only build provider sets for AcknowledgeChallenge and PinChallenge methods for now
        if !method.name.starts_with("AcknowledgeChallenge.")
            && !method.name.starts_with("PinChallenge.")
        {
            continue;
        }
        // </pca>

        println!(
            "*** _DEBUG: build_provider_sets: method.name={:?}",
            method.name
        );

        if let Some(tags) = &method.tags {
            let mut has_event = false;
            let mut has_caps = false;
            let mut has_x_allow_focus_for = false;
            let mut has_x_response_for = false;
            let mut has_x_error_for = false;

            for tag in tags {
                println!("*** _DEBUG: build_provider_sets: tag={:?}", tag);
                if tag.name.eq("event") {
                    has_event = true;
                } else if tag.name.eq("capabilities") {
                    has_caps = true;
                    has_x_provides = tag.get_provides();
                    has_x_allow_focus_for |= tag.allow_focus_for.is_some();
                    has_x_response_for |= tag.response_for.is_some();
                    has_x_error_for |= tag.error_for.is_some();
                }
            }

            if let Some(p) = has_x_provides {
                let mut provider_set = provider_sets
                    .get(&p.as_str())
                    .unwrap_or(&ProviderSet::new())
                    .clone();

                if has_event && has_caps {
                    provider_set.request = Some(method.clone());
                }
                if has_x_allow_focus_for {
                    provider_set.focus = Some(method.clone());
                }
                if has_x_response_for {
                    provider_set.response = Some(method.clone());
                }
                if has_x_error_for {
                    provider_set.error = Some(method.clone());
                }

                let module: Vec<&str> = method.name.split('.').collect();
                provider_set.attributes = ProviderAttributes::get(&module[0]);

                println!(
                    "*** _DEBUG: build_provider_sets: provider_set={:?}",
                    provider_set
                );

                provider_sets.insert(p.as_str(), provider_set.to_owned());
            }
        }
    }
    provider_sets
}
// </pca>

#[derive(Debug, Clone)]
pub struct OpenRpcState {
    open_rpc: FireboltOpenRpc,
    exclusory: Option<ExclusoryImpl>,
    firebolt_cap_map: Arc<RwLock<HashMap<String, CapabilitySet>>>,
    ripple_cap_map: Arc<RwLock<HashMap<String, CapabilitySet>>>,
    cap_policies: Arc<RwLock<HashMap<String, CapabilityPolicy>>>,
    extended_rpc: Arc<RwLock<Vec<FireboltOpenRpc>>>,
    // <pca>
    provider_map: Arc<RwLock<HashMap<String, ProviderSet>>>,
    // </pca>
    // <pca> 2
    openrpc_validator: Arc<RwLock<FireboltOpenRpcValidator>>,
    // </pca>
}

impl OpenRpcState {
    pub fn load_additional_rpc(rpc: &mut FireboltOpenRpc, file_contents: &'static str) {
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
    pub fn new(exclusory: Option<ExclusoryImpl>) -> OpenRpcState {
        let version_manifest: FireboltVersionManifest =
            serde_json::from_str(std::include_str!("./firebolt-open-rpc.json")).unwrap();
        let firebolt_open_rpc: FireboltOpenRpc = version_manifest.clone().into();
        let ripple_rpc_file = std::include_str!("./ripple-rpc.json");
        let mut ripple_open_rpc: FireboltOpenRpc = FireboltOpenRpc::default();
        Self::load_additional_rpc(&mut ripple_open_rpc, ripple_rpc_file);

        // <pca> 2
        let openrpc_validator = FireboltOpenRpcValidator::expect_from_file_path(
            "core/main/src/state/firebolt-open-rpc.json",
        );
        // </pca>

        OpenRpcState {
            firebolt_cap_map: Arc::new(RwLock::new(firebolt_open_rpc.get_methods_caps())),
            ripple_cap_map: Arc::new(RwLock::new(ripple_open_rpc.get_methods_caps())),
            exclusory,
            cap_policies: Arc::new(RwLock::new(version_manifest.capabilities)),
            // <pca>
            //open_rpc: firebolt_open_rpc,
            open_rpc: firebolt_open_rpc.clone(),
            // </pca>
            extended_rpc: Arc::new(RwLock::new(Vec::new())),
            // <pca>
            //provider_map: Arc::new(RwLock::new(firebolt_open_rpc.get_methods_provider_set())),
            provider_map: Arc::new(RwLock::new(build_provider_sets(&firebolt_open_rpc.methods))),
            // </pca>
            // <pca> 2
            openrpc_validator: Arc::new(RwLock::new(openrpc_validator)),
            // </pca>
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
        println!("*** _DEBUG: get_perms_for_method: result={:?}", result);
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

    // <pca>
    pub fn get_provider_map(&self) -> HashMap<String, ProviderSet> {
        self.provider_map.read().unwrap().clone()
    }

    pub fn get_version(&self) -> FireboltSemanticVersion {
        self.open_rpc.info.clone()
    }

    pub fn get_openrpc_validator(&self) -> FireboltOpenRpcValidator {
        self.openrpc_validator.read().unwrap().clone()
    }
    // </pca>
}
