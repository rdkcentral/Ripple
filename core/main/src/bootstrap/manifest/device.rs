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
    api::manifest::device_manifest::DeviceManifest, log::info, utils::error::RippleError,
};

pub struct LoadDeviceManifestStep;

impl LoadDeviceManifestStep {
    pub fn get_manifest() -> DeviceManifest {
        let r = try_manifest_files();
        if let Ok(r) = r {
            return r;
        }

        r.expect("Need valid Device Manifest")
    }
}

type DeviceManifestLoader = Vec<fn() -> Result<(String, DeviceManifest), RippleError>>;

fn try_manifest_files() -> Result<DeviceManifest, RippleError> {
    let dm_arr: DeviceManifestLoader = if cfg!(feature = "local_dev") {
        vec![load_from_env, load_from_home]
    } else if cfg!(test) {
        vec![load_from_env]
    } else {
        vec![load_from_etc]
    };

    for dm_provider in dm_arr {
        if let Ok((p, m)) = dm_provider() {
            info!("loaded_manifest_file_content={}", p);
            return Ok(m);
        }
    }
    Err(RippleError::BootstrapError)
}

fn load_from_env() -> Result<(String, DeviceManifest), RippleError> {
    let device_manifest_path = std::env::var("DEVICE_MANIFEST");
    match device_manifest_path {
        Ok(path) => DeviceManifest::load(path),
        Err(_) => Err(RippleError::MissingInput),
    }
}

fn load_from_home() -> Result<(String, DeviceManifest), RippleError> {
    match std::env::var("HOME") {
        Ok(home) => DeviceManifest::load(format!("{}/.ripple/firebolt-device-manifest.json", home)),
        Err(_) => Err(RippleError::MissingInput),
    }
}

fn load_from_etc() -> Result<(String, DeviceManifest), RippleError> {
    DeviceManifest::load("/etc/firebolt-device-manifest.json".into())
}
