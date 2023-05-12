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

use std::collections::HashMap;

use crate::{
    processor::storage::{
        storage_manager::{StorageManager, StorageManagerError},
        storage_property::StorageProperty,
    },
    state::platform_state::PlatformState,
};

pub const US_PRIVACY_KEY: &'static str = "us_privacy";
pub const LMT_KEY: &'static str = "lmt";

#[derive(Debug, Clone)]
struct AllowAppContentAdTargetingSettings {
    lmt: String,
    us_privacy: String,
}

impl AllowAppContentAdTargetingSettings {
    pub fn new(limit_ad_targeting: bool) -> Self {
        let (lmt, us_privacy) = match limit_ad_targeting {
            true => ("1", "1-Y-"),
            false => ("0", "1-N-"),
        };
        AllowAppContentAdTargetingSettings {
            lmt: lmt.to_owned(),
            us_privacy: us_privacy.to_owned(),
        }
    }

    pub fn get_allow_app_content_ad_targeting_settings(&self) -> HashMap<String, String> {
        HashMap::from([
            (US_PRIVACY_KEY.to_owned(), self.us_privacy.to_owned()),
            (LMT_KEY.to_owned(), self.lmt.to_owned()),
        ])
    }
}
impl Default for AllowAppContentAdTargetingSettings {
    fn default() -> Self {
        Self {
            /*
            As per X1 privacy settings documentation, default privacy setting is lmt = 0 us_privacy = 1-N-
            (i.e. , Customer has not opted-out)
            https://developer.comcast.com/documentation/limit-ad-tracking-and-ccpa-technical-requirements-1
            */
            lmt: "0".to_owned(),
            us_privacy: "1-N-".to_owned(),
        }
    }
}

pub async fn get_allow_app_content_ad_targeting_settings(
    platform_state: &PlatformState,
) -> HashMap<String, String> {
    let data = StorageProperty::AllowAppContentAdTargeting.as_data();

    match StorageManager::get_bool_from_namespace(
        platform_state,
        data.namespace.to_string(),
        data.key,
    )
    .await
    {
        Ok(resp) => AllowAppContentAdTargetingSettings::new(resp.as_value())
            .get_allow_app_content_ad_targeting_settings(),
        Err(StorageManagerError::NotFound) => AllowAppContentAdTargetingSettings::default()
            .get_allow_app_content_ad_targeting_settings(),
        _ => AllowAppContentAdTargetingSettings::new(true)
            .get_allow_app_content_ad_targeting_settings(),
    }
}

pub struct PrivacyImpl {
    pub platform_state: PlatformState,
}

impl PrivacyImpl {
    pub async fn get_limit_ad_tracking(state: &PlatformState) -> bool {
        match StorageManager::get_bool(state, StorageProperty::LimitAdTracking).await {
            Ok(resp) => resp,
            Err(_) => true,
        }
    }
}
