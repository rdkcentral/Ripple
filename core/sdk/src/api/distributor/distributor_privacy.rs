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

use crate::api::usergrant_entry::UserGrantInfo;
use crate::{
    api::{
        session::AccountSession,
        storage_property::{StorageAdjective, StorageProperty},
    },
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest, ExtnResponse},
    framework::ripple_contract::RippleContract,
};

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
}

impl Default for PrivacySettings {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum PrivacySettingsStoreRequest {
    GetPrivacySettings(StorageProperty),
    SetPrivacySettings(StorageProperty, bool),
    SetAllPrivacySettings(PrivacySettingsData),
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

impl ExtnPayloadProvider for PrivacySettingsStoreRequest {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Request(ExtnRequest::PrivacySettingsStore(storage_request)) => {
                Some(storage_request)
            }
            _ => None,
        }
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::PrivacySettingsStore(self.clone()))
    }

    fn contract() -> RippleContract {
        RippleContract::Storage(StorageAdjective::PrivacyLocal)
    }
}

impl ExtnPayloadProvider for PrivacySettings {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Response(ExtnResponse::Value(v)) = payload {
            if let Ok(v) = serde_json::from_value(v) {
                return Some(v);
            }
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::Value(
            serde_json::to_value(self.clone()).unwrap(),
        ))
    }

    fn contract() -> crate::framework::ripple_contract::RippleContract {
        RippleContract::Storage(StorageAdjective::PrivacyCloud)
    }
}

impl ExtnPayloadProvider for PrivacySettingsData {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::PrivacySettingsStore(
            PrivacySettingsStoreRequest::SetAllPrivacySettings(settings),
        )) = payload
        {
            return Some(settings);
        }
        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::PrivacySettingsStore(
            PrivacySettingsStoreRequest::SetAllPrivacySettings(self.clone()),
        ))
    }

    fn contract() -> crate::framework::ripple_contract::RippleContract {
        RippleContract::Storage(StorageAdjective::PrivacyLocal)
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

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum PrivacyCloudRequest {
    GetProperty(GetPropertyParams),
    GetProperties(AccountSession),
    SetProperty(SetPropertyParams),
    GetPartnerExclusions(AccountSession),
}

impl PrivacyCloudRequest {
    pub fn get_session(&self) -> AccountSession {
        match self {
            PrivacyCloudRequest::GetProperty(params) => params.dist_session.clone(),
            PrivacyCloudRequest::GetProperties(session) => session.clone(),
            PrivacyCloudRequest::SetProperty(params) => params.dist_session.clone(),
            PrivacyCloudRequest::GetPartnerExclusions(session) => session.clone(),
        }
    }
}

impl ExtnPayloadProvider for PrivacyCloudRequest {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::PrivacySettings(p)) = payload {
            return Some(p);
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::PrivacySettings(self.clone()))
    }

    fn contract() -> crate::framework::ripple_contract::RippleContract {
        RippleContract::Storage(StorageAdjective::PrivacyCloud)
    }
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

impl ExtnPayloadProvider for ExclusionPolicy {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Response(ExtnResponse::Value(v)) = payload {
            if let Ok(v) = serde_json::from_value(v) {
                return Some(v);
            }
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::Value(
            serde_json::to_value(self.clone()).unwrap(),
        ))
    }

    fn contract() -> crate::framework::ripple_contract::RippleContract {
        RippleContract::Storage(StorageAdjective::PrivacyCloud)
    }
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum PrivacyResponse {
    None,
    Bool(bool),
    Settings(PrivacySettings),
    Exclusions(ExclusionPolicy),
    Grants(UserGrants),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils::test_extn_payload_provider;

    #[test]
    fn test_get_session_get_property() {
        let params = GetPropertyParams {
            setting: PrivacySetting::AppDataCollection("test_app_data_collection".to_string()),
            dist_session: AccountSession {
                id: "test_session_id".to_string(),
                token: "test_token".to_string(),
                account_id: "test_account_id".to_string(),
                device_id: "test_device_id".to_string(),
            },
        };
        let request = PrivacyCloudRequest::GetProperty(params.clone());
        let session = request.get_session();
        assert_eq!(session, params.dist_session);
    }

    #[test]
    fn test_get_session_get_properties() {
        let session = AccountSession {
            id: "test_session_id".to_string(),
            token: "test_token".to_string(),
            account_id: "test_account_id".to_string(),
            device_id: "test_device_id".to_string(),
        };
        let request = PrivacyCloudRequest::GetProperties(session.clone());
        let result = request.get_session();
        assert_eq!(result, session);
    }

    #[test]
    fn test_get_session_set_property() {
        let params = SetPropertyParams {
            setting: PrivacySetting::AppDataCollection("test_app_data_collection".to_string()),
            value: true,
            dist_session: AccountSession {
                id: "test_session_id".to_string(),
                token: "test_token".to_string(),
                account_id: "test_account_id".to_string(),
                device_id: "test_device_id".to_string(),
            },
        };
        let request = PrivacyCloudRequest::SetProperty(params.clone());
        let session = request.get_session();
        assert_eq!(session, params.dist_session);
    }

    #[test]
    fn test_get_session_get_partner_exclusions() {
        let session = AccountSession {
            id: "test_session_id".to_string(),
            token: "test_token".to_string(),
            account_id: "test_account_id".to_string(),
            device_id: "test_device_id".to_string(),
        };

        let request = PrivacyCloudRequest::GetPartnerExclusions(session.clone());
        let result = request.get_session();
        assert_eq!(result, session);
    }

    #[test]
    fn test_extn_request_privacy_cloud() {
        let get_property_params = GetPropertyParams {
            setting: PrivacySetting::AppDataCollection("test_app_data_collection".to_string()),
            dist_session: AccountSession {
                id: "test_session_id".to_string(),
                token: "test_token".to_string(),
                account_id: "test_account_id".to_string(),
                device_id: "test_device_id".to_string(),
            },
        };

        let privacy_cloud_request = PrivacyCloudRequest::GetProperty(get_property_params);
        let contract_type: RippleContract = RippleContract::Storage(StorageAdjective::PrivacyCloud);

        test_extn_payload_provider(privacy_cloud_request, contract_type);
    }

    #[test]
    fn test_extn_request_privacy_settings_store() {
        let storage_property = StorageProperty::ClosedCaptionsEnabled;
        let privacy_settings_request =
            PrivacySettingsStoreRequest::GetPrivacySettings(storage_property);
        let contract_type: RippleContract = RippleContract::Storage(StorageAdjective::PrivacyLocal);
        test_extn_payload_provider(privacy_settings_request, contract_type);
    }

    #[test]
    fn test_extn_payload_provider_for_exclusion_policy() {
        let exclusion_policy = ExclusionPolicy {
            acr: Some(ExclusionPolicyData {
                data_events: vec![DataEventType::Watched, DataEventType::BusinessIntelligence],
                entity_reference: vec![String::from("entity_reference_acr")],
                derivative_propagation: true,
            }),
            app_content_ad_targeting: Some(ExclusionPolicyData {
                data_events: vec![DataEventType::Watched],
                entity_reference: vec![String::from("entity_reference_app_content_ad_targeting")],
                derivative_propagation: false,
            }),
            business_analytics: Some(ExclusionPolicyData {
                data_events: vec![DataEventType::Watched, DataEventType::BusinessIntelligence],
                entity_reference: vec![String::from("entity_reference_business_analytics")],
                derivative_propagation: false,
            }),
            camera_analytics: Some(ExclusionPolicyData {
                data_events: vec![DataEventType::Watched],
                entity_reference: vec![String::from("entity_reference_camera_analytics")],
                derivative_propagation: true,
            }),
            continue_watching: Some(ExclusionPolicyData {
                data_events: vec![DataEventType::Watched],
                entity_reference: vec![String::from("entity_reference_continue_watching")],
                derivative_propagation: true,
            }),
            personalization: Some(ExclusionPolicyData {
                data_events: vec![DataEventType::Watched],
                entity_reference: vec![String::from("entity_reference_personalization")],
                derivative_propagation: true,
            }),
            primary_browse_ad_targeting: Some(ExclusionPolicyData {
                data_events: vec![DataEventType::Watched],
                entity_reference: vec![String::from(
                    "entity_reference_primary_browse_ad_targeting",
                )],
                derivative_propagation: true,
            }),
            primary_content_ad_targeting: Some(ExclusionPolicyData {
                data_events: vec![DataEventType::Watched],
                entity_reference: vec![String::from(
                    "entity_reference_primary_content_ad_targeting",
                )],
                derivative_propagation: true,
            }),
            product_analytics: Some(ExclusionPolicyData {
                data_events: vec![DataEventType::Watched],
                entity_reference: vec![String::from("entity_reference_product_analytics")],
                derivative_propagation: true,
            }),
            remote_diagnostics: Some(ExclusionPolicyData {
                data_events: vec![DataEventType::Watched],
                entity_reference: vec![String::from("entity_reference_remote_diagnostics")],
                derivative_propagation: true,
            }),
            unentitled_continue_watching: Some(ExclusionPolicyData {
                data_events: vec![DataEventType::Watched],
                entity_reference: vec![String::from(
                    "entity_reference_unentitled_continue_watching",
                )],
                derivative_propagation: true,
            }),
            unentitled_personalization: Some(ExclusionPolicyData {
                data_events: vec![DataEventType::Watched],
                entity_reference: vec![String::from("entity_reference_unentitled_personalization")],
                derivative_propagation: true,
            }),
            watch_history: Some(ExclusionPolicyData {
                data_events: vec![DataEventType::Watched],
                entity_reference: vec![String::from("entity_reference_watch_history")],
                derivative_propagation: true,
            }),
        };

        let contract_type: RippleContract = RippleContract::Storage(StorageAdjective::PrivacyCloud);
        test_extn_payload_provider(exclusion_policy, contract_type);
    }

    #[test]
    fn test_extn_payload_provider_for_privacy_settings() {
        let privacy_settings = PrivacySettings {
            allow_acr_collection: true,
            allow_resume_points: true,
            allow_app_content_ad_targeting: true,
            allow_business_analytics: true,
            allow_camera_analytics: true,
            allow_personalization: true,
            allow_primary_browse_ad_targeting: true,
            allow_primary_content_ad_targeting: true,
            allow_product_analytics: true,
            allow_remote_diagnostics: true,
            allow_unentitled_personalization: true,
            allow_unentitled_resume_points: true,
            allow_watch_history: true,
        };

        let contract_type: RippleContract = RippleContract::Storage(StorageAdjective::PrivacyCloud);
        test_extn_payload_provider(privacy_settings, contract_type);
    }

    #[test]
    fn test_extn_payload_provider_for_privacy_settings_data() {
        let privacy_settings_data = PrivacySettingsData {
            allow_acr_collection: Some(true),
            allow_resume_points: Some(true),
            allow_app_content_ad_targeting: Some(true),
            allow_business_analytics: Some(true),
            allow_camera_analytics: Some(true),
            allow_personalization: Some(true),
            allow_primary_browse_ad_targeting: Some(true),
            allow_primary_content_ad_targeting: Some(true),
            allow_product_analytics: Some(true),
            allow_remote_diagnostics: Some(true),
            allow_unentitled_personalization: Some(true),
            allow_unentitled_resume_points: Some(true),
            allow_watch_history: Some(true),
        };

        let contract_type: RippleContract = RippleContract::Storage(StorageAdjective::PrivacyLocal);
        test_extn_payload_provider(privacy_settings_data, contract_type);
    }
}
