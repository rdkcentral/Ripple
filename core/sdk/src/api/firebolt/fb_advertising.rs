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

use crate::api::session::AccountSession;

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

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[cfg_attr(feature = "mock", derive(PartialEq))]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coppa: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
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
    use rstest::rstest;
    use std::collections::HashMap;

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

    // Comprehensive JSON serialization/deserialization tests for Firebolt OpenRPC compliance

    #[test]
    fn test_ad_id_request_params_json_serialization() {
        let mut privacy_data = HashMap::new();
        privacy_data.insert("key1".to_string(), "value1".to_string());
        privacy_data.insert("key2".to_string(), "value2".to_string());

        let mut scope = HashMap::new();
        scope.insert("device".to_string(), "type".to_string());

        let params = AdIdRequestParams {
            privacy_data,
            app_id: "com.example.app".to_string(),
            dist_session: AccountSession {
                id: "session123".to_string(),
                token: "token456".to_string(),
                account_id: "account789".to_string(),
                device_id: "device321".to_string(),
            },
            scope,
        };

        let json = serde_json::to_string(&params).unwrap();
        let deserialized: AdIdRequestParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_ad_id_request_params_json_deserialization() {
        let json = r#"{
            "privacy_data": {"gdpr": "1", "ccpa": "0"},
            "app_id": "com.test.app",
            "dist_session": {
                "id": "test_session",
                "token": "test_token",
                "account_id": "test_account",
                "device_id": "test_device"
            },
            "scope": {"platform": "tv"}
        }"#;

        let deserialized: AdIdRequestParams = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized.app_id, "com.test.app");
        assert_eq!(deserialized.dist_session.id, "test_session");
        assert_eq!(deserialized.privacy_data.get("gdpr").unwrap(), "1");
        assert_eq!(deserialized.scope.get("platform").unwrap(), "tv");
    }

    #[test]
    fn test_ad_id_request_params_invalid_json() {
        let invalid_json = r#"{"privacy_data": "invalid"}"#;
        let result: Result<AdIdRequestParams, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_ad_id_response_json_serialization() {
        let response = AdIdResponse {
            ifa: "12345678-1234-1234-1234-123456789abc".to_string(),
            ifa_type: "rida".to_string(),
            lmt: "0".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: AdIdResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(response, deserialized);
    }

    #[test]
    fn test_ad_id_response_json_deserialization() {
        let json = r#"{
            "ifa": "98765432-8765-4321-8765-987654321abc",
            "ifa_type": "advertising_id",
            "lmt": "1"
        }"#;

        let deserialized: AdIdResponse = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized.ifa, "98765432-8765-4321-8765-987654321abc");
        assert_eq!(deserialized.ifa_type, "advertising_id");
        assert_eq!(deserialized.lmt, "1");
    }

    #[test]
    fn test_ad_id_response_invalid_json() {
        let invalid_json = r#"{"ifa": 123}"#;
        let result: Result<AdIdResponse, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_ad_config_request_params_json_serialization() {
        let mut privacy_data = HashMap::new();
        privacy_data.insert("privacy".to_string(), "consent".to_string());

        let mut scope = HashMap::new();
        scope.insert("region".to_string(), "us".to_string());

        let params = AdConfigRequestParams {
            privacy_data,
            durable_app_id: "durable_app_123".to_string(),
            dist_session: AccountSession {
                id: "config_session".to_string(),
                token: "config_token".to_string(),
                account_id: "config_account".to_string(),
                device_id: "config_device".to_string(),
            },
            environment: "production".to_string(),
            scope,
        };

        let json = serde_json::to_string(&params).unwrap();
        let deserialized: AdConfigRequestParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_ad_config_request_params_json_deserialization() {
        let json = r#"{
            "privacy_data": {"consent": "given"},
            "durable_app_id": "app456",
            "dist_session": {
                "id": "session456",
                "token": "token789",
                "account_id": "account123",
                "device_id": "device456"
            },
            "environment": "staging",
            "scope": {"locale": "en-US"}
        }"#;

        let deserialized: AdConfigRequestParams = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized.durable_app_id, "app456");
        assert_eq!(deserialized.environment, "staging");
        assert_eq!(deserialized.privacy_data.get("consent").unwrap(), "given");
    }

    #[test]
    fn test_ad_config_request_params_invalid_json() {
        let invalid_json = r#"{"durable_app_id": 456}"#;
        let result: Result<AdConfigRequestParams, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_ad_config_response_json_serialization() {
        let response = AdConfigResponse {
            ad_server_url: "https://ad.server.com".to_string(),
            ad_server_url_template: "https://ad.server.com/{params}".to_string(),
            ad_network_id: "network123".to_string(),
            ad_profile_id: "profile456".to_string(),
            ad_site_section_id: "section789".to_string(),
            app_bundle_id: "com.app.bundle".to_string(),
            ifa: "ifa_value_123".to_string(),
            ifa_value: "ifa_456".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: AdConfigResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(response, deserialized);
    }

    #[test]
    fn test_ad_config_response_default() {
        let response = AdConfigResponse::default();
        let json = serde_json::to_string(&response).unwrap();
        let deserialized: AdConfigResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(response, deserialized);
        assert!(response.ad_server_url.is_empty());
        assert!(response.app_bundle_id.is_empty());
    }

    #[test]
    fn test_ad_config_response_json_deserialization() {
        let json = r#"{
            "ad_server_url": "https://test.ad.com",
            "ad_server_url_template": "https://test.ad.com/{id}",
            "ad_network_id": "test_network",
            "ad_profile_id": "test_profile",
            "ad_site_section_id": "test_section",
            "app_bundle_id": "com.test.bundle",
            "ifa": "test_ifa",
            "ifa_value": "test_ifa_value"
        }"#;

        let deserialized: AdConfigResponse = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized.ad_server_url, "https://test.ad.com");
        assert_eq!(deserialized.ad_network_id, "test_network");
        assert_eq!(deserialized.app_bundle_id, "com.test.bundle");
    }

    #[test]
    fn test_advertising_framework_config_json_serialization() {
        let config = AdvertisingFrameworkConfig {
            ad_server_url: "https://framework.ad.com".to_string(),
            ad_server_url_template: "https://framework.ad.com/{param}".to_string(),
            ad_network_id: "framework_network".to_string(),
            ad_profile_id: "framework_profile".to_string(),
            ad_site_section_id: "framework_section".to_string(),
            ad_opt_out: false,
            privacy_data: "privacy_string".to_string(),
            ifa_value: "framework_ifa_value".to_string(),
            ifa: "framework_ifa".to_string(),
            app_name: "Framework App".to_string(),
            app_bundle_id: "com.framework.app".to_string(),
            distributor_app_id: "dist_app_123".to_string(),
            device_ad_attributes: "device_attrs".to_string(),
            coppa: 0,
            authentication_entity: "auth_entity".to_string(),
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AdvertisingFrameworkConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.ad_server_url, deserialized.ad_server_url);
        assert_eq!(config.ad_opt_out, deserialized.ad_opt_out);
        assert_eq!(config.coppa, deserialized.coppa);
    }

    #[test]
    fn test_advertising_framework_config_default() {
        let config = AdvertisingFrameworkConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AdvertisingFrameworkConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.ad_server_url, deserialized.ad_server_url);
        assert!(!config.ad_opt_out);
        assert_eq!(config.coppa, 0);
    }

    #[test]
    fn test_advertising_framework_config_json_deserialization() {
        let json = r#"{
            "adServerUrl": "https://test.framework.com",
            "adServerUrlTemplate": "https://test.framework.com/{test}",
            "adNetworkId": "test_framework_network",
            "adProfileId": "test_framework_profile",
            "adSiteSectionId": "test_framework_section",
            "adOptOut": true,
            "privacyData": "test_privacy",
            "ifaValue": "test_framework_ifa_value",
            "ifa": "test_framework_ifa",
            "appName": "Test Framework App",
            "appBundleId": "com.test.framework",
            "distributorAppId": "test_dist_app",
            "deviceAdAttributes": "test_device_attrs",
            "coppa": 1,
            "authenticationEntity": "test_auth"
        }"#;

        let deserialized: AdvertisingFrameworkConfig = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized.ad_server_url, "https://test.framework.com");
        assert!(deserialized.ad_opt_out);
        assert_eq!(deserialized.coppa, 1);
        assert_eq!(deserialized.app_name, "Test Framework App");
    }

    #[test]
    fn test_environment_enum_json_serialization() {
        let prod = Environment::Prod;
        let test = Environment::Test;

        let prod_json = serde_json::to_string(&prod).unwrap();
        let test_json = serde_json::to_string(&test).unwrap();

        assert_eq!(prod_json, "\"prod\"");
        assert_eq!(test_json, "\"test\"");

        let deserialized_prod: Environment = serde_json::from_str(&prod_json).unwrap();
        let deserialized_test: Environment = serde_json::from_str(&test_json).unwrap();

        assert_eq!(prod, deserialized_prod);
        assert_eq!(test, deserialized_test);
    }

    #[test]
    fn test_environment_enum_json_deserialization() {
        let prod_json = "\"prod\"";
        let test_json = "\"test\"";

        let deserialized_prod: Environment = serde_json::from_str(prod_json).unwrap();
        let deserialized_test: Environment = serde_json::from_str(test_json).unwrap();

        assert_eq!(deserialized_prod, Environment::Prod);
        assert_eq!(deserialized_test, Environment::Test);
    }

    #[test]
    fn test_environment_enum_invalid_json() {
        let invalid_json = "\"invalid\"";
        let result: Result<Environment, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_environment_enum_default() {
        let default_env = Environment::default();
        assert_eq!(default_env, Environment::Prod);
    }

    #[test]
    fn test_ad_config_json_serialization() {
        let config = AdConfig {
            environment: Environment::Test,
            coppa: Some(true),
            authentication_entity: Some("test_auth".to_string()),
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AdConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_ad_config_json_deserialization() {
        let json = r#"{
            "environment": "test",
            "coppa": false,
            "authenticationEntity": "auth_test"
        }"#;

        let deserialized: AdConfig = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized.environment, Environment::Test);
        assert_eq!(deserialized.coppa, Some(false));
        assert_eq!(
            deserialized.authentication_entity,
            Some("auth_test".to_string())
        );
    }

    #[test]
    fn test_ad_config_optional_fields_serialization() {
        let config = AdConfig {
            environment: Environment::Prod,
            coppa: None,
            authentication_entity: None,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AdConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
        assert_eq!(deserialized.coppa, None);
        assert_eq!(deserialized.authentication_entity, None);
    }

    #[test]
    fn test_ad_config_default() {
        let config = AdConfig::default();
        assert_eq!(config.environment, Environment::Prod);
        assert_eq!(config.coppa, None);
        assert_eq!(config.authentication_entity, None);
    }

    #[test]
    fn test_get_ad_config_json_serialization() {
        let get_config = GetAdConfig {
            options: AdConfig {
                environment: Environment::Test,
                coppa: Some(false),
                authentication_entity: Some("get_test_auth".to_string()),
            },
        };

        let json = serde_json::to_string(&get_config).unwrap();
        let deserialized: GetAdConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(get_config, deserialized);
    }

    #[test]
    fn test_get_ad_config_json_deserialization() {
        let json = r#"{
            "options": {
                "environment": "prod",
                "coppa": true,
                "authenticationEntity": "get_auth_entity"
            }
        }"#;

        let deserialized: GetAdConfig = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized.options.environment, Environment::Prod);
        assert_eq!(deserialized.options.coppa, Some(true));
        assert_eq!(
            deserialized.options.authentication_entity,
            Some("get_auth_entity".to_string())
        );
    }

    #[test]
    fn test_get_ad_config_default_implementation() {
        let default_config = GetAdConfig::default();
        let json = serde_json::to_string(&default_config).unwrap();
        let deserialized: GetAdConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(default_config, deserialized);
        assert_eq!(default_config.options.environment, Environment::Prod);
        assert_eq!(default_config.options.coppa, Some(false));
        assert_eq!(
            default_config.options.authentication_entity,
            Some("".to_string())
        );
    }

    #[test]
    fn test_get_ad_config_invalid_json() {
        let invalid_json = r#"{"options": "invalid"}"#;
        let result: Result<GetAdConfig, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_ad_config_minimal_json() {
        let json = r#"{"environment": "prod"}"#;
        let deserialized: AdConfig = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized.environment, Environment::Prod);
        assert_eq!(deserialized.coppa, None);
        assert_eq!(deserialized.authentication_entity, None);
    }

    #[test]
    fn test_advertising_framework_config_invalid_json() {
        let invalid_json = r#"{"adOptOut": "invalid_boolean"}"#;
        let result: Result<AdvertisingFrameworkConfig, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_complex_nested_serialization_roundtrip() {
        let mut privacy_data = HashMap::new();
        privacy_data.insert("gdpr".to_string(), "1".to_string());
        privacy_data.insert("ccpa".to_string(), "0".to_string());

        let request = AdIdRequestParams {
            privacy_data,
            app_id: "com.complex.test".to_string(),
            dist_session: AccountSession {
                id: "complex_session".to_string(),
                token: "complex_token".to_string(),
                account_id: "complex_account".to_string(),
                device_id: "complex_device".to_string(),
            },
            scope: {
                let mut scope = HashMap::new();
                scope.insert("platform".to_string(), "smart_tv".to_string());
                scope.insert("region".to_string(), "north_america".to_string());
                scope
            },
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: AdIdRequestParams = serde_json::from_str(&json).unwrap();
        assert_eq!(request, deserialized);
        assert_eq!(deserialized.privacy_data.len(), 2);
        assert_eq!(deserialized.scope.len(), 2);
    }
}
