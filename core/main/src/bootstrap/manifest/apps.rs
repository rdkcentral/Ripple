use std::fs;

use ripple_sdk::{
    api::manifest::{app_library::DefaultLibrary, device_manifest::AppLibraryEntry},
    log::{error, info, warn},
    serde_json,
    utils::error::RippleError,
};
pub struct LoadAppLibraryStep;

impl LoadAppLibraryStep {
    pub fn get_manifest(path: String) -> Result<Vec<AppLibraryEntry>, RippleError> {
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

        if let Ok(library) = result {
            return Ok(library);
        }

        info!("loading default app library");
        let contents = std::include_str!("./default-app-library.json");
        info!("loaded_default_app_library_file_content={}", contents);
        match serde_json::from_str::<DefaultLibrary>(&contents) {
            Ok(al) => return Ok(al.default_library),
            Err(_) => error!("Invalid Default app library file"),
        }
        Err(RippleError::BootstrapError)
    }
}
