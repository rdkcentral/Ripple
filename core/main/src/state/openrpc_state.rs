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
            CapabilitySet, FireboltOpenRpc, FireboltOpenRpcMethod, FireboltVersionManifest,
            OpenRPCParser,
        },
    },
    manifest::exclusory::{Exclusory, ExclusoryImpl},
};
use ripple_sdk::log::error;
use ripple_sdk::{api::firebolt::fb_openrpc::CapabilityPolicy, serde_json};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

#[derive(Debug, Clone)]
pub enum ApiSurface {
    Firebolt,
    Ripple,
}

#[derive(Debug, Clone)]
pub struct OpenRpcState {
    open_rpc: FireboltOpenRpc,
    exclusory: Option<ExclusoryImpl>,
    firebolt_cap_map: Arc<RwLock<HashMap<String, CapabilitySet>>>,
    ripple_cap_map: Arc<RwLock<HashMap<String, CapabilitySet>>>,
    cap_policies: Arc<RwLock<HashMap<String, CapabilityPolicy>>>,
    extended_rpc: Arc<RwLock<Vec<FireboltOpenRpc>>>,
}

impl OpenRpcState {
    pub fn load_additional_rpc(rpc: &mut FireboltOpenRpc, file_contents: &'static str) {
        let addl_rpc = serde_json::from_str::<OpenRPCParser>(&file_contents);
        if let Err(_) = addl_rpc {
            error!("Could not read additional RPC file");
            return;
        }

        for m in addl_rpc.unwrap().methods {
            rpc.methods.push(m.clone());
        }
    }
    pub fn new(exclusory: Option<ExclusoryImpl>) -> OpenRpcState {
        let version_manifest: FireboltVersionManifest =
            serde_json::from_str(std::include_str!("./firebolt-open-rpc.json")).unwrap();
        let firebolt_open_rpc: FireboltOpenRpc = version_manifest.clone().into();
        let ripple_rpc_file = std::include_str!("./ripple-rpc.json");
        // let mut ripple_open_rpc: FireboltOpenRpc = version_manifest.clone().into();
        let mut ripple_open_rpc: FireboltOpenRpc = FireboltOpenRpc::default();
        Self::load_additional_rpc(&mut ripple_open_rpc, ripple_rpc_file);

        OpenRpcState {
            firebolt_cap_map: Arc::new(RwLock::new(firebolt_open_rpc.clone().get_methods_caps())),
            ripple_cap_map: Arc::new(RwLock::new(ripple_open_rpc.clone().get_methods_caps())),
            exclusory,
            cap_policies: Arc::new(RwLock::new(version_manifest.capabilities)),
            open_rpc: firebolt_open_rpc,
            extended_rpc: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn add_open_rpc(&self, open_rpc: FireboltOpenRpc) {
        let cap_map = open_rpc.clone().get_methods_caps();
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
        if let Some(method) = self.open_rpc.methods.iter().find(|x| x.name == property) {
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
                if let Some(method) = ext_rpc.methods.iter().find(|x| x.name == property) {
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
                x.name == method_name
                    && x.tags.is_some()
                    && x.tags
                        .as_ref()
                        .unwrap()
                        .iter()
                        .find(|tag| tag.name == "property" && tag.allow_value.is_some())
                        .is_some()
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
                        x.name == method_name
                            && x.tags.is_some()
                            && x.tags
                                .as_ref()
                                .unwrap()
                                .iter()
                                .find(|tag| tag.name == "property" && tag.allow_value.is_some())
                                .is_some()
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
}
