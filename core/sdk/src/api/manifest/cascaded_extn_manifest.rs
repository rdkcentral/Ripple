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

use super::extn_manifest::{ExtnManifest, ExtnManifestEntry};
use super::MergeConfig;
use log::{info, warn};
use serde::Deserialize;
use std::collections::HashMap;
use std::{fs, path::Path};

use crate::{extn::extn_id::ExtnId, utils::error::RippleError};

/// Contains the default path for the manifest
/// file extension type based on platform
#[derive(Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct CascadedExtnManifest {
    pub default_path: Option<String>,
    pub default_extension: Option<String>,
    pub extns: Option<Vec<ExtnManifestEntry>>,
    pub required_contracts: Option<Vec<String>>,
    pub rpc_aliases: Option<HashMap<String, Vec<String>>>,
    pub rpc_overrides: Option<HashMap<String, String>>,
    pub timeout: Option<u64>,
    pub rules_path: Option<Vec<String>>,
    pub extn_sdks: Option<Vec<String>>,
    pub provider_registrations: Option<Vec<String>>,
}

#[derive(Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct ExtnResolutionEntry {
    pub capability: String,
    pub priority: Option<u64>,
    pub exclusion: Option<bool>,
}

impl MergeConfig<CascadedExtnManifest> for ExtnManifest {
    fn merge_config(&mut self, cascaded: CascadedExtnManifest) {
        if let Some(cas_default_path) = cascaded.default_path {
            self.default_path = cas_default_path;
        }
        if let Some(cas_default_extension) = cascaded.default_extension {
            self.default_extension = cas_default_extension
        }
        if let Some(cas_extens) = cascaded.extns {
            self.extns.extend(cas_extens);
        }
        if let Some(cas_required_contracts) = cascaded.required_contracts {
            self.extn_sdks.extend(cas_required_contracts);
        }
        if let Some(cas_rpc_aliases) = cascaded.rpc_aliases {
            for (key, values) in cas_rpc_aliases {
                self.rpc_aliases
                    .entry(key)
                    .or_default()
                    .extend(values);
            }
        }
        if let Some(cas_rpc_overrides) = cascaded.rpc_overrides {
            self.rpc_overrides.extend(cas_rpc_overrides);
        }
        if let Some(cas_timeout) = cascaded.timeout {
            self.timeout = Some(cas_timeout);
        }
        if let Some(cas_rules_path) = cascaded.rules_path {
            self.rules_path.extend(cas_rules_path);
        }
        if let Some(cas_extn_sdks) = cascaded.extn_sdks {
            self.extn_sdks.extend(cas_extn_sdks);
        }
        if let Some(cas_provider_reg) = cascaded.provider_registrations {
            self.provider_registrations.extend(cas_provider_reg);
        }
    }
}

impl CascadedExtnManifest {
    pub fn load(path: String) -> Result<(String, CascadedExtnManifest), RippleError> {
        info!("Trying to load device manifest from path={}", path);
        if let Some(p) = Path::new(&path).to_str() {
            if let Ok(contents) = fs::read_to_string(p) {
                return Self::load_from_content(contents);
            }
        }
        info!("No device manifest found in {}", path);
        Err(RippleError::MissingInput)
    }

    pub fn load_from_content(
        contents: String,
    ) -> Result<(String, CascadedExtnManifest), RippleError> {
        match serde_json::from_str::<CascadedExtnManifest>(&contents) {
            Ok(manifest) => Ok((contents, manifest)),
            Err(err) => {
                warn!("{:?} could not load device manifest", err);
                Err(RippleError::InvalidInput)
            }
        }
    }

    pub fn get_launcher_capability(&self) -> Option<ExtnId> {
        for extn in self.extns.clone().unwrap() {
            for symbol in extn.symbols {
                if let Some(cap) = symbol.get_launcher_capability() {
                    return Some(cap);
                }
            }
        }

        None
    }

    pub fn get_distributor_capability(&self) -> Option<ExtnId> {
        for extn in self.extns.clone().unwrap() {
            for symbol in extn.symbols {
                if let Some(cap) = symbol.get_distributor_capability() {
                    return Some(cap);
                }
            }
        }

        None
    }

    pub fn get_extn_permissions(&self) -> HashMap<String, Vec<String>> {
        let mut map = HashMap::new();
        self.extns.clone().unwrap().into_iter().for_each(|x| {
            x.symbols.into_iter().for_each(|y| {
                if let Ok(cap) = ExtnId::try_from(y.id) {
                    map.insert(cap.to_string(), y.uses);
                }
            })
        });
        map
    }

    pub fn get_timeout(&self) -> u64 {
        self.timeout.unwrap_or(10000)
    }

    pub fn has_rpc_override_method(&self, method: &str) -> Option<String> {
        self.rpc_overrides.clone().unwrap().get(method).cloned()
    }
}

/// Some unit tests which use defaults are failing because we need default providers for unit testing
impl Default for CascadedExtnManifest {
    fn default() -> Self {
        Self {
            default_path: Some(String::default()),
            default_extension: Some(String::default()),
            extns: Some(Vec::new()),
            required_contracts: Some(Vec::new()),
            rpc_aliases: Some(HashMap::new()),
            rpc_overrides: Some(HashMap::new()),
            timeout: None,
            rules_path: Some(Vec::new()),
            extn_sdks: Some(Vec::new()),
            provider_registrations: Some(default_providers()),
        }
    }
}

pub fn default_providers() -> Vec<String> {
    let value = [
        "AcknowledgeChallenge.",
        "PinChallenge.",
        "Discovery.userInterest",
        "Discovery.onRequestUserInterest",
        "Discovery.userInterestResponse",
        "Content.requestUserInterest",
        "Content.onUserInterest",
        "IntegratedPlayer.",
    ];
    value.iter().map(|x| x.to_string()).collect()
}
