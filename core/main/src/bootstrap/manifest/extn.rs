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
pub struct ExtnManifestLoadResult {
    pub extn_manifest: ExtnManifest,
    pub load_path: String,
}

fn try_manifest_files() -> Result<ExtnManifest, RippleError> {
    let path = resolve_manifest_file_path()?;
    load_manifest(path)
}
fn load_manifest(path: String) -> Result<ExtnManifest, RippleError> {
    match ExtnManifest::load(path) {
        Ok((p, m)) => {
            info!("loaded_extn_file_content={}", p);
            Ok(m)
        }
        Err(e) => Err(e),
    }
}

pub fn resolve_manifest_file_path() -> Result<String, RippleError> {
    if cfg!(any(feature = "local_dev", feature = "pre_prod")) {
        path_from_env().or(path_from_home().or(path_from_opt().or(path_from_etc())))
    } else if cfg!(test) {
        path_from_env()
    } else {
        path_from_etc()
    }
}
fn path_from_env() -> Result<String, RippleError> {
    let device_manifest_path = std::env::var("EXTN_MANIFEST");
    match device_manifest_path {
        Ok(path) => Ok(path),
        Err(_) => Err(RippleError::MissingInput),
    }
}
fn path_from_home() -> Result<String, RippleError> {
    match std::env::var("HOME") {
        Ok(home) => Ok(format!("{}/.ripple/firebolt-extn-manifest.json", home)),
        Err(_) => Err(RippleError::MissingInput),
    }
}
fn path_from_opt() -> Result<String, RippleError> {
    Ok("/opt/firebolt-extn-manifest.json".into())
}
fn path_from_etc() -> Result<String, RippleError> {
    Ok("/etc/firebolt-extn-manifest.json".into())
}
