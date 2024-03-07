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
use std::sync::{Arc, RwLock};

use ripple_sdk::api::distributor::distributor_privacy::PrivacySettingsData;

#[derive(Debug, Clone, Default)]
pub struct RippleCache {
    pub privacy_settings_cache: Arc<RwLock<PrivacySettingsData>>,
}

impl RippleCache {
    pub fn get_privacy_settings_cache(&self) -> PrivacySettingsData {
        self.privacy_settings_cache.read().unwrap().clone()
    }
    pub fn update_privacy_settings_cache(&self, value: &PrivacySettingsData) {
        let mut cache = self.privacy_settings_cache.write().unwrap();
        *cache = value.clone();
    }
}
