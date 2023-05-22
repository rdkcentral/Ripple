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

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    api::{device::entertainment_data::ContentIdentifiers, session::AccountSession},
    utils::serde_utils::{optional_date_time_str_serde, progress_value_deserialize},
};

pub const DISCOVERY_EVENT_ON_NAVIGATE_TO: &'static str = "discovery.onNavigateTo";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DiscoveryContext {
    pub source: String,
}

impl DiscoveryContext {
    pub fn new(source: &str) -> DiscoveryContext {
        return DiscoveryContext {
            source: source.to_string(),
        };
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NavigationIntent {
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    pub context: DiscoveryContext,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SectionIntentData {
    pub section_name: String,
}

impl Default for NavigationIntent {
    fn default() -> NavigationIntent {
        NavigationIntent {
            action: "home".to_string(),
            data: None,
            context: DiscoveryContext::new("device"),
        }
    }
}

impl PartialEq for NavigationIntent {
    fn eq(&self, other: &Self) -> bool {
        self.action.eq(&other.action) && self.context.eq(&other.context)
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct LaunchRequest {
    #[serde(rename = "appId")]
    pub app_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intent: Option<NavigationIntent>,
}

impl LaunchRequest {
    pub fn new(
        app_id: String,
        action: String,
        data: Option<Value>,
        source: String,
    ) -> LaunchRequest {
        LaunchRequest {
            app_id,
            intent: Some(NavigationIntent {
                action,
                data,
                context: DiscoveryContext { source },
            }),
        }
    }

    pub fn get_intent(&self) -> NavigationIntent {
        self.intent.clone().unwrap_or_default()
    }
}

//TODO: need to update 1.0 code

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct EntitlementData {
    pub entitlement_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
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
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum LocalizedString {
    Simple(String),
    Locale(HashMap<String, String>),
}

#[derive(Debug, Deserialize, Clone)]
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
#[derive(Debug, Deserialize, Clone)]
pub struct WatchNextInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<LocalizedString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
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

#[derive(Debug, Deserialize, Clone)]
pub struct ContentAccessRequest {
    pub ids: ContentAccessIdentifiers,
}

pub const ENTITY_INFO_EVENT: &'static str = "discovery.onPullEntityInfo";
pub const ENTITY_INFO_CAPABILITY: &'static str = "discovery:entity-info";
pub const PURCHASED_CONTENT_EVENT: &'static str = "discovery.onPullPurchasedContent";
pub const PURCHASED_CONTENT_CAPABILITY: &'static str = "discovery:purchased-content";
pub const EVENT_DISCOVERY_POLICY_CHANGED: &'static str = "discovery.onPolicyChanged";

#[derive(Debug, Deserialize, Clone)]
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

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Availability {
    #[serde(rename = "type")]
    pub _type: ContentType,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catalog_id: Option<String>,
    #[serde(
        with = "optional_date_time_str_serde",
        skip_serializing_if = "Option::is_none"
    )]
    pub start_time: Option<String>,
    #[serde(
        with = "optional_date_time_str_serde",
        skip_serializing_if = "Option::is_none"
    )]
    pub end_time: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
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
            entitlements: sign_in_info.entitlements.clone(),
        }
    }
}

impl From<EntitlementsInfo> for ContentAccessIdentifiers {
    fn from(entitlements_info: EntitlementsInfo) -> Self {
        ContentAccessIdentifiers {
            availabilities: None,
            entitlements: Some(entitlements_info.entitlements.clone()),
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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContentAccessEntitlement {
    pub entitlement_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SessionParams {
    pub app_id: String, // eg: Netflix
    pub dist_session: AccountSession,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContentAccessInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availabilities: Option<Vec<ContentAccessAvailability>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entitlements: Option<Vec<ContentAccessEntitlement>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ClearContentSetParams {
    pub session_info: SessionParams,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContentAccessListSetParams {
    pub session_info: SessionParams,
    pub content_access_info: ContentAccessInfo,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentAccessResponse {}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SignInRequestParams {
    pub session_info: SessionParams,
    pub is_signed_in: bool, /*true for signIn, false for signOut */
}

// #[derive(Debug, Clone)]
// pub enum DiscoveryAccountLinkRequest {
//     EntitlementsAccountLink(EntitlementsAccountLinkRequestParams),
//     MediaEventAccountLink(MediaEventsAccountLinkRequestParams),
//     LaunchPadAccountLink(LaunchPadAccountLinkRequestParams),
//     SignIn(SignInRequestParams),
// }

pub const PROGRESS_UNIT_SECONDS: &'static str = "Seconds";
pub const PROGRESS_UNIT_PERCENT: &'static str = "Percent";

pub const ACCOUNT_LINK_TYPE_ACCOUNT_LINK: &'static str = "AccountLink";
pub const ACCOUNT_LINK_TYPE_ENTITLEMENT_UPDATES: &'static str = "EntitlementUpdates";
pub const ACCOUNT_LINK_TYPE_LAUNCH_PAD: &'static str = "LaunchPad";

pub const ACCOUNT_LINK_ACTION_SIGN_IN: &'static str = "SignIn";
pub const ACCOUNT_LINK_ACTION_SIGN_OUT: &'static str = "SignOut";
pub const ACCOUNT_LINK_ACTION_APP_LAUNCH: &'static str = "AppLaunch";
pub const ACCOUNT_LINK_ACTION_CREATE: &'static str = "Create";
pub const ACCOUNT_LINK_ACTION_DELETE: &'static str = "Delete";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiscoveryEntitlement {
    pub entitlement_id: String,
    pub start_time: i64,
    pub end_time: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ProgressUnit {
    Seconds,
    Percent,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaEvent {
    pub content_id: String,
    pub completed: bool,
    pub progress: f32,
    pub progress_unit: ProgressUnit,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaEventsAccountLinkRequestParams {
    pub media_event: MediaEvent,
    pub content_partner_id: String,
    pub client_supports_opt_out: bool,
    pub dist_session: AccountSession,
    pub data_tags: HashSet<String>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaEventsAccountLinkResponse {}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LaunchPadAccountLinkResponse {}

// #[async_trait]
// pub trait AccountLinkService {
//     async fn entitlements_account_link(
//         self: Box<Self>,
//         request: DpabRequest,
//         params: EntitlementsAccountLinkRequestParams,
//     );
//     async fn media_events_account_link(
//         self: Box<Self>,
//         request: DpabRequest,
//         params: MediaEventsAccountLinkRequestParams,
//     );
//     async fn launch_pad_account_link(
//         self: Box<Self>,
//         request: DpabRequest,
//         params: LaunchPadAccountLinkRequestParams,
//     );
//     async fn sign_in(self: Box<Self>, request: DpabRequest, params: SignInRequestParams);
// }
