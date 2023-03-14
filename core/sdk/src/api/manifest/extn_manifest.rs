use std::{collections::HashMap, fs, path::Path};

use log::{info, warn};
use serde::Deserialize;

use crate::{extn::extn_capability::ExtnCapability, utils::error::RippleError};

/// Contains the default path for the manifest
/// file extension type based on platform
#[derive(Deserialize, Debug, Clone)]
pub struct ExtnManifest {
    pub default_path: String,
    pub default_extension: String,
    pub extns: Vec<ExtnEntry>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ExtnResolutionEntry {
    pub capability: String,
    pub priority: Option<u64>,
    pub exclusion: Option<bool>,
}

/// Contains Resolution strategies and path for the manifest.
#[derive(Deserialize, Debug, Clone)]
pub struct ExtnEntry {
    pub path: String,
    pub symbols: Vec<ExtnSymbol>,
    pub resolution: Option<Vec<ExtnResolutionEntry>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ExtnSymbol {
    pub capability: String,
    pub permissions: Vec<String>,
}

impl ExtnSymbol {
    fn get_launcher_capability(&self) -> Option<ExtnCapability> {
        if let Ok(cap) = ExtnCapability::try_from(self.capability.clone()) {
            if cap.is_launcher_channel() {
                return Some(cap);
            }
        }
        None
    }

    fn get_distributor_capability(&self) -> Option<ExtnCapability> {
        if let Ok(cap) = ExtnCapability::try_from(self.capability.clone()) {
            if cap.is_distributor_channel() {
                return Some(cap);
            }
        }
        None
    }
}

impl ExtnEntry {
    pub fn get_path(&self, default_path: &str, default_extn: &str) -> String {
        let path = self.path.clone();
        // has absolute path
        let path = match path.starts_with("/") {
            true => path,
            false => format!("{}{}", default_path, path),
        };

        let path = match Path::new(&path).extension() {
            Some(_) => path,
            None => format!("{}.{}", path, default_extn),
        };

        path
    }

    pub fn get_symbol(&self, capability: ExtnCapability) -> Option<ExtnSymbol> {
        let ref_cap = capability.to_string();
        self.symbols
            .clone()
            .into_iter()
            .find(|x| x.capability.eq(&ref_cap))
            .clone()
    }
}

impl ExtnManifest {
    pub fn load(path: String) -> Result<(String, ExtnManifest), RippleError> {
        info!("Trying to load device manifest from path={}", path);
        if let Ok(contents) = fs::read_to_string(&path) {
            Self::load_from_content(contents)
        } else {
            info!("No device manifest found in {}", path);
            Err(RippleError::MissingInput)
        }
    }

    pub fn load_from_content(contents: String) -> Result<(String, ExtnManifest), RippleError> {
        match serde_json::from_str::<ExtnManifest>(&contents) {
            Ok(manifest) => Ok((String::from(contents), manifest)),
            Err(err) => {
                warn!("{:?} could not load device manifest", err);
                Err(RippleError::InvalidInput)
            }
        }
    }

    pub fn get_launcher_capability(&self) -> Option<ExtnCapability> {
        for extn in self.extns.clone() {
            for symbol in extn.symbols {
                if let Some(cap) = symbol.get_launcher_capability() {
                    return Some(cap);
                }
            }
        }
        return None;
    }

    pub fn get_distributor_capability(&self) -> Option<ExtnCapability> {
        for extn in self.extns.clone() {
            for symbol in extn.symbols {
                if let Some(cap) = symbol.get_distributor_capability() {
                    return Some(cap);
                }
            }
        }
        return None;
    }

    pub fn get_extn_permissions(&self) -> HashMap<String, Vec<String>> {
        let mut map = HashMap::new();
        self.extns.clone().into_iter().for_each(|x| {
            x.symbols.into_iter().for_each(|y| {
                if let Ok(cap) = ExtnCapability::try_from(y.capability) {
                    map.insert(cap.to_string(), y.permissions.clone());
                }
            })
        });
        map
    }
}
