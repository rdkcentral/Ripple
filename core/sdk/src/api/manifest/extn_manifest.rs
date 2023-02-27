use std::{fs, path::Path};

use log::{info, warn};
use serde::Deserialize;

use crate::utils::error::RippleError;

/// Contains the default path for the manifest
/// file extension type based on platform
/// Resolution strategies
///

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

#[derive(Deserialize, Debug, Clone)]
pub struct ExtnEntry {
    pub path: String,
    pub resolution: Option<Vec<ExtnResolutionEntry>>,
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
}
