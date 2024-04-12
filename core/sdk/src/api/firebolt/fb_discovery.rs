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

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::utils::error::RippleError;
use crate::{
    api::{
        device::entertainment_data::{ContentIdentifiers, NavigationIntent},
        session::AccountSession,
    },
    utils::serde_utils::{optional_date_time_str_serde, progress_value_deserialize},
};
use async_trait::async_trait;

pub const DISCOVERY_EVENT_ON_NAVIGATE_TO: &str = "discovery.onNavigateTo";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DiscoveryContext {
    pub source: String,
}

impl DiscoveryContext {
    pub fn new(source: &str) -> DiscoveryContext {
        DiscoveryContext {
            source: source.to_string(),
        }
    }
}

#[derive(Deserialize, PartialEq, Serialize, Clone, Debug)]
pub struct LaunchRequest {
    #[serde(rename = "appId")]
    pub app_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intent: Option<NavigationIntent>,
}

impl LaunchRequest {
    pub fn get_intent(&self) -> NavigationIntent {
        self.intent.clone().unwrap_or_default()
    }
}

//TODO: need to update 1.0 code

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct EntitlementData {
    pub entitlement_id: String,
    #[serde(
        default,
        with = "optional_date_time_str_serde",
        skip_serializing_if = "Option::is_none"
    )]
    pub start_time: Option<String>,
    #[serde(
        default,
        with = "optional_date_time_str_serde",
        skip_serializing_if = "Option::is_none"
    )]
    pub end_time: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct EntitlementsInfo {
    pub entitlements: Vec<EntitlementData>,
}

impl From<EntitlementsInfo> for ContentAccessRequest {
    fn from(entitlements_info: EntitlementsInfo) -> Self {
        ContentAccessRequest {
            ids: entitlements_info.into(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct SignInInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entitlements: Option<Vec<EntitlementData>>,
}

//type LocalizedString = string | object
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum LocalizedString {
    Simple(String),
    Locale(HashMap<String, String>),
}

#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WatchedInfo {
    pub entity_id: String,
    #[serde(default, deserialize_with = "progress_value_deserialize")]
    pub progress: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<bool>,
    #[serde(
        default,
        with = "optional_date_time_str_serde",
        skip_serializing_if = "Option::is_none"
    )]
    pub watched_on: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WatchNextInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<LocalizedString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub identifiers: ContentIdentifiers,
    #[serde(
        default,
        with = "optional_date_time_str_serde",
        skip_serializing_if = "Option::is_none"
    )]
    pub expires: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<HashMap<String, HashMap<String, String>>>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct NavigateCompanyPageRequest {
    #[serde(rename = "companyId")]
    pub company_id: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct ContentAccessRequest {
    pub ids: ContentAccessIdentifiers,
}

pub const ENTITY_INFO_EVENT: &str = "discovery.onPullEntityInfo";
pub const ENTITY_INFO_CAPABILITY: &str = "discovery:entity-info";
pub const PURCHASED_CONTENT_EVENT: &str = "discovery.onPullPurchasedContent";
pub const EVENT_ON_SIGN_IN: &str = "discovery.onSignIn";
pub const EVENT_ON_SIGN_OUT: &str = "discovery.onSignOut";
pub const PURCHASED_CONTENT_CAPABILITY: &str = "discovery:purchased-content";
pub const EVENT_DISCOVERY_POLICY_CHANGED: &str = "discovery.onPolicyChanged";

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum ContentType {
    ChannelLineup,
    ProgramLineup,
}
impl ContentType {
    pub fn as_string(&self) -> &'static str {
        match self {
            ContentType::ChannelLineup => "channel-lineup",
            ContentType::ProgramLineup => "program-lineup",
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Availability {
    #[serde(rename = "type")]
    pub _type: ContentType,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catalog_id: Option<String>,
    #[serde(
        default,
        with = "optional_date_time_str_serde",
        skip_serializing_if = "Option::is_none"
    )]
    pub start_time: Option<String>,
    #[serde(
        default,
        with = "optional_date_time_str_serde",
        skip_serializing_if = "Option::is_none"
    )]
    pub end_time: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContentAccessIdentifiers {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availabilities: Option<Vec<Availability>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entitlements: Option<Vec<EntitlementData>>,
}

impl From<SignInInfo> for ContentAccessIdentifiers {
    fn from(sign_in_info: SignInInfo) -> Self {
        ContentAccessIdentifiers {
            availabilities: None,
            entitlements: sign_in_info.entitlements,
        }
    }
}

impl From<EntitlementsInfo> for ContentAccessIdentifiers {
    fn from(entitlements_info: EntitlementsInfo) -> Self {
        ContentAccessIdentifiers {
            availabilities: None,
            entitlements: Some(entitlements_info.entitlements),
        }
    }
}

impl From<SignInInfo> for ContentAccessRequest {
    fn from(sign_in_info: SignInInfo) -> Self {
        ContentAccessRequest {
            ids: sign_in_info.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum DiscoveryRequest {
    SetContentAccess(ContentAccessListSetParams),
    ClearContent(ClearContentSetParams),
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContentAccessEntitlement {
    pub entitlement_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContentAccessAvailability {
    #[serde(rename = "type")]
    pub _type: String,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catalog_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SessionParams {
    pub app_id: String, // eg: Netflix
    pub dist_session: AccountSession,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContentAccessInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availabilities: Option<Vec<ContentAccessAvailability>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entitlements: Option<Vec<ContentAccessEntitlement>>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct ClearContentSetParams {
    pub session_info: SessionParams,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct ContentAccessListSetParams {
    pub session_info: SessionParams,
    pub content_access_info: ContentAccessInfo,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentAccessResponse {}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct SignInRequestParams {
    pub session_info: SessionParams,
    pub is_signed_in: bool, /*true for signIn, false for signOut */
}

#[derive(Debug, Clone)]
pub enum DiscoveryAccountLinkRequest {
    EntitlementsAccountLink(EntitlementsAccountLinkRequestParams),
    MediaEventAccountLink(MediaEventsAccountLinkRequestParams),
    LaunchPadAccountLink(LaunchPadAccountLinkRequestParams),
    SignIn(SignInRequestParams),
}

pub const PROGRESS_UNIT_SECONDS: &str = "Seconds";
pub const PROGRESS_UNIT_PERCENT: &str = "Percent";

pub const ACCOUNT_LINK_TYPE_ACCOUNT_LINK: &str = "AccountLink";
pub const ACCOUNT_LINK_TYPE_ENTITLEMENT_UPDATES: &str = "EntitlementUpdates";
pub const ACCOUNT_LINK_TYPE_LAUNCH_PAD: &str = "LaunchPad";

pub const ACCOUNT_LINK_ACTION_SIGN_IN: &str = "SignIn";
pub const ACCOUNT_LINK_ACTION_SIGN_OUT: &str = "SignOut";
pub const ACCOUNT_LINK_ACTION_APP_LAUNCH: &str = "AppLaunch";
pub const ACCOUNT_LINK_ACTION_CREATE: &str = "Create";
pub const ACCOUNT_LINK_ACTION_DELETE: &str = "Delete";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiscoveryEntitlement {
    pub entitlement_id: String,
    pub start_time: i64,
    pub end_time: i64,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ProgressUnit {
    Seconds,
    Percent,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct MediaEvent {
    pub content_id: String,
    pub completed: bool,
    pub progress: f32,
    pub progress_unit: Option<ProgressUnit>,
    pub watched_on: Option<String>,
    pub app_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccountLaunchpad {
    pub expiration: i64,
    pub app_name: String,
    pub content_id: Option<String>,
    pub deeplink: Option<String>,
    pub content_url: Option<String>,
    pub app_id: String,
    pub title: HashMap<String, String>,
    pub images: HashMap<String, HashMap<String, String>>,
    pub account_link_type: String,
    pub account_link_action: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntitlementsAccountLinkRequestParams {
    pub account_link_type: Option<String>,
    pub account_link_action: Option<String>,
    pub entitlements: Vec<DiscoveryEntitlement>,
    pub app_id: String,
    pub content_partner_id: String,
    pub dist_session: AccountSession,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntitlementsAccountLinkResponse {}

#[derive(Deserialize, Serialize, Debug, Clone, Hash, PartialEq, Eq)]
pub struct DataTagInfo {
    pub tag_name: String,
    pub propagation_state: bool,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct MediaEventsAccountLinkRequestParams {
    pub media_event: MediaEvent,
    pub content_partner_id: String,
    pub client_supports_opt_out: bool,
    pub dist_session: AccountSession,
    pub data_tags: HashSet<DataTagInfo>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaEventsAccountLinkResponse {}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LaunchPadAccountLinkRequestParams {
    pub link_launchpad: AccountLaunchpad,
    pub content_partner_id: String,
    pub dist_session: AccountSession,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LaunchPadAccountLinkResponse {}

#[async_trait]
pub trait AccountLinkService {
    async fn entitlements_account_link(
        self: Box<Self>,
        params: EntitlementsAccountLinkRequestParams,
    ) -> Result<EntitlementsAccountLinkResponse, RippleError>;
    async fn media_events_account_link(
        self: Box<Self>,
        params: MediaEventsAccountLinkRequestParams,
    ) -> Result<MediaEventsAccountLinkResponse, RippleError>;
    async fn launch_pad_account_link(
        self: Box<Self>,
        params: LaunchPadAccountLinkRequestParams,
    ) -> Result<LaunchPadAccountLinkResponse, RippleError>;
    async fn sign_in(self: Box<Self>, params: SignInRequestParams) -> Result<(), RippleError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::device::entertainment_data::{HomeIntent, NavigationIntentStrict};

    #[test]
    fn test_new_discovery_context() {
        let context = DiscoveryContext::new("test_source");
        assert_eq!(context.source, "test_source");
    }

    #[test]
    fn test_get_intent_home() {
        let home_intent = HomeIntent {
            context: DiscoveryContext {
                source: "test_source".to_string(),
            },
        };

        let launch_request = LaunchRequest {
            app_id: "test_app".to_string(),
            intent: Some(NavigationIntent::NavigationIntentStrict(
                NavigationIntentStrict::Home(home_intent),
            )),
        };

        let intent = launch_request.get_intent();
        assert_eq!(
            intent,
            NavigationIntent::NavigationIntentStrict(NavigationIntentStrict::Home(HomeIntent {
                context: DiscoveryContext {
                    source: "test_source".to_string()
                }
            }))
        );
    }

    #[test]
    fn test_entitlements_to_content_access_request() {
        let ed = EntitlementData {
            entitlement_id: "test_entitlement_id".to_string(),
            start_time: Some("2024-01-26T12:00:00Z".to_string()),
            end_time: Some("2024-02-01T12:00:00Z".to_string()),
        };
        let entitlements_info = EntitlementsInfo {
            entitlements: vec![ed.clone()],
        };

        let content_access_request: ContentAccessRequest = entitlements_info.into();
        assert_eq!(content_access_request.ids.entitlements, Some(vec![ed]));
    }

    #[test]
    fn test_content_type_as_string() {
        let channel_lineup = ContentType::ChannelLineup;
        assert_eq!(channel_lineup.as_string(), "channel-lineup");

        let program_lineup = ContentType::ProgramLineup;
        assert_eq!(program_lineup.as_string(), "program-lineup");
    }

    fn get_mock_entitlement_data() -> Vec<EntitlementData> {
        vec![
            EntitlementData {
                entitlement_id: "entitlement_id1".to_string(),
                start_time: Some("2021-01-01T00:00:00.000Z".to_string()),
                end_time: Some("2021-01-01T00:00:00.000Z".to_string()),
            },
            EntitlementData {
                entitlement_id: "entitlement_id2".to_string(),
                start_time: Some("2021-01-02T00:00:00.000Z".to_string()),
                end_time: Some("2021-01-02T00:00:00.000Z".to_string()),
            },
        ]
    }

    #[test]
    fn test_from_sign_in_info_to_content_access_identifiers() {
        let sign_in_info = SignInInfo {
            entitlements: Some(get_mock_entitlement_data()),
        };

        let content_access_identifiers = ContentAccessIdentifiers::from(sign_in_info);
        assert_eq!(
            content_access_identifiers,
            ContentAccessIdentifiers {
                availabilities: None,
                entitlements: Some(get_mock_entitlement_data()),
            }
        );
    }

    #[test]
    fn test_from_entitlements_info_to_content_access_identifiers() {
        let entitlements_info = EntitlementsInfo {
            entitlements: get_mock_entitlement_data(),
        };

        let content_access_identifiers = ContentAccessIdentifiers::from(entitlements_info);
        assert_eq!(
            content_access_identifiers,
            ContentAccessIdentifiers {
                availabilities: None,
                entitlements: Some(get_mock_entitlement_data()),
            }
        );
    }

    #[test]
    fn test_from_sign_in_info_to_content_access_request() {
        let sign_in_info = SignInInfo {
            entitlements: Some(get_mock_entitlement_data()),
        };

        let content_access_request = ContentAccessRequest::from(sign_in_info);
        assert_eq!(
            content_access_request,
            ContentAccessRequest {
                ids: ContentAccessIdentifiers {
                    availabilities: None,
                    entitlements: Some(get_mock_entitlement_data()),
                },
            }
        );
    }

    #[test]
    fn test_entitlements_data() {
        let ed = "{\"entitlementId\":\"\"}";
        assert!(serde_json::from_str::<EntitlementData>(ed).is_ok());
        let ed = "{\"entitlementId\":\"value\"}";
        assert!(serde_json::from_str::<EntitlementData>(ed).is_ok());
        let ed = "{\"entitlementId\":\"value\",\"startTime\":\"01022023\"}";
        assert!(serde_json::from_str::<EntitlementData>(ed).is_err());
        let ed = "{\"entitlementId\":\"value\",\"startTime\":\"2021-01-01T00:00:00.000Z\"}";
        assert!(serde_json::from_str::<EntitlementData>(ed).is_ok());
        let ed = "{\"entitlementId\":\"value\",\"startTime\":\"2021-01-01T00:00:00.000Z\",\"endTime\":\"01022023\"}";
        assert!(serde_json::from_str::<EntitlementData>(ed).is_err());
        let ed = "{\"entitlementId\":\"value\",\"startTime\":\"2021-01-01T00:00:00.000Z\",\"endTime\":\"2021-01-01T00:00:00.000Z\"}";
        assert!(serde_json::from_str::<EntitlementData>(ed).is_ok());
    }

    #[test]
    fn test_watched_info() {
        let wi = "{\"entityId\":\"value\", \"progress\":0.0, \"watchedOn\": \"2021-01-01T00:00:00.000Z\"}";
        assert!(serde_json::from_str::<WatchedInfo>(wi).is_ok());

        let wi =
            "{\"entityId\":\"\", \"progress\":0.0, \"watchedOn\": \"2021-01-01T00:00:00.000Z\"}";
        assert!(serde_json::from_str::<WatchedInfo>(wi).is_ok());

        let wi = "{\"entityId\":\"value\", \"progress\":-1.0, \"watchedOn\": \"2021-01-01T00:00:00.000Z\"}";
        assert!(serde_json::from_str::<WatchedInfo>(wi).is_err());

        let wi = "{\"entityId\":\"value\", \"progress\":0.0, \"watchedOn\": \"01022023Z\"}";
        assert!(serde_json::from_str::<WatchedInfo>(wi).is_err());

        let wi = "{\"entityId\":\"value\", \"progress\":0.0, \"watchedOn\": \"2021-01-01T00:00:00.000Z\",\"completed\":true}";
        if let Ok(v) = serde_json::from_str::<WatchedInfo>(wi) {
            assert!(v.completed.unwrap())
        } else {
            panic!("invalid watched info")
        }
    }

    #[test]
    fn test_schema() {
        if let Ok(v) = serde_json::from_str::<LaunchRequest>("{\"appId\":\"test\",\"intent\":{\"action\":\"playback\",\"data\":{\"programType\":\"movie\",\"entityId\":\"example-movie-id\"},\"context\":{\"source\":\"voice\"}}}"){
            assert!(v.app_id.eq("test"))
        } else {
            panic!("Launch Schema Fail")
        }
    }
}
