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
use super::extn_manifest::{ExtnManifest, ExtnManifestEntry, ExtnResolutionEntry, ExtnSymbol};
use super::MergeConfig;
use crate::utils::error::RippleError;
use log::{info, warn};
use serde::Deserialize;
use std::collections::HashMap;
use std::{fs, path::Path};
/// Contains the default path for the manifest
/// file extension type based on platform
#[derive(Deserialize, Debug, Clone)]
pub struct CascadedExtnManifest {
    pub default_path: Option<String>,
    pub default_extension: Option<String>,
    pub extns: Option<Vec<CascadedExtnManifestEntry>>,
    pub required_contracts: Option<Vec<String>>,
    pub rpc_aliases: Option<HashMap<String, Vec<String>>>,
    pub rpc_overrides: Option<HashMap<String, String>>,
    pub timeout: Option<u64>,
    pub rules_path: Option<Vec<String>>,
    pub extn_sdks: Option<Vec<String>>,
    pub provider_registrations: Option<Vec<String>>,
}
impl MergeConfig<CascadedExtnManifest> for ExtnManifest {
    fn merge_config(&mut self, cascaded: CascadedExtnManifest) {
        if let Some(cas_default_path) = cascaded.default_path {
            self.default_path = cas_default_path;
        }
        if let Some(cas_default_extension) = cascaded.default_extension {
            self.default_extension = cas_default_extension
        }
        if let Some(cascaded_extns) = cascaded.extns {
            for cascaded_entry in cascaded_extns {
                if let Some(path) = &cascaded_entry.path {
                    // Try to find a matching entry by path to merge, otherwise push a new one
                    if let Some(existing_entry) = self.extns.iter_mut().find(|e| &e.path == path) {
                        existing_entry.merge_config(cascaded_entry);
                    } else if let Some(new_entry) = ExtnManifestEntry::from_cascaded(cascaded_entry)
                    {
                        self.extns.push(new_entry);
                    }
                }
            }
        }
        if let Some(cas_required_contracts) = cascaded.required_contracts {
            self.required_contracts.extend(cas_required_contracts);
            self.required_contracts.sort();
        }
        if let Some(cas_rpc_aliases) = cascaded.rpc_aliases {
            for (key, values) in cas_rpc_aliases {
                self.rpc_aliases.entry(key).or_default().extend(values);
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
            self.extn_sdks.sort();
        }
        if let Some(cas_extn_sdks) = cascaded.extn_sdks {
            self.extn_sdks.extend(cas_extn_sdks);
            self.extn_sdks.sort();
        }
        if let Some(cas_provider_reg) = cascaded.provider_registrations {
            self.provider_registrations.extend(cas_provider_reg);
            self.provider_registrations.sort();
        }
    }
}

impl CascadedExtnManifest {
    pub fn load(path: String) -> Result<(String, CascadedExtnManifest), RippleError> {
        info!("Trying to load extn manifest from path={}", path);
        if let Some(p) = Path::new(&path).to_str() {
            if let Ok(contents) = fs::read_to_string(p) {
                return Self::load_from_content(contents);
            }
        }
        info!("No extns manifest found in {}", path);
        Err(RippleError::MissingInput)
    }
    pub fn load_from_content(
        contents: String,
    ) -> Result<(String, CascadedExtnManifest), RippleError> {
        match serde_json::from_str::<CascadedExtnManifest>(&contents) {
            Ok(manifest) => Ok((contents, manifest)),
            Err(err) => {
                warn!("{:?} could not load extn manifest", err);
                Err(RippleError::InvalidInput)
            }
        }
    }
}

impl ExtnManifestEntry {
    fn from_cascaded(cascaded: CascadedExtnManifestEntry) -> Option<Self> {
        cascaded.path.map(|path| ExtnManifestEntry {
            path,
            symbols: cascaded
                .symbols
                .unwrap_or_default()
                .into_iter()
                .map(|s| ExtnSymbol {
                    id: s.id.unwrap_or_default(), // Default if no ID in Cascaded
                    uses: s.uses.unwrap_or_default(),
                    fulfills: s.fulfills.unwrap_or_default(),
                    config: s
                        .config
                        .map(|c| {
                            c.into_iter()
                                .filter_map(|(k, v)| v.map(|vv| (k, vv)))
                                .collect()
                        })
                        .filter(|c: &std::collections::HashMap<String, String>| !c.is_empty()),
                })
                .collect(),
            resolution: cascaded.resolution,
        })
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct CascadedExtnManifestEntry {
    pub path: Option<String>,
    pub symbols: Option<Vec<CascadedExtnSymbol>>,
    pub resolution: Option<Vec<ExtnResolutionEntry>>,
}

impl MergeConfig<CascadedExtnManifestEntry> for ExtnManifestEntry {
    fn merge_config(&mut self, cascaded: CascadedExtnManifestEntry) {
        if let Some(path) = cascaded.path {
            self.path = path;
        }
        if let Some(cascaded_symbols) = cascaded.symbols {
            for cascaded_symbol in cascaded_symbols {
                // Try to find a matching symbol by ID to merge, otherwise push a new one
                if let Some(existing_symbol) = self
                    .symbols
                    .iter_mut()
                    .find(|s| s.id == *cascaded_symbol.id.as_ref().unwrap_or(&String::new()))
                {
                    existing_symbol.merge_config(cascaded_symbol);
                } else if let Some(id) = cascaded_symbol.id {
                    // If no ID, we can't really merge reliably, so we only add if there's an ID
                    self.symbols.push(ExtnSymbol {
                        id,
                        uses: cascaded_symbol.uses.unwrap_or_default(),
                        fulfills: cascaded_symbol.fulfills.unwrap_or_default(),
                        config: cascaded_symbol.config.map(|c| {
                            c.into_iter()
                                .filter_map(|(k, v)| v.map(|vv| (k, vv)))
                                .collect()
                        }),
                    });
                }
            }
        }
        if let Some(cascaded_resolution) = cascaded.resolution {
            match &mut self.resolution {
                Some(existing_resolution) => {
                    for cascaded_entry in cascaded_resolution {
                        if let Some(existing_entry) = existing_resolution
                            .iter_mut()
                            .find(|r| r.capability == cascaded_entry.capability)
                        {
                            existing_entry.priority = cascaded_entry.priority;
                            existing_entry.exclusion = cascaded_entry.exclusion;
                        } else {
                            existing_resolution.push(cascaded_entry);
                        }
                    }
                }
                None => {
                    self.resolution = Some(cascaded_resolution);
                }
            }
        }
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct CascadedExtnSymbol {
    pub id: Option<String>,
    pub uses: Option<Vec<String>>,
    pub fulfills: Option<Vec<String>>,
    pub config: Option<HashMap<String, Option<String>>>,
}
impl MergeConfig<CascadedExtnSymbol> for ExtnSymbol {
    fn merge_config(&mut self, cascaded: CascadedExtnSymbol) {
        if let Some(id) = cascaded.id {
            self.id = id;
        }
        if let Some(uses) = cascaded.uses {
            self.uses.extend(uses);
            self.uses.dedup(); // Remove duplicates if needed
        }
        if let Some(fulfills) = cascaded.fulfills {
            self.fulfills.extend(fulfills);
            self.fulfills.dedup(); // Remove duplicates if needed
        }
        if let Some(cascaded_config) = cascaded.config {
            match &mut self.config {
                Some(existing_config) => {
                    for (key, value) in cascaded_config {
                        if let Some(val) = value {
                            existing_config.insert(key, val);
                        } else {
                            existing_config.remove(&key); // Remove if Cascaded has None
                        }
                    }
                }
                None => {
                    let mut new_config = HashMap::new();
                    for (key, value) in cascaded_config {
                        if let Some(val) = value {
                            new_config.insert(key, val);
                        }
                    }
                    if !new_config.is_empty() {
                        self.config = Some(new_config);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::api::manifest::extn_manifest::tests::Mockable as mock_extn_manifests;

    use super::*;
    use ripple_sdk::Mockable;

    impl Mockable for CascadedExtnManifest {
        fn mock() -> Self {
            let (_, manifest) = CascadedExtnManifest::load_from_content(
                include_str!("mock_manifests/cascaded-extn-manifest-example.json").to_string(),
            )
            .unwrap();
            manifest
        }
    }

    #[test]
    fn test_get_merge_config_timeout() {
        let mut default_manifest = ExtnManifest::mock();
        let cascaded_manifest = CascadedExtnManifest::mock();
        default_manifest.merge_config(cascaded_manifest);
        let timeout = default_manifest.get_timeout();
        assert_eq!(timeout, 10000);
    }

    #[test]
    fn test_get_merge_config_default_path() {
        let mut default_manifest = ExtnManifest::mock();
        let cascaded_manifest = CascadedExtnManifest::mock();
        default_manifest.merge_config(cascaded_manifest);
        assert_eq!(default_manifest.default_path, "/tmp");
    }

    #[test]
    fn test_get_merge_config_default_extension() {
        let mut default_manifest = ExtnManifest::mock();
        let cascaded_manifest = CascadedExtnManifest::mock();
        default_manifest.merge_config(cascaded_manifest);
        assert_eq!(default_manifest.default_extension, "json");
    }

    #[test]
    fn test_get_merge_config_required_contracts() {
        let mut default_manifest = ExtnManifest::mock();
        // Modify the mock CascadedExtnManifest to include "make"
        let cascaded_manifest = CascadedExtnManifest::mock();
        default_manifest.merge_config(cascaded_manifest);
        let contains_mock_val_a = default_manifest
            .required_contracts
            .contains(&"mock_val_a".to_string());
        assert!(
            contains_mock_val_a,
            "Expected 'mock_val_a' to be present in required_contracts"
        );
    }

    #[test]
    fn test_get_merge_config_extn_manifest_entry() {
        let mut default_manifest = ExtnManifest::mock();
        let cascaded_manifest = CascadedExtnManifest::mock();
        default_manifest.merge_config(cascaded_manifest);
        // Check if an extension with path "libsocket" is present
        let libsocket_present = default_manifest
            .extns
            .iter()
            .any(|extn| extn.path == "libsocket");
        assert!(
            libsocket_present,
            "Expected an extension with path 'libsocket'"
        );
    }

    #[test]
    fn test_get_merge_config_extn_symbol() {
        let mut default_manifest = ExtnManifest::mock();
        let cascaded_manifest = CascadedExtnManifest::mock();
        default_manifest.merge_config(cascaded_manifest);
        // Check if an extension with path "libsocket" is present
        if let Some(libsocket_extn) = default_manifest
            .extns
            .iter()
            .find(|extn| extn.path == "libsocket")
        {
            let symbol_id_present = libsocket_extn
                .symbols
                .iter()
                .any(|sym| sym.id == "ripple:channel:device:socket");
            assert!(
                symbol_id_present,
                "Expected symbol 'ripple:channel:device:socket' in 'libsocket'"
            );
        }
    }
}
