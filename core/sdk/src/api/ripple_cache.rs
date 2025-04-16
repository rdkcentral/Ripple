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

use log::debug;

use crate::api::{
    distributor::distributor_privacy::PrivacySettingsData, storage_property::StorageProperty,
};

use super::{distributor::distributor_privacy::PrivacySettings, storage_manager::IStorageOperator};

#[derive(Debug, Clone, Default)]
pub struct RippleCache {
    // Cache for privacy settings
    privacy_settings_cache: Arc<RwLock<PrivacySettingsData>>,
    // Add more caches for other settings as required
}

impl RippleCache {
    pub fn get(&self) -> PrivacySettingsData {
        self.privacy_settings_cache.read().unwrap().clone()
    }

    pub fn get_cached_bool_storage_property(&self, property: &StorageProperty) -> Option<bool> {
        if property.is_a_privacy_setting_property() {
            // check if the privacy setting property is available in cache
            let cache = self.privacy_settings_cache.read().unwrap();
            property.get_privacy_setting_value(&cache)
        } else {
            // We can add caching support for non-privacy setting properties in future
            None
        }
    }

    pub fn update_cached_bool_storage_property(
        &self,
        state: &impl IStorageOperator,
        property: &StorageProperty,
        value: bool,
    ) {
        {
            // update the privacy setting property in cache
            debug!(
                "Updating cache for storage property privacy setting {:?} and value {}",
                property, value
            );
            {
                let mut cache = self.privacy_settings_cache.write().unwrap();
                property.set_privacy_setting_value(&mut cache, value);
            }
            state.on_privacy_updated(self.clone());
        }
    }

    pub fn update_all_privacy_cache(&self, data: &PrivacySettings) {
        debug!("Updating all privacy cache {:?}", data);
        let mut cache = self.privacy_settings_cache.write().unwrap();
        cache.update(data);
    }
}
