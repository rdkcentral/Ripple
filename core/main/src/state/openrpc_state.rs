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
                FireboltVersionManifest,
            },
            provider::ProviderAttributes,
        },
        manifest::exclusory::{Exclusory, ExclusoryImpl},
    },
    utils::error::RippleError,
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
pub struct ProviderRelationSet {
    pub capability: Option<String>,
    pub attributes: Option<&'static ProviderAttributes>,
    pub event: bool,
    pub provides: Option<String>,
    pub provides_to: Option<String>,
    pub provided_by: Option<String>,
    pub uses: Option<Vec<String>>,
    pub allow_focus_for: Option<String>,
    pub response_for: Option<String>,
    pub error_for: Option<String>,
}

impl ProviderRelationSet {
    pub fn new() -> ProviderRelationSet {
        ProviderRelationSet::default()
    }
}

#[derive(Debug, Clone)]
pub struct OpenRpcState {
    open_rpc: FireboltOpenRpc,
    exclusory: Option<ExclusoryImpl>,
    firebolt_cap_map: Arc<RwLock<HashMap<String, CapabilitySet>>>,
    ripple_cap_map: Arc<RwLock<HashMap<String, CapabilitySet>>>,
    cap_policies: Arc<RwLock<HashMap<String, CapabilityPolicy>>>,
    extended_rpc: Arc<RwLock<Vec<FireboltOpenRpc>>>,
    provider_relation_map: Arc<RwLock<HashMap<String, ProviderRelationSet>>>,
    openrpc_validator: Arc<RwLock<FireboltOpenRpcValidator>>,
    provider_registrations: Vec<String>,
}

impl OpenRpcState {
    fn load_open_rpc(path: &str) -> Option<FireboltOpenRpc> {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                debug!("load_open_rpc: loading from {path}");
                let firebolt_version_manifest: Result<FireboltVersionManifest, _> =
                    serde_json::from_str(&content);
                match firebolt_version_manifest {
                    Ok(fvm) => {
                        return Some(fvm.into());
                    }
                    _ => {
                        error!("load_open_rpc: can't parse {path}");
                    }
                }
            }
            Err(e) => {
                error!("load_open_rpc: can't read {path}, e={:?}", e);
            }
        }

        None
    }

    pub fn add_extension_open_rpc(&self, path: &str) -> Result<(), RippleError> {
        match Self::load_open_rpc(path) {
            Some(open_rpc) => {
                self.build_provider_relation_sets(&open_rpc.methods);
                self.add_open_rpc(open_rpc);
                Ok(())
            }
            None => Err(RippleError::ParseError),
        }
    }

    pub fn new(
        exclusory: Option<ExclusoryImpl>,
        extn_sdks: Vec<String>,
        provider_registrations: Vec<String>,
    ) -> OpenRpcState {
        let open_rpc_path = load_firebolt_open_rpc_path().expect("Need valid open-rpc file");
        let version_manifest: FireboltVersionManifest = serde_json::from_str(&open_rpc_path)
            .expect("Failed parsing FireboltVersionManifest from open RPC file");
        let firebolt_open_rpc: FireboltOpenRpc = version_manifest.clone().into();
        let ripple_open_rpc: FireboltOpenRpc = FireboltOpenRpc::default();
        let openrpc_validator: FireboltOpenRpcValidator = serde_json::from_str(&open_rpc_path)
            .expect("Failed parsing FireboltOpenRpcValidator from open RPC file");

        let v = OpenRpcState {
            firebolt_cap_map: Arc::new(RwLock::new(firebolt_open_rpc.get_methods_caps())),
            ripple_cap_map: Arc::new(RwLock::new(ripple_open_rpc.get_methods_caps())),
            exclusory,
            cap_policies: Arc::new(RwLock::new(version_manifest.capabilities)),
            open_rpc: firebolt_open_rpc.clone(),
            extended_rpc: Arc::new(RwLock::new(Vec::new())),
            provider_relation_map: Arc::new(RwLock::new(HashMap::new())),
            openrpc_validator: Arc::new(RwLock::new(openrpc_validator)),
            provider_registrations,
        };
        v.build_provider_relation_sets(&firebolt_open_rpc.methods);

        for path in extn_sdks {
            if v.add_extension_open_rpc(&path).is_err() {
                error!("Error adding extn_sdk from {path}");
            }
        }

        v
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

    pub fn get_provider_relation_map(&self) -> HashMap<String, ProviderRelationSet> {
        self.provider_relation_map.read().unwrap().clone()
    }

    pub fn set_provider_relation_map(
        &self,
        provider_relation_map: HashMap<String, ProviderRelationSet>,
    ) {
        *self.provider_relation_map.write().unwrap() = provider_relation_map;
    }

    pub fn get_version(&self) -> FireboltSemanticVersion {
        self.open_rpc.info.clone()
    }

    pub fn get_openrpc_validator(&self) -> FireboltOpenRpcValidator {
        self.openrpc_validator.read().unwrap().clone()
    }

    fn is_provider_enabled(&self, method: &str) -> bool {
        let method_lower_case = method.to_lowercase();
        self.provider_registrations
            .iter()
            .any(|x| method_lower_case.starts_with(&x.to_lowercase()))
    }

    pub fn build_provider_relation_sets(&self, openrpc_methods: &Vec<FireboltOpenRpcMethod>) {
        let mut provider_relation_sets: HashMap<String, ProviderRelationSet> = HashMap::default();

        for method in openrpc_methods {
            let mut has_x_provides = None;

            if !self.is_provider_enabled(&method.name) {
                continue;
            } else {
                debug!("Provider enabled {:?}", method.name);
            }

            if let Some(tags) = &method.tags {
                let mut has_event = false;
                let mut x_allow_focus_for = None;
                let mut x_response_for = None;
                let mut x_error_for = None;
                let mut x_provided_by = None;
                let mut x_provides = None;
                let mut x_uses = None;

                for tag in tags {
                    if tag.name.eq("event") {
                        has_event = true;
                    } else if tag.name.eq("capabilities") {
                        has_x_provides = tag.get_provides();
                        x_allow_focus_for = tag.allow_focus_for.clone();
                        x_response_for = tag.response_for.clone();
                        x_error_for = tag.error_for.clone();
                        x_provided_by = tag.provided_by.clone();
                        x_provides = tag.provides.clone();
                        x_uses = tag.uses.clone();
                    }
                }

                let mut provider_relation_set = provider_relation_sets
                    .get(&FireboltOpenRpcMethod::name_with_lowercase_module(
                        &method.name,
                    ))
                    .unwrap_or(&ProviderRelationSet::new())
                    .clone();

                if has_x_provides.is_some() {
                    provider_relation_set.allow_focus_for = x_allow_focus_for;
                    provider_relation_set.response_for = x_response_for;
                    provider_relation_set.error_for = x_error_for;
                    provider_relation_set.capability = x_provides;
                } else {
                    // x-provided-by can only be set if x-provides isn't.
                    provider_relation_set.provided_by = x_provided_by.clone();
                    if let Some(provided_by) = x_provided_by {
                        let mut provided_by_set = provider_relation_sets
                            .get(&provided_by)
                            .unwrap_or(&ProviderRelationSet::new())
                            .clone();

                        provided_by_set.provides_to = Some(method.name.clone());

                        provider_relation_sets.insert(
                            FireboltOpenRpcMethod::name_with_lowercase_module(&provided_by),
                            provided_by_set.to_owned(),
                        );
                    }
                }

                provider_relation_set.uses = x_uses;
                provider_relation_set.event = has_event;

                // If this is an event, then it provides the capability.
                if provider_relation_set.event {
                    provider_relation_set.provides = provider_relation_set.capability.clone();
                }

                let module: Vec<&str> = method.name.split('.').collect();
                provider_relation_set.attributes = ProviderAttributes::get(module[0]);

                provider_relation_sets.insert(
                    FireboltOpenRpcMethod::name_with_lowercase_module(&method.name),
                    provider_relation_set.to_owned(),
                );
            }
        }

        self.provider_relation_map
            .write()
            .unwrap()
            .extend(provider_relation_sets)
    }
}

fn load_firebolt_open_rpc_path() -> Option<String> {
    let mut fb_open_rpc_file = "".to_string();
    if cfg!(feature = "local_dev") {
        let key = "FIREBOLT_OPEN_RPC";
        let env_var = std::env::var(key);
        if let Ok(path) = env_var {
            fb_open_rpc_file = path;
        };
    } else if cfg!(test) {
        fb_open_rpc_file = "../../openrpc_validator/src/test/firebolt-open-rpc.json".to_string();
    } else {
        fb_open_rpc_file = "/etc/ripple/openrpc/firebolt-open-rpc.json".to_string();
    }

    match std::fs::read_to_string(&fb_open_rpc_file) {
        Ok(content) => {
            debug!("loading firebolt_open_rpc from {}", &fb_open_rpc_file);
            Some(content)
        }
        Err(e) => {
            error!(
                "can't read firebolt_open_rpc from path :{}, e={:?}",
                &fb_open_rpc_file, e
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use ripple_sdk::api::manifest::extn_manifest::default_providers;

    use crate::state::openrpc_state::OpenRpcState;

    #[test]
    fn test_provider_support() {
        let state = OpenRpcState::new(None, Vec::new(), default_providers());
        assert!(state.is_provider_enabled("AcknowledgeChallenge.onRequestChallenge"));
        assert!(state.is_provider_enabled("PinChallenge."));
        assert!(state.is_provider_enabled("Discovery.userInterest"));
        assert!(state.is_provider_enabled("Discovery.onRequestUserInterest"));
        assert!(state.is_provider_enabled("Discovery.userInterestResponse"));
        assert!(state.is_provider_enabled("Content.requestUserInterest"));
        assert!(state.is_provider_enabled("Content.onUserInterest"));
        assert!(state.is_provider_enabled("IntegratedPlayer."));
        assert!(state.is_provider_enabled("integratedPlayer."));
        assert!(state.is_provider_enabled("integratedplayer."));
    }
}
