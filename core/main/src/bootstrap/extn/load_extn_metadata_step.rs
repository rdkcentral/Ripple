use std::ffi::OsStr;

use ripple_sdk::{
    api::manifest::extn_manifest::ExtnManifestEntry,
    async_trait::async_trait,
    extn::ffi::ffi_library::load_extn_library_metadata,
    framework::bootstrap::Bootstep,
    libloading::Library,
    log::{debug, error, info, warn},
    utils::error::RippleError,
};

use crate::state::{bootstrap_state::BootstrapState, extn_state::LoadedLibrary};

/// Loads the metadata from the Extensions shared libraries. This first step helps ripple catalogue the contents of a given dynamic linked file.
/// This step will be used for resolutions and also setting up permissions in future.
pub struct LoadExtensionMetadataStep;

impl LoadExtensionMetadataStep {
    unsafe fn load_extension_library<P: AsRef<OsStr>>(
        filename: P,
        entry: ExtnManifestEntry,
    ) -> Option<LoadedLibrary> {
        let r = Library::new(filename.as_ref());
        if r.is_err() {
            debug!("Extn not found");
            return None;
        }

        let library = r.unwrap();
        let metadata = load_extn_library_metadata(&library);
        if metadata.is_some() {
            return Some(LoadedLibrary::new(library, metadata.unwrap(), entry));
        }
        None
    }
}

#[async_trait]
impl Bootstep<BootstrapState> for LoadExtensionMetadataStep {
    fn get_name(&self) -> String {
        "LoadExtensionMetadataStep".into()
    }

    async fn setup(&self, state: BootstrapState) -> Result<(), RippleError> {
        debug!("Starting Extension Library step");
        let manifest = state.platform_state.get_manifest();
        let default_path = manifest.default_path.clone();
        let default_extn = manifest.default_extension.clone();
        let extn_paths: Vec<(String, ExtnManifestEntry)> = manifest
            .extns
            .into_iter()
            .map(|f| {
                (f.get_path(&default_path, &default_extn), f)
                // TODO Add Resolution checks later on
            })
            .collect();
        unsafe {
            let mut loaded_extns = state.extn_state.loaded_libraries.write().unwrap();
            for (extn_path, entry) in extn_paths {
                debug!("");
                debug!("");
                debug!(
                    "******************Loading {}************************",
                    extn_path
                );
                let r = Self::load_extension_library(extn_path.clone(), entry);
                match r {
                    Some(loaded_extn) => {
                        loaded_extns.push(loaded_extn);
                    }
                    None => warn!(
                        "file={} doesnt contain a valid extension library",
                        extn_path
                    ),
                }
                debug!("-------------------------------------------------");
                debug!("");
                debug!("");
            }
            let valid_extension_libraries = loaded_extns.len();
            if valid_extension_libraries == 0 {
                error!("No valid extensions");
                return Err(RippleError::ExtnError);
            }
            info!("Total Libraries loaded={}", valid_extension_libraries);
        }
        Ok(())
    }
}
