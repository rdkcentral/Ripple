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

use std::ffi::OsStr;

use ripple_sdk::{
    api::manifest::extn_manifest::ExtnManifestEntry,
    async_trait::async_trait,
    extn::ffi::ffi_library::load_extn_library_metadata,
    framework::bootstrap::Bootstep,
    libloading::Library,
    log::{debug, info, warn},
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
        if let Some(metadata) = metadata {
            return Some(LoadedLibrary::new(library, metadata, entry));
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
                        info!("Adding {}", loaded_extn.metadata.symbols.len());
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
                warn!("No valid extensions");
            }
            info!("Total Libraries loaded={}", valid_extension_libraries);
        }
        Ok(())
    }
}
