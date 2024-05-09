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

use log::{info, warn};
use serde::Deserialize;
use std::collections::HashMap;
use std::{fs, path::Path};

use crate::{extn::extn_id::ExtnId, utils::error::RippleError};

/// Contains the default path for the manifest
/// file extension type based on platform
#[derive(Deserialize, Debug, Clone, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct ExtnManifest {
    pub default_path: String,
    pub default_extension: String,
    pub extns: Vec<ExtnManifestEntry>,
    pub required_contracts: Vec<String>,
    pub rpc_aliases: HashMap<String, Vec<String>>,
    pub timeout: Option<u64>,
}

#[derive(Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct ExtnResolutionEntry {
    pub capability: String,
    pub priority: Option<u64>,
    pub exclusion: Option<bool>,
}

/// Contains Resolution strategies and path for the manifest.
#[derive(Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct ExtnManifestEntry {
    pub path: String,
    pub symbols: Vec<ExtnSymbol>,
    pub resolution: Option<Vec<ExtnResolutionEntry>>,
}

#[derive(Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct ExtnSymbol {
    pub id: String,
    pub uses: Vec<String>,
    pub fulfills: Vec<String>,
    pub config: Option<HashMap<String, String>>,
}

impl ExtnSymbol {
    fn get_launcher_capability(&self) -> Option<ExtnId> {
        if let Ok(cap) = ExtnId::try_from(self.id.clone()) {
            if cap.is_launcher_channel() {
                return Some(cap);
            }
        }
        None
    }

    fn get_distributor_capability(&self) -> Option<ExtnId> {
        if let Ok(cap) = ExtnId::try_from(self.id.clone()) {
            if cap.is_distributor_channel() {
                return Some(cap);
            }
        }
        None
    }
}

impl ExtnManifestEntry {
    pub fn get_path(&self, default_path: &str, default_extn: &str) -> String {
        let path = self.path.clone();
        // has absolute path
        let path = match path.starts_with('/') {
            true => path,
            false => format!("{}{}", default_path, path),
        };

        let path = match Path::new(&path).extension() {
            Some(_) => path,
            None => format!("{}.{}", path, default_extn),
        };

        path
    }

    pub fn get_symbol(&self, capability: ExtnId) -> Option<ExtnSymbol> {
        let ref_cap = capability.to_string();
        self.symbols.clone().into_iter().find(|x| x.id.eq(&ref_cap))
    }
}

impl ExtnManifest {
    pub fn load(path: String) -> Result<(String, ExtnManifest), RippleError> {
        info!("Trying to load device manifest from path={}", path);
        if let Some(p) = Path::new(&path).to_str() {
            if let Ok(contents) = fs::read_to_string(p) {
                return Self::load_from_content(contents);
            }
        }
        info!("No device manifest found in {}", path);
        Err(RippleError::MissingInput)
    }

    pub fn load_from_content(contents: String) -> Result<(String, ExtnManifest), RippleError> {
        match serde_json::from_str::<ExtnManifest>(&contents) {
            Ok(manifest) => Ok((contents, manifest)),
            Err(err) => {
                warn!("{:?} could not load device manifest", err);
                Err(RippleError::InvalidInput)
            }
        }
    }

    pub fn get_launcher_capability(&self) -> Option<ExtnId> {
        for extn in self.extns.clone() {
            for symbol in extn.symbols {
                if let Some(cap) = symbol.get_launcher_capability() {
                    return Some(cap);
                }
            }
        }

        None
    }

    pub fn get_distributor_capability(&self) -> Option<ExtnId> {
        for extn in self.extns.clone() {
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
        self.extns.clone().into_iter().for_each(|x| {
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
}
#[cfg(test)]
mod tests {
    use crate::extn::extn_id::{ExtnClassId, ExtnType};

    use super::*;

    pub trait Mockable {
        fn mock() -> ExtnManifest
        where
            Self: Sized;
    }
    #[cfg(test)]
    impl Mockable for ExtnManifest {
        fn mock() -> ExtnManifest {
            ExtnManifest {
                default_path: "/tmp".to_string(),
                default_extension: "json".to_string(),
                extns: vec![],
                required_contracts: vec![],
                rpc_aliases: HashMap::new(),
                timeout: Some(5000),
            }
        }
    }

    #[test]
    fn test_get_path() {
        let mut extn_manifest_entry = ExtnManifestEntry {
            path: "/absolute/path".to_string(),
            symbols: vec![],
            resolution: None,
        };

        assert_eq!(
            extn_manifest_entry.get_path("/default/path/", "default_extn"),
            "/absolute/path.default_extn"
        );

        extn_manifest_entry.path = "relative/path".to_string();
        assert_eq!(
            extn_manifest_entry.get_path("/default/path/", "default_extn"),
            "/default/path/relative/path.default_extn"
        );
    }

    #[test]
    fn test_get_symbol() {
        let dist_channel = ExtnId::new_channel(ExtnClassId::Distributor, "test".into()).to_string();
        let symbol = ExtnSymbol {
            id: dist_channel.clone(),
            uses: vec![],
            fulfills: vec![],
            config: None,
        };
        let extn_manifest_entry = ExtnManifestEntry {
            path: "relative/path".to_string(),
            symbols: vec![symbol.clone()],
            resolution: None,
        };
        assert_eq!(
            extn_manifest_entry.get_symbol(ExtnId::try_from(dist_channel).unwrap()),
            Some(symbol)
        );
    }

    #[test]
    fn test_load_from_content_valid() {
        let contents = r#"
            {
                "default_path": "",
                "default_extension": "",
                "extns": [],
                "required_contracts": [],
                "rpc_aliases": {},
                "timeout": null
            }
        "#;

        let expected_manifest = ExtnManifest {
            default_path: "".to_string(),
            default_extension: "".to_string(),
            extns: vec![],
            required_contracts: vec![],
            rpc_aliases: HashMap::new(),
            timeout: None,
        };

        assert_eq!(
            ExtnManifest::load_from_content(contents.to_string()),
            Ok((contents.to_string(), expected_manifest))
        );
    }

    #[test]
    fn test_load_from_content_invalid() {
        let contents = "invalid_json";

        assert_eq!(
            ExtnManifest::load_from_content(contents.to_string()),
            Err(RippleError::InvalidInput)
        );
    }

    #[test]
    fn test_symbol_get_launcher_capability() {
        let launcher_channel =
            ExtnId::new_channel(ExtnClassId::Launcher, "test".into()).to_string();
        let symbol = ExtnSymbol {
            id: launcher_channel,
            uses: vec![],
            fulfills: vec![],
            config: None,
        };

        let capability = symbol.get_launcher_capability();
        assert_eq!(
            capability,
            Some(ExtnId {
                _type: ExtnType::Channel,
                class: ExtnClassId::Launcher,
                service: "test".to_string(),
            })
        );
    }

    #[test]
    fn test_symbol_get_distributor_capability() {
        let dist_channel = ExtnId::new_channel(ExtnClassId::Distributor, "test".into()).to_string();
        let symbol = ExtnSymbol {
            id: dist_channel,
            uses: vec![],
            fulfills: vec![],
            config: None,
        };

        let capability = symbol.get_distributor_capability();
        assert_eq!(
            capability,
            Some(ExtnId {
                _type: ExtnType::Channel,
                class: ExtnClassId::Distributor,
                service: "test".to_string(),
            })
        );
    }

    #[test]
    fn test_get_distributor_capability() {
        let mut manifest = ExtnManifest::mock();
        let dist_channel = ExtnId::new_channel(ExtnClassId::Distributor, "test".into()).to_string();
        let symbol = ExtnSymbol {
            id: dist_channel.clone(),
            uses: vec![],
            fulfills: vec![],
            config: None,
        };
        let extn_manifest_entry = ExtnManifestEntry {
            path: "relative/path".to_string(),
            symbols: vec![symbol],
            resolution: None,
        };
        manifest.extns = vec![extn_manifest_entry];

        assert_eq!(
            manifest.get_distributor_capability(),
            Some(ExtnId::try_from(dist_channel).unwrap())
        );
    }

    #[test]
    fn test_get_launcher_capability() {
        let mut manifest = ExtnManifest::mock();
        let launcher_channel =
            ExtnId::new_channel(ExtnClassId::Launcher, "test".into()).to_string();
        let symbol = ExtnSymbol {
            id: launcher_channel.clone(),
            uses: vec![],
            fulfills: vec![],
            config: None,
        };
        let extn_manifest_entry = ExtnManifestEntry {
            path: "relative/path".to_string(),
            symbols: vec![symbol],
            resolution: None,
        };

        manifest.extns = vec![extn_manifest_entry];
        assert_eq!(
            manifest.get_launcher_capability(),
            Some(ExtnId::try_from(launcher_channel).unwrap())
        );
    }

    #[test]
    fn test_get_extn_permissions() {
        let mut manifest = ExtnManifest::mock();
        let launcher_channel =
            ExtnId::new_channel(ExtnClassId::Launcher, "test".into()).to_string();
        let symbol = ExtnSymbol {
            id: launcher_channel.clone(),
            uses: vec!["config".to_string()],
            fulfills: vec!["test".to_string()],
            config: None,
        };
        let extn_manifest_entry = ExtnManifestEntry {
            path: "relative/path".to_string(),
            symbols: vec![symbol],
            resolution: None,
        };

        manifest.extns = vec![extn_manifest_entry];
        let mut expected_map = HashMap::new();
        expected_map.insert(launcher_channel, vec!["config".to_string()]);

        assert_eq!(manifest.get_extn_permissions(), expected_map);
    }

    #[test]
    fn test_get_timeout() {
        let mut manifest = ExtnManifest::mock();
        assert_eq!(manifest.get_timeout(), 5000);

        manifest.timeout = None;
        assert_eq!(manifest.get_timeout(), 10000);
    }
}
