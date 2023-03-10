use std::fs;

use ripple_sdk::{
    api::manifest::{app_library::DefaultLibrary, device_manifest::AppLibraryEntry},
    log::{info, warn},
    serde_json,
    utils::error::RippleError,
};
pub struct LoadAppLibraryStep;

impl LoadAppLibraryStep {
    pub fn load_app_library(path: String) -> Result<Vec<AppLibraryEntry>, RippleError> {
        info!("Trying to load app library from {}", path);
        let result = match fs::read_to_string(&path) {
            Ok(contents) => match serde_json::from_str::<DefaultLibrary>(&contents) {
                Ok(al) => Ok(al.default_library),
                Err(_) => {
                    warn!("could not load app library from path {}", path);
                    Err(RippleError::InvalidInput)
                }
            },
            Err(e) => {
                info!("Error: e={}", e);
                Err(RippleError::MissingInput)
            }
        };

        Ok(result.expect("Need valid App Library"))
    }
}
