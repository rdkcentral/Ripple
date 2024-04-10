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

use ripple_sdk::{
    api::manifest::extn_manifest::ExtnManifest, log::info, utils::error::RippleError,
};

pub struct LoadExtnManifestStep;

impl LoadExtnManifestStep {
    pub fn get_manifest() -> ExtnManifest {
        try_manifest_files().expect("Need valid extn manifest")
    }
}

type ExtnManifestLoader = Vec<fn() -> Result<(String, ExtnManifest), RippleError>>;

fn try_manifest_files() -> Result<ExtnManifest, RippleError> {
    let dm_arr: ExtnManifestLoader = if cfg!(feature = "local_dev") {
        vec![load_from_env, load_from_home]
    } else if cfg!(test) {
        vec![load_from_env]
    } else {
        vec![load_from_etc]
    };

    for dm_provider in dm_arr {
        if let Ok((p, m)) = dm_provider() {
            info!("loaded_extn_file_content={}", p);
            return Ok(m);
        }
    }
    Err(RippleError::BootstrapError)
}

fn load_from_env() -> Result<(String, ExtnManifest), RippleError> {
    let device_manifest_path = std::env::var("EXTN_MANIFEST");
    match device_manifest_path {
        Ok(path) => ExtnManifest::load(path),
        Err(_) => Err(RippleError::MissingInput),
    }
}

fn load_from_home() -> Result<(String, ExtnManifest), RippleError> {
    match std::env::var("HOME") {
        Ok(home) => ExtnManifest::load(format!("{}/.ripple/firebolt-extn-manifest.json", home)),
        Err(_) => Err(RippleError::MissingInput),
    }
}

fn load_from_etc() -> Result<(String, ExtnManifest), RippleError> {
    ExtnManifest::load("/etc/firebolt-extn-manifest.json".into())
}
