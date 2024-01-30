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

use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt};

use crate::{
    api::session::AccountSession,
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest, ExtnResponse},
    framework::ripple_contract::RippleContract,
};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum AdvertisingRequest {
    GetAdInitObject(AdInitObjectRequestParams),
    GetAdIdObject(AdIdRequestParams),
    ResetAdIdentifier(AccountSession),
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct AdInitObjectRequestParams {
    pub privacy_data: HashMap<String, String>,
    pub environment: String,
    pub durable_app_id: String,
    pub app_version: String,
    pub distributor_app_id: String,
    pub device_ad_attributes: HashMap<String, String>,
    pub coppa: bool,
    pub authentication_entity: String,
    pub dist_session: AccountSession,
    pub scope: HashMap<String, String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct AdInitObjectResponse {
    pub ad_server_url: String,
    pub ad_server_url_template: String,
    pub ad_network_id: String,
    pub ad_profile_id: String,
    pub ad_site_section_id: String,
    pub ad_opt_out: bool,
    pub privacy_data: String,
    pub ifa_value: String,
    pub ifa: String,
    pub app_name: String,
    pub app_bundle_id: String,
    pub app_version: String,
    pub distributor_app_id: String,
    pub device_ad_attributes: String,
    pub coppa: String,
    pub authentication_entity: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct AdIdRequestParams {
    pub privacy_data: HashMap<String, String>,
    pub app_id: String,
    pub dist_session: AccountSession,
    pub scope: HashMap<String, String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct AdIdResponse {
    pub ifa: String,
    pub ifa_type: String,
    pub lmt: String,
}

impl ExtnPayloadProvider for AdvertisingRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Advertising(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<AdvertisingRequest> {
        if let ExtnPayload::Request(ExtnRequest::Advertising(r)) = payload {
            return Some(r);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::Advertising
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum AdvertisingResponse {
    None,
    AdInitObject(AdInitObjectResponse),
    AdIdObject(AdIdResponse),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvertisingFrameworkConfig {
    pub ad_server_url: String,
    pub ad_server_url_template: String,
    pub ad_network_id: String,
    pub ad_profile_id: String,
    pub ad_site_section_id: String,
    pub ad_opt_out: bool,
    pub privacy_data: String,
    pub ifa_value: String,
    pub ifa: String,
    pub app_name: String,
    pub app_bundle_id: String,
    pub distributor_app_id: String,
    pub device_ad_attributes: String,
    pub coppa: u32,
    pub authentication_entity: String,
}

impl ExtnPayloadProvider for AdvertisingResponse {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::Advertising(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Response(ExtnResponse::Advertising(v)) = payload {
            return Some(v);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::Advertising
    }
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GetAdConfig {
    pub options: AdConfig,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    #[default]
    Prod,
    Test,
}

impl fmt::Display for Environment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Environment::Prod => write!(f, "prod"),
            Environment::Test => write!(f, "test"),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct AdConfig {
    #[serde(default)]
    pub environment: Environment,
    // COPPA stands for Children's Online Privacy Protection Act.
    pub coppa: Option<bool>,
    pub authentication_entity: Option<String>,
}

impl Default for GetAdConfig {
    fn default() -> Self {
        GetAdConfig {
            options: AdConfig {
                environment: Environment::default(),
                coppa: Some(false),
                authentication_entity: Some("".to_owned()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils::test_extn_payload_provider;

    #[test]
    fn test_extn_request_advertising() {
        let ad_init_request_params = AdInitObjectRequestParams {
            privacy_data: HashMap::new(),
            environment: "test_environment".to_string(),
            durable_app_id: "test_durable_app_id".to_string(),
            app_version: "test_app_version".to_string(),
            distributor_app_id: "test_distributor_app_id".to_string(),
            device_ad_attributes: HashMap::new(),
            coppa: true,
            authentication_entity: "test_authentication_entity".to_string(),
            dist_session: AccountSession {
                id: "test_session_id".to_string(),
                token: "test_token".to_string(),
                account_id: "test_account_id".to_string(),
                device_id: "test_device_id".to_string(),
            },
            scope: HashMap::new(),
        };

        let advertising_request = AdvertisingRequest::GetAdInitObject(ad_init_request_params);
        let contract_type: RippleContract = RippleContract::Advertising;

        test_extn_payload_provider(advertising_request, contract_type);
    }

    #[test]
    fn test_extn_response_advertising() {
        let ad_init_object_response = AdInitObjectResponse {
            ad_server_url: "https://example.com/ad_server".to_string(),
            ad_server_url_template: "https://example.com/ad_template".to_string(),
            ad_network_id: "network_id".to_string(),
            ad_profile_id: "profile_id".to_string(),
            ad_site_section_id: "section_id".to_string(),
            ad_opt_out: false,
            privacy_data: "privacy_data".to_string(),
            ifa_value: "ifa_value".to_string(),
            ifa: "ifa".to_string(),
            app_name: "MyApp".to_string(),
            app_bundle_id: "com.example.myapp".to_string(),
            app_version: "1.0.0".to_string(),
            distributor_app_id: "distributor_id".to_string(),
            device_ad_attributes: "device_attributes".to_string(),
            coppa: "coppa_value".to_string(),
            authentication_entity: "auth_entity".to_string(),
        };

        let advertising_response = AdvertisingResponse::AdInitObject(ad_init_object_response);
        let contract_type: RippleContract = RippleContract::Advertising;

        test_extn_payload_provider(advertising_response, contract_type);
    }
}
