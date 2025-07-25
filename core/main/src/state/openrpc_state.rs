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

use openrpc_validator::jsonschema::JSONSchema;
use ripple_sdk::log::{debug, error, info};
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

use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use openrpc_validator::{FireboltOpenRpc as FireboltOpenRpcValidator, RpcMethodValidator};

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

#[derive(Debug, Clone, Default)]
pub struct OpenRpcState {
    open_rpc: Arc<FireboltOpenRpc>,
    exclusory: Arc<Option<ExclusoryImpl>>,
    firebolt_cap_map: Arc<RwLock<HashMap<String, CapabilitySet>>>,
    ripple_cap_map: Arc<RwLock<HashMap<String, CapabilitySet>>>,
    cap_policies: Arc<RwLock<HashMap<String, CapabilityPolicy>>>,
    extended_rpc: Arc<RwLock<Vec<FireboltOpenRpc>>>,
    provider_relation_map: Arc<RwLock<HashMap<String, ProviderRelationSet>>>,
    openrpc_validator: Arc<RwLock<RpcMethodValidator>>,
    provider_registrations: Arc<Vec<String>>,
    json_schema_cache: Arc<RwLock<HashMap<String, JSONSchema>>>,
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

    pub fn new_instance(
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
        let mut rpc_method_validator = RpcMethodValidator::new();
        rpc_method_validator.add_schema(openrpc_validator);
        let v = OpenRpcState {
            firebolt_cap_map: Arc::new(RwLock::new(firebolt_open_rpc.get_methods_caps())),
            ripple_cap_map: Arc::new(RwLock::new(ripple_open_rpc.get_methods_caps())),
            exclusory: Arc::new(exclusory),
            cap_policies: Arc::new(RwLock::new(version_manifest.capabilities)),
            open_rpc: Arc::new(firebolt_open_rpc.clone()),
            extended_rpc: Arc::new(RwLock::new(Vec::new())),
            provider_relation_map: Arc::new(RwLock::new(HashMap::new())),
            openrpc_validator: Arc::new(RwLock::new(rpc_method_validator)),
            provider_registrations: Arc::new(provider_registrations),
            json_schema_cache: Arc::new(RwLock::new(HashMap::new())),
        };
        v.build_provider_relation_sets(&firebolt_open_rpc.methods);
        for path in extn_sdks {
            if v.add_extension_open_rpc(&path).is_err() {
                error!("Error adding extn_sdk from {path}");
            }
            if v.add_extension_open_rpc_to_validator(path).is_err() {
                error!("Error adding openrpc extn_sdk to validator");
            }
        }

        v
    }
    pub fn new(
        exclusory: Option<ExclusoryImpl>,
        extn_sdks: Vec<String>,
        provider_registrations: Vec<String>,
    ) -> Arc<OpenRpcState> {
        Arc::new(Self::new_instance(
            exclusory,
            extn_sdks,
            provider_registrations,
        ))
    }

    pub fn add_open_rpc(&self, open_rpc: FireboltOpenRpc) {
        self.extend_caps(open_rpc.get_methods_caps());
        self.extend_policies(open_rpc.get_capability_policy());

        let mut ext_rpcs = self.extended_rpc.write().unwrap();
        ext_rpcs.push(open_rpc);
    }

    pub fn is_app_excluded(&self, app_id: &str) -> bool {
        if let Some(e) = &*self.exclusory {
            return e.is_app_all_excluded(app_id);
        }

        false
    }

    // Add extension open rpc to the validator
    pub fn add_extension_open_rpc_to_validator(&self, path: String) -> Result<(), RippleError> {
        let extension_open_rpc_string = load_extension_open_rpc(path);
        if let Some(open_rpc) = extension_open_rpc_string {
            return match serde_json::from_str::<FireboltOpenRpcValidator>(&open_rpc) {
                Ok(additional_open_rpc_validator) => {
                    let mut validator = self.openrpc_validator.write().unwrap();
                    validator.add_schema(additional_open_rpc_validator);
                    return Ok(());
                }
                Err(e) => {
                    error!("Error parsing openrpc validator from e={:?}", e);
                    Err(RippleError::ParseError)
                }
            };
        };
        Err(RippleError::ParseError)
    }

    pub fn is_excluded(&self, method: String, app_id: String) -> bool {
        if let Some(e) = &*self.exclusory {
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

    pub fn get_open_rpc(&self) -> Arc<FireboltOpenRpc> {
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

    pub fn get_openrpc_validator(&self) -> RpcMethodValidator {
        self.openrpc_validator.read().unwrap().clone()
    }

    pub fn is_provider_enabled(&self, method: &str) -> bool {
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

    pub fn add_json_schema_cache(&self, method: String, schema: JSONSchema) {
        let mut json_cache = self.json_schema_cache.write().unwrap();
        let _ = json_cache.insert(method, schema);
    }

    pub fn validate_schema(&self, method: &str, value: &Value) -> Result<(), Option<String>> {
        let json_cache = self.json_schema_cache.read().unwrap();
        if let Some(schema) = json_cache.get(method) {
            if let Err(e) = schema.validate(value) {
                let mut error_string = String::new();
                for error in e {
                    error_string.push_str(&format!("{} ", error));
                }
                Err(Some(error_string))
            } else {
                Ok(())
            }
        } else {
            Err(None)
        }
    }
}

fn load_firebolt_open_rpc_from_file(fb_open_rpc_file: &str) -> Result<String, RippleError> {
    match std::fs::read_to_string(fb_open_rpc_file) {
        Ok(content) => {
            debug!("loading firebolt_open_rpc from {}", &fb_open_rpc_file);
            Ok(content)
        }
        Err(e) => {
            error!(
                "can't read firebolt_open_rpc from path: {}, e={:?}",
                &fb_open_rpc_file, e
            );
            Err(RippleError::ProcessorError)
        }
    }
}
/*
this is only used once so far, but a bit more maintainable as a const
*/

/*
test , local_dev and contract tests load the firebolt open rpc file from either a path or from the openrpc_validator version as compiled in.
*/
#[cfg(any(
    feature = "local_dev",
    feature = "websocket_contract_tests",
    feature = "http_contract_tests",
    test
))]
fn load_firebolt_open_rpc_path() -> Result<String, RippleError> {
    if let Ok(path) = std::env::var("FIREBOLT_OPEN_RPC") {
        info!(
            " loading firebolt_open_rpc from FIREBOLT_OPEN_RPC env var path: {}",
            &path
        );
        load_firebolt_open_rpc_from_file(&path)
    } else {
        info!(" local_dev or contract_tests: loading firebolt_open_rpc from file openrpc_validator/src/test/firebolt-open-rpc.json");
        let fb_open_rpc_file =
            include_str!("../../../../openrpc_validator/src/test/firebolt-open-rpc.json");
        Ok(fb_open_rpc_file.to_string())
    }
}
// /*
// Production load of the firebolt open rpc file
// */
#[cfg(not(any(
    feature = "local_dev",
    feature = "websocket_contract_tests",
    feature = "http_contract_tests",
    test
)))]
fn load_firebolt_open_rpc_path() -> Result<String, RippleError> {
    info!(
        "production: loading firebolt_open_rpc from file {}",
        "/etc/ripple/openrpc/firebolt-open-rpc.json"
    );
    load_firebolt_open_rpc_from_file("/etc/ripple/openrpc/firebolt-open-rpc.json")
}

fn load_extension_open_rpc(path: String) -> Option<String> {
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            debug!("loading extension open_rpc from {}", &path);
            Some(content)
        }
        Err(e) => {
            error!(
                "can't read extension_open_rpc from path :{}, e={:?}",
                &path, e
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
