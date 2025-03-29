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

use ripple_sdk::api::manifest::device_manifest::{try_manifest_files, DeviceManifest};

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
