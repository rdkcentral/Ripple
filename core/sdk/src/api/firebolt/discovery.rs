use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::message::{DistributorSession, DpabRequest};

#[derive(Debug, Clone)]
pub enum DiscoveryAccountLinkRequest {
    EntitlementsAccountLink(EntitlementsAccountLinkRequestParams),
    MediaEventAccountLink(MediaEventsAccountLinkRequestParams),
    LaunchPadAccountLink(LaunchPadAccountLinkRequestParams),
    SignIn(SignInRequestParams),
}

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
    pub dist_session: DistributorSession,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntitlementsAccountLinkResponse {}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaEventsAccountLinkRequestParams {
    pub media_event: MediaEvent,
    pub content_partner_id: String,
    pub client_supports_opt_out: bool,
    pub dist_session: DistributorSession,
    pub data_tags: HashSet<String>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaEventsAccountLinkResponse {}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LaunchPadAccountLinkRequestParams {
    pub link_launchpad: AccountLaunchpad,
    pub content_partner_id: String,
    pub dist_session: DistributorSession,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LaunchPadAccountLinkResponse {}

#[async_trait]
pub trait AccountLinkService {
    async fn entitlements_account_link(
        self: Box<Self>,
        request: DpabRequest,
        params: EntitlementsAccountLinkRequestParams,
    );
    async fn media_events_account_link(
        self: Box<Self>,
        request: DpabRequest,
        params: MediaEventsAccountLinkRequestParams,
    );
    async fn launch_pad_account_link(
        self: Box<Self>,
        request: DpabRequest,
        params: LaunchPadAccountLinkRequestParams,
    );
    async fn sign_in(self: Box<Self>, request: DpabRequest, params: SignInRequestParams);
}

#[derive(Debug, Clone)]
pub enum DiscoveryRequest {
    SetContentAccess(ContentAccessListSetParams),
    ClearContent(ClearContentSetParams),
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
    pub dist_session: DistributorSession,
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
#[derive(Debug, Serialize, Clone)]
pub struct ContentAccessListSetParams {
    pub session_info: SessionParams,
    pub content_access_info: ContentAccessInfo,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentAccessResponse {}

#[derive(Debug, Clone)]
pub struct SignInRequestParams {
    pub session_info: SessionParams,
    pub is_signed_in: bool, /*true for signIn, false for signOut */
}
