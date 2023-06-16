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


use std::{fs, path::Path};

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
        if let Some(p) = Path::new(&path).to_str() {
            let result = match fs::read_to_string(&p) {
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
            return Ok(result.expect("Need valid App Library"));
        }
        Err(RippleError::BootstrapError)
    }
}
