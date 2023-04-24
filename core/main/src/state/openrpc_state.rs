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
use ripple_sdk::api::{
    firebolt::fb_openrpc::{
        CapabilitySet, FireboltOpenRpc, FireboltOpenRpcMethod, FireboltVersionManifest,
    },
    manifest::exclusory::{Exclusory, ExclusoryImpl},
};
use ripple_sdk::{api::firebolt::fb_openrpc::CapabilityPolicy, serde_json};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

#[derive(Debug, Clone)]
pub struct OpenRpcState {
    open_rpc: FireboltOpenRpc,
    exclusory: Option<ExclusoryImpl>,
    cap_map: Arc<RwLock<HashMap<String, CapabilitySet>>>,
    cap_policies: Arc<RwLock<HashMap<String, CapabilityPolicy>>>,
}

impl OpenRpcState {
    pub fn new(exclusory: Option<ExclusoryImpl>) -> OpenRpcState {
        let version_manifest: FireboltVersionManifest =
            serde_json::from_str(std::include_str!("./firebolt-open-rpc.json")).unwrap();
        let open_rpc: FireboltOpenRpc = version_manifest.clone().into();

        OpenRpcState {
            cap_map: Arc::new(RwLock::new(open_rpc.clone().get_methods_caps())),
            exclusory,
            cap_policies: Arc::new(RwLock::new(version_manifest.capabilities)),
            open_rpc,
        }
    }

    pub fn is_excluded(&self, method: String) -> bool {
        if let Some(e) = &self.exclusory {
            if !e.can_resolve(method.clone()) {
                return true;
            }
        }
        false
    }

    pub fn get_caps_for_method(&self, method: String) -> Option<CapabilitySet> {
        let c = { self.cap_map.read().unwrap().get(&method).cloned() };
        if let Some(caps) = c {
            Some(CapabilitySet {
                use_caps: caps.use_caps.clone(),
                manage_caps: caps.manage_caps.clone(),
                provide_cap: caps.provide_cap.clone(),
            })
        } else {
            None
        }
    }

    pub fn get_capability_policy(&self, cap: String) -> Option<CapabilityPolicy> {
        self.cap_policies.read().unwrap().get(&cap).cloned()
    }

    pub fn extend_caps(&self, caps: HashMap<String, CapabilitySet>) {
        let mut cap_map = self.cap_map.write().unwrap();
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
                return tags
                    .iter()
                    .find(|x| x.name == "property" && x.allow_value.is_some())
                    .map_or(false, |_| true);
            }
        }
        false
    }

    pub fn get_method_with_allow_value_property(
        &self,
        method_name: String,
    ) -> Option<FireboltOpenRpcMethod> {
        self.open_rpc
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
    }

    pub fn get_open_rpc(&self) -> FireboltOpenRpc {
        self.open_rpc.clone()
    }
}
