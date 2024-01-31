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
#[derive(Deserialize, Debug, Clone)]
pub struct ExtnManifest {
    pub default_path: String,
    pub default_extension: String,
    pub extns: Vec<ExtnManifestEntry>,
    pub required_contracts: Vec<String>,
    pub rpc_aliases: HashMap<String, Vec<String>>,
    pub timeout: Option<u64>,
    pub passthrough_rpcs: Option<PassthroughRpcs>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PassthroughRpcs {
    pub endpoints: Vec<PassthroughEndpoint>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PassthroughEndpoint {
    pub url: String,
    pub protocol: PassthroughProtocol,
    pub rpcs: Vec<String>,
    pub authenticaton: Option<String>,
    pub token: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum PassthroughProtocol {
    Websocket,
    Http,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ExtnResolutionEntry {
    pub capability: String,
    pub priority: Option<u64>,
    pub exclusion: Option<bool>,
}

/// Contains Resolution strategies and path for the manifest.
#[derive(Deserialize, Debug, Clone)]
pub struct ExtnManifestEntry {
    pub path: String,
    pub symbols: Vec<ExtnSymbol>,
    pub resolution: Option<Vec<ExtnResolutionEntry>>,
}

#[derive(Deserialize, Debug, Clone)]
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
