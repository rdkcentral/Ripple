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
    GetAdIdObject(AdIdRequestParams),
    ResetAdIdentifier(AccountSession),
    GetAdConfig(AdConfigRequestParams),
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

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct AdConfigRequestParams {
    pub privacy_data: HashMap<String, String>,
    pub durable_app_id: String,
    pub dist_session: AccountSession,
    pub environment: String,
    pub scope: HashMap<String, String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Default)]
pub struct AdConfigResponse {
    pub ad_server_url: String,
    pub ad_server_url_template: String,
    pub ad_network_id: String,
    pub ad_profile_id: String,
    pub ad_site_section_id: String,
    pub app_bundle_id: String,
    pub ifa: String,
    pub ifa_value: String,
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

#[derive(Serialize, PartialEq, Deserialize, Debug, Clone)]
pub enum AdvertisingResponse {
    None,
    AdIdObject(AdIdResponse),
    AdConfig(AdConfigResponse),
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
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
#[cfg_attr(test, derive(PartialEq))]
#[serde(rename_all = "camelCase")]
pub struct GetAdConfig {
    pub options: AdConfig,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
#[cfg_attr(test, derive(PartialEq))]
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
#[cfg_attr(test, derive(PartialEq))]
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
    use rstest::rstest;

    #[rstest]
    #[case(Environment::Prod, "prod")]
    #[case(Environment::Test, "test")]
    fn test_display_impl(#[case] environment: Environment, #[case] expected_output: &str) {
        let result = format!("{}", environment);
        assert_eq!(result, expected_output);
    }

    #[test]
    fn test_default() {
        let default_config = GetAdConfig::default();
        let expected_config = GetAdConfig {
            options: AdConfig {
                environment: Environment::Prod,
                coppa: Some(false),
                authentication_entity: Some("".to_owned()),
            },
        };
        assert_eq!(default_config, expected_config);
    }

    #[test]
    fn test_extn_payload_provider_for_advertising_request() {
        let ad_id_request_params = AdIdRequestParams {
            privacy_data: HashMap::new(),
            app_id: String::from("test_app"),
            dist_session: AccountSession {
                id: String::from("test_id"),
                token: String::from("test_token"),
                account_id: String::from("test_account_id"),
                device_id: String::from("test_device_id"),
            },
            scope: HashMap::new(),
        };

        let advertising_request = AdvertisingRequest::GetAdIdObject(ad_id_request_params);

        let contract_type: RippleContract = RippleContract::Advertising;
        test_extn_payload_provider(advertising_request, contract_type);
    }

    #[test]
    fn test_extn_payload_provider_for_advertising_response() {
        let ad_id_response = AdIdResponse {
            ifa: String::from("test_ifa"),
            ifa_type: String::from("test_ifa_type"),
            lmt: String::from("test_lmt"),
        };

        let advertising_response = AdvertisingResponse::AdIdObject(ad_id_response);

        let contract_type: RippleContract = RippleContract::Advertising;
        test_extn_payload_provider(advertising_response, contract_type);
    }
}
