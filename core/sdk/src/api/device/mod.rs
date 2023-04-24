// If not stated otherwise in this file or this component's license file the
// following copyright and licenses apply:
//
// Copyright 2023 RDK Management
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
use serde::Deserialize;

pub mod device_accessibility_data;
pub mod device_accessory;
pub mod device_browser;
pub mod device_info_request;
pub mod device_operator;
pub mod device_request;
pub mod device_storage;
pub mod device_wifi;
pub mod device_window_manager;

/// Contains a list of available Platformtypes supported by Ripple.
#[derive(Debug, Deserialize, Clone)]
pub enum DevicePlatformType {
    /// Device platform which uses JsonRpc based websocket to perform device operations. Mored details can be found in <https://github.com/rdkcentral/Thunder>.
    Thunder,
}

impl ToString for DevicePlatformType {
    fn to_string(&self) -> String {
        match self {
            DevicePlatformType::Thunder => "thunder".into(),
        }
    }
}
