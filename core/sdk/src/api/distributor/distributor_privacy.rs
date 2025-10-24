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

use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::api::session::AccountSession;
use crate::api::usergrant_entry::UserGrantInfo;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum PrivacySetting {
    AppDataCollection(String),
    AppEntitlementCollection(String),
    BusinessAnalytics,
    ContinueWatching,
    UnentitledContinueWatching,
    WatchHistory,
    ProductAnalytics,
    Personalization,
    UnentitledPersonalization,
    RemoteDiagnostics,
    PrimaryContentAdTargeting,
    PrimaryBrowseAdTargeting,
    AppContentAdTargeting,
    Acr,
    CameraAnalytics,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacySettings {
    #[serde(rename = "allowACRCollection")]
    pub allow_acr_collection: bool,
    pub allow_resume_points: bool,
    pub allow_app_content_ad_targeting: bool,
    pub allow_business_analytics: bool,
    pub allow_camera_analytics: bool,
    pub allow_personalization: bool,
    pub allow_primary_browse_ad_targeting: bool,
    pub allow_primary_content_ad_targeting: bool,
    pub allow_product_analytics: bool,
    pub allow_remote_diagnostics: bool,
    pub allow_unentitled_personalization: bool,
    pub allow_unentitled_resume_points: bool,
    pub allow_watch_history: bool,
}

impl PrivacySettings {
    pub fn new() -> Self {
        PrivacySettings {
            allow_acr_collection: false,
            allow_resume_points: false,
            allow_app_content_ad_targeting: false,
            allow_business_analytics: true,
            allow_camera_analytics: false,
            allow_personalization: false,
            allow_primary_browse_ad_targeting: false,
            allow_primary_content_ad_targeting: false,
            allow_product_analytics: false,
            allow_remote_diagnostics: false,
            allow_unentitled_personalization: false,
            allow_unentitled_resume_points: false,
            allow_watch_history: false,
        }
    }

    pub fn update_privacy_setting(&mut self, setting: PrivacySetting, value: bool) {
        match setting {
            PrivacySetting::Acr => self.allow_acr_collection = value,
            PrivacySetting::AppContentAdTargeting => self.allow_app_content_ad_targeting = value,
            PrivacySetting::BusinessAnalytics => self.allow_business_analytics = value,
            PrivacySetting::CameraAnalytics => self.allow_camera_analytics = value,
            PrivacySetting::ContinueWatching => self.allow_resume_points = value,
            PrivacySetting::Personalization => self.allow_personalization = value,
            PrivacySetting::PrimaryBrowseAdTargeting => {
                self.allow_primary_browse_ad_targeting = value
            }
            PrivacySetting::PrimaryContentAdTargeting => {
                self.allow_primary_content_ad_targeting = value
            }
            PrivacySetting::ProductAnalytics => self.allow_product_analytics = value,
            PrivacySetting::RemoteDiagnostics => self.allow_remote_diagnostics = value,
            PrivacySetting::UnentitledContinueWatching => {
                self.allow_unentitled_resume_points = value
            }
            PrivacySetting::UnentitledPersonalization => {
                self.allow_unentitled_personalization = value
            }
            PrivacySetting::WatchHistory => self.allow_watch_history = value,
            _ => {}
        }
    }

    pub fn update(&mut self, data: PrivacySettings) {
        self.allow_acr_collection = data.allow_acr_collection;
        self.allow_resume_points = data.allow_resume_points;

        self.allow_app_content_ad_targeting = data.allow_app_content_ad_targeting;

        self.allow_business_analytics = data.allow_business_analytics;

        self.allow_camera_analytics = data.allow_camera_analytics;

        self.allow_personalization = data.allow_personalization;

        self.allow_primary_browse_ad_targeting = data.allow_primary_browse_ad_targeting;

        self.allow_primary_content_ad_targeting = data.allow_primary_content_ad_targeting;

        self.allow_product_analytics = data.allow_product_analytics;

        self.allow_remote_diagnostics = data.allow_remote_diagnostics;

        self.allow_unentitled_personalization = data.allow_unentitled_personalization;

        self.allow_unentitled_resume_points = data.allow_unentitled_resume_points;

        self.allow_watch_history = data.allow_watch_history;
    }
}

impl Default for PrivacySettings {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default, PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct PrivacySettingsData {
    pub allow_acr_collection: Option<bool>,
    pub allow_resume_points: Option<bool>,
    pub allow_app_content_ad_targeting: Option<bool>,
    pub allow_business_analytics: Option<bool>,
    pub allow_camera_analytics: Option<bool>,
    pub allow_personalization: Option<bool>,
    pub allow_primary_browse_ad_targeting: Option<bool>,
    pub allow_primary_content_ad_targeting: Option<bool>,
    pub allow_product_analytics: Option<bool>,
    pub allow_remote_diagnostics: Option<bool>,
    pub allow_unentitled_personalization: Option<bool>,
    pub allow_unentitled_resume_points: Option<bool>,
    pub allow_watch_history: Option<bool>,
}

impl PrivacySettingsData {
    pub fn update(&mut self, data: &PrivacySettings) {
        self.allow_acr_collection = Some(data.allow_acr_collection);
        self.allow_resume_points = Some(data.allow_resume_points);
        self.allow_app_content_ad_targeting = Some(data.allow_app_content_ad_targeting);
        self.allow_business_analytics = Some(data.allow_business_analytics);
        self.allow_camera_analytics = Some(data.allow_camera_analytics);
        self.allow_personalization = Some(data.allow_personalization);
        self.allow_primary_browse_ad_targeting = Some(data.allow_primary_browse_ad_targeting);
        self.allow_primary_content_ad_targeting = Some(data.allow_primary_content_ad_targeting);
        self.allow_product_analytics = Some(data.allow_product_analytics);
        self.allow_remote_diagnostics = Some(data.allow_remote_diagnostics);
        self.allow_unentitled_personalization = Some(data.allow_unentitled_personalization);
        self.allow_unentitled_resume_points = Some(data.allow_unentitled_resume_points);
        self.allow_watch_history = Some(data.allow_watch_history);
    }
}

impl From<PrivacySettings> for PrivacySettingsData {
    fn from(data: PrivacySettings) -> Self {
        PrivacySettingsData {
            allow_acr_collection: Some(data.allow_acr_collection),
            allow_resume_points: Some(data.allow_resume_points),
            allow_app_content_ad_targeting: Some(data.allow_app_content_ad_targeting),
            allow_business_analytics: Some(data.allow_business_analytics),
            allow_camera_analytics: Some(data.allow_camera_analytics),
            allow_personalization: Some(data.allow_personalization),
            allow_primary_browse_ad_targeting: Some(data.allow_primary_browse_ad_targeting),
            allow_primary_content_ad_targeting: Some(data.allow_primary_content_ad_targeting),
            allow_product_analytics: Some(data.allow_product_analytics),
            allow_remote_diagnostics: Some(data.allow_remote_diagnostics),
            allow_unentitled_personalization: Some(data.allow_unentitled_personalization),
            allow_unentitled_resume_points: Some(data.allow_unentitled_resume_points),
            allow_watch_history: Some(data.allow_watch_history),
        }
    }
}
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContentListenRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    pub listen: bool,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct GetPropertyParams {
    pub setting: PrivacySetting,
    pub dist_session: AccountSession,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct SetPropertyParams {
    pub setting: PrivacySetting,
    pub value: bool,
    pub dist_session: AccountSession,
}

#[derive(Debug, PartialEq, Default, Serialize, Deserialize, Clone)]
pub struct ExclusionPolicyData {
    pub data_events: Vec<DataEventType>,
    pub entity_reference: Vec<String>,
    pub derivative_propagation: bool,
}

#[derive(Debug, PartialEq, Default, Serialize, Deserialize, Clone)]
pub struct ExclusionPolicy {
    pub acr: Option<ExclusionPolicyData>,
    pub app_content_ad_targeting: Option<ExclusionPolicyData>,
    pub business_analytics: Option<ExclusionPolicyData>,
    pub camera_analytics: Option<ExclusionPolicyData>,
    pub continue_watching: Option<ExclusionPolicyData>,
    pub personalization: Option<ExclusionPolicyData>,
    pub primary_browse_ad_targeting: Option<ExclusionPolicyData>,
    pub primary_content_ad_targeting: Option<ExclusionPolicyData>,
    pub product_analytics: Option<ExclusionPolicyData>,
    pub remote_diagnostics: Option<ExclusionPolicyData>,
    pub unentitled_continue_watching: Option<ExclusionPolicyData>,
    pub unentitled_personalization: Option<ExclusionPolicyData>,
    pub watch_history: Option<ExclusionPolicyData>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub enum DataEventType {
    Watched,
    BusinessIntelligence,
    Unknown,
}

impl FromStr for DataEventType {
    type Err = ();
    fn from_str(input: &str) -> Result<DataEventType, Self::Err> {
        match input {
            "Watch_History" => Ok(DataEventType::Watched),
            "Product_Analytics" => Ok(DataEventType::BusinessIntelligence),
            _ => Ok(DataEventType::Unknown),
        }
    }
}

pub struct AppSetting {
    pub app_id: Option<String>,
    pub value: Option<bool>,
}

pub type UserGrants = Vec<UserGrantInfo>;

#[derive(Debug, Deserialize, Clone)]
pub enum PrivacyResponse {
    None,
    Bool(bool),
    Settings(PrivacySettings),
    Grants(UserGrants),
}

#[cfg(test)]
mod tests {
    // All extension payload tests commented out - using direct RPC calls now
    // The core data structures above are tested through RPC integration tests

    // use super::*;
    // use crate::utils::test_utils::test_extn_payload_provider;

    // #[test]
    // fn test_extn_payload_provider_for_exclusion_policy() {
    //     let exclusion_policy = ExclusionPolicy {
    //         acr: Some(ExclusionPolicyData {
    //             data_events: vec![DataEventType::Watched, DataEventType::BusinessIntelligence],
    //             entity_reference: vec![String::from("entity_reference_acr")],
    //             derivative_propagation: true,
    //         }),
    //         app_content_ad_targeting: Some(ExclusionPolicyData {
    //             data_events: vec![DataEventType::Watched],
    //             entity_reference: vec![String::from("entity_reference_app_content_ad_targeting")],
    //             derivative_propagation: false,
    //         }),
    //         business_analytics: Some(ExclusionPolicyData {
    //             data_events: vec![DataEventType::Watched, DataEventType::BusinessIntelligence],
    //             entity_reference: vec![String::from("entity_reference_business_analytics")],
    //             derivative_propagation: false,
    //         }),
    //         camera_analytics: Some(ExclusionPolicyData {
    //             data_events: vec![DataEventType::Watched],
    //             entity_reference: vec![String::from("entity_reference_camera_analytics")],
    //             derivative_propagation: true,
    //         }),
    //         continue_watching: Some(ExclusionPolicyData {
    //             data_events: vec![DataEventType::Watched],
    //             entity_reference: vec![String::from("entity_reference_continue_watching")],
    //             derivative_propagation: true,
    //         }),
    //         personalization: Some(ExclusionPolicyData {
    //             data_events: vec![DataEventType::Watched],
    //             entity_reference: vec![String::from("entity_reference_personalization")],
    //             derivative_propagation: true,
    //         }),
    //         primary_browse_ad_targeting: Some(ExclusionPolicyData {
    //             data_events: vec![DataEventType::Watched],
    //             entity_reference: vec![String::from(
    //                 "entity_reference_primary_browse_ad_targeting",
    //             )],
    //             derivative_propagation: true,
    //         }),
    //         primary_content_ad_targeting: Some(ExclusionPolicyData {
    //             data_events: vec![DataEventType::Watched],
    //             entity_reference: vec![String::from(
    //                 "entity_reference_primary_content_ad_targeting",
    //             )],
    //             derivative_propagation: true,
    //         }),
    //         product_analytics: Some(ExclusionPolicyData {
    //             data_events: vec![DataEventType::Watched],
    //             entity_reference: vec![String::from("entity_reference_product_analytics")],
    //             derivative_propagation: true,
    //         }),
    //         remote_diagnostics: Some(ExclusionPolicyData {
    //             data_events: vec![DataEventType::Watched],
    //             entity_reference: vec![String::from("entity_reference_remote_diagnostics")],
    //             derivative_propagation: true,
    //         }),
    //         unentitled_continue_watching: Some(ExclusionPolicyData {
    //             data_events: vec![DataEventType::Watched],
    //             entity_reference: vec![String::from(
    //                 "entity_reference_unentitled_continue_watching",
    //             )],
    //             derivative_propagation: true,
    //         }),
    //         unentitled_personalization: Some(ExclusionPolicyData {
    //             data_events: vec![DataEventType::Watched],
    //             entity_reference: vec![String::from("entity_reference_unentitled_personalization")],
    //             derivative_propagation: true,
    //         }),
    //         watch_history: Some(ExclusionPolicyData {
    //             data_events: vec![DataEventType::Watched],
    //             entity_reference: vec![String::from("entity_reference_watch_history")],
    //             derivative_propagation: true,
    //         }),
    //     };

    //     let contract_type: RippleContract = RippleContract::Storage(StorageAdjective::PrivacyCloud);
    //     test_extn_payload_provider(exclusion_policy, contract_type);
    // }

    // #[test]
    // fn test_extn_payload_provider_for_privacy_settings() {
    //     let privacy_settings = PrivacySettings {
    //         allow_acr_collection: true,
    //         allow_resume_points: true,
    //         allow_app_content_ad_targeting: true,
    //         allow_business_analytics: true,
    //         allow_camera_analytics: true,
    //         allow_personalization: true,
    //         allow_primary_browse_ad_targeting: true,
    //         allow_primary_content_ad_targeting: true,
    //         allow_product_analytics: true,
    //         allow_remote_diagnostics: true,
    //         allow_unentitled_personalization: true,
    //         allow_unentitled_resume_points: true,
    //         allow_watch_history: true,
    //     };

    //     let contract_type: RippleContract = RippleContract::Storage(StorageAdjective::PrivacyCloud);
    //     test_extn_payload_provider(privacy_settings, contract_type);
    // }

    // #[test]
    // fn test_extn_payload_provider_for_privacy_settings_data() {
    //     let privacy_settings_data = PrivacySettingsData {
    //         allow_acr_collection: Some(true),
    //         allow_resume_points: Some(true),
    //         allow_app_content_ad_targeting: Some(true),
    //         allow_business_analytics: Some(true),
    //         allow_camera_analytics: Some(true),
    //         allow_personalization: Some(true),
    //         allow_primary_browse_ad_targeting: Some(true),
    //         allow_primary_content_ad_targeting: Some(true),
    //         allow_product_analytics: Some(true),
    //         allow_remote_diagnostics: Some(true),
    //         allow_unentitled_personalization: Some(true),
    //         allow_unentitled_resume_points: Some(true),
    //         allow_watch_history: Some(true),
    //     };

    //     let contract_type: RippleContract = RippleContract::Storage(StorageAdjective::PrivacyLocal);
    //     test_extn_payload_provider(privacy_settings_data, contract_type);
    // }
}
