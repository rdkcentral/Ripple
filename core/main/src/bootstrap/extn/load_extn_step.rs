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

use std::thread;

use ripple_sdk::{
    api::manifest::extn_manifest::ExtnManifestEntry,
    async_trait::async_trait,
    extn::ffi::ffi_channel::load_channel_builder,
    framework::bootstrap::Bootstep,
    log::{debug, info, warn},
    utils::error::RippleError,
};

use crate::state::bootstrap_state::BootstrapState;
use ripple_sdk::libloading::Library;
use std::ffi::OsStr;

#[derive(Debug)]
pub struct LoadedLibrary {
    pub library: Library,
    pub entry: ExtnManifestEntry,
}

impl LoadedLibrary {
    pub fn new(library: Library, entry: ExtnManifestEntry) -> LoadedLibrary {
        LoadedLibrary { library, entry }
    }
}

/// Actual bootstep which loads the extensions into the ExtnState.
/// Currently this step loads
/// 1. Device Channel
/// 2. Device Extensions
pub struct LoadExtensionsStep;

impl LoadExtensionsStep {
    unsafe fn load_extension_library<P: AsRef<OsStr>>(
        filename: P,
        entry: ExtnManifestEntry,
    ) -> Option<LoadedLibrary> {
        match Library::new(&filename) {
            Ok(library) => Some(LoadedLibrary::new(library, entry)),
            Err(err) => {
                debug!("Extn not found: {:?}", err);
                None
            }
        }
    }

    async fn pre_setup(&self, state: BootstrapState) -> Result<Vec<LoadedLibrary>, RippleError> {
        debug!("Starting Extension Library step");
        let manifest = state.platform_state.get_manifest();
        let default_path = manifest.default_path;
        let default_extn = manifest.default_extension;
        let mut extn_paths: Vec<(String, ExtnManifestEntry)> = manifest
            .extns
            .into_iter()
            .map(|f| {
                (f.get_path(&default_path, &default_extn), f)
                // TODO Add Resolution checks later on
            })
            .collect();
        extn_paths.shrink_to_fit();
        let mut loaded_extns = Vec::new();
        unsafe {
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
                        info!("Adding {}", loaded_extn.entry.path);
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
        loaded_extns.shrink_to_fit();
        Ok(loaded_extns)
    }
}

#[async_trait]
impl Bootstep<BootstrapState> for LoadExtensionsStep {
    fn get_name(&self) -> String {
        "LoadExtensionsStep".into()
    }
    async fn setup(&self, state: BootstrapState) -> Result<(), RippleError> {
        let loaded_extensions = self.pre_setup(state.clone()).await?;
        for extn in loaded_extensions.iter() {
            unsafe {
                let path = &extn.entry.path;
                let library = &extn.library;
                info!("Starting library at  path {}", path);
                if let Ok(builder) = load_channel_builder(library) {
                    thread::spawn(move || {
                        (builder.start)();
                    });
                }
            }
        }

        Ok(())
    }
}
