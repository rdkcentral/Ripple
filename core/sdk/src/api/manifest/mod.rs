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

pub mod app_library;
pub mod apps;
pub mod cascaded_device_manifest;
pub mod cascaded_extn_manifest;
pub mod device_manifest;
pub mod exclusory;
pub mod extn_manifest;
pub mod persistent_store;
pub mod remote_feature;
pub mod ripple_manifest_loader;

pub trait MergeConfig<T> {
    fn merge_config(&mut self, t: T);
}
