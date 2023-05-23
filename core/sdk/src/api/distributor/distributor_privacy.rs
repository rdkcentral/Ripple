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

use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::{
    api::session::AccountSession,
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest, ExtnResponse},
    framework::ripple_contract::RippleContract,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum PrivacySetting {
    AppDataCollection(String),
    AppEntitlementCollection(String),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacySettings {
    #[serde(rename = "allowACRCollection")]
    pub allow_acr_collection: bool,
    pub allow_resume_points: bool,
    pub allow_app_content_ad_targeting: bool,
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

impl ExtnPayloadProvider for PrivacySettings {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Response(r) => match r {
                ExtnResponse::Value(v) => {
                    if let Ok(v) = serde_json::from_value(v) {
                        return Some(v);
                    }
                }
                _ => {}
            },
            _ => {}
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::Value(
            serde_json::to_value(self.clone()).unwrap(),
        ))
    }

    fn contract() -> crate::framework::ripple_contract::RippleContract {
        RippleContract::PrivacySettings
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContentListenRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    pub listen: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetPropertyParams {
    pub setting: PrivacySetting,
    pub dist_session: AccountSession,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetPropertyParams {
    pub setting: PrivacySetting,
    pub value: bool,
    pub dist_session: AccountSession,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrivacyRequest {
    GetProperty(GetPropertyParams),
    GetProperties(AccountSession),
    SetProperty(SetPropertyParams),
    GetPartnerExclusions(AccountSession),
}

impl PrivacyRequest {
    pub fn get_session(&self) -> AccountSession {
        match self {
            PrivacyRequest::GetProperty(params) => params.dist_session.clone(),
            PrivacyRequest::GetProperties(session) => session.clone(),
            PrivacyRequest::SetProperty(params) => params.dist_session.clone(),
            PrivacyRequest::GetPartnerExclusions(session) => session.clone(),
        }
    }
}

impl ExtnPayloadProvider for PrivacyRequest {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Request(r) => match r {
                ExtnRequest::PrivacySettings(p) => return Some(p),
                _ => {}
            },
            _ => {}
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::PrivacySettings(self.clone()))
    }

    fn contract() -> crate::framework::ripple_contract::RippleContract {
        RippleContract::PrivacySettings
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct ExclusionPolicyData {
    pub data_events: Vec<DataEventType>,
    pub entity_reference: Vec<String>,
    pub derivative_propagation: bool,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
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
        match payload {
            ExtnPayload::Response(r) => match r {
                ExtnResponse::Value(v) => {
                    if let Ok(v) = serde_json::from_value(v) {
                        return Some(v);
                    }
                }
                _ => {}
            },
            _ => {}
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::Value(
            serde_json::to_value(self.clone()).unwrap(),
        ))
    }

    fn contract() -> crate::framework::ripple_contract::RippleContract {
        RippleContract::PrivacySettings
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub enum DataEventType {
    Watched,
    BusinessIntelligence,
}

impl FromStr for DataEventType {
    type Err = ();
    fn from_str(input: &str) -> Result<DataEventType, Self::Err> {
        match input {
            "Watch_History" => Ok(DataEventType::Watched),
            _ => Err(()),
        }
    }
}
