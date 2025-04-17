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

use crate::utils::error::RippleError;

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
            self.required_contracts.extend(cas_required_contracts);
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
