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
    pub fn load_app_library() -> Vec<AppLibraryEntry> {
        let r = try_app_library_files();
        if let Ok(r) = r {
            return r;
        }

        r.expect("Need valid App Library")
    }
}

type AppLibraryLoader = Vec<fn() -> Result<(String, Vec<AppLibraryEntry>), RippleError>>;

fn try_app_library_files() -> Result<Vec<AppLibraryEntry>, RippleError> {
    let al_arr: AppLibraryLoader = if cfg!(feature = "local_dev") {
        vec![load_from_env, load_from_home, load_from_etc]
    } else if cfg!(test) {
        vec![load_from_env]
    } else {
        vec![load_from_etc]
    };

    for al_provider in al_arr {
        if let Ok((p, al)) = al_provider() {
            info!("loaded_app_library_file_content={}", p);
            return Ok(al);
        }
    }
    // Return an empty Vec<AppLibraryEntry>
    Ok(Vec::new())
}

fn load_from_env() -> Result<(String, Vec<AppLibraryEntry>), RippleError> {
    match std::env::var("APP_LIBRARY") {
        Ok(path) => load(path),
        Err(_) => Err(RippleError::MissingInput),
    }
}

fn load_from_home() -> Result<(String, Vec<AppLibraryEntry>), RippleError> {
    match std::env::var("HOME") {
        Ok(home) => load(format!("{}/.ripple/firebolt-app-library.json", home)),
        Err(_) => Err(RippleError::MissingInput),
    }
}

fn load_from_etc() -> Result<(String, Vec<AppLibraryEntry>), RippleError> {
    load("/etc/firebolt-app-library.json".into())
}

fn load(path: String) -> Result<(String, Vec<AppLibraryEntry>), RippleError> {
    info!("Trying to load app library from {}", path);
    let p = Path::new(&path);
    if let Some(p_str) = p.to_str() {
        match fs::read_to_string(p_str) {
            Ok(contents) => match serde_json::from_str::<DefaultLibrary>(&contents) {
                Ok(al) => Ok((path, al.default_library)),
                Err(_) => {
                    warn!("Could not parse app library from path {}", path);
                    Err(RippleError::InvalidInput)
                }
            },
            Err(e) => {
                info!("Error reading file: {}", e);
                Err(RippleError::MissingInput)
            }
        }
    } else {
        Err(RippleError::ParseError)
    }
}
