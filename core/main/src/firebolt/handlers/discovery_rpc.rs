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

use crate::{
    firebolt::rpc::RippleRPCProvider,
    service::apps::app_events::AppEvents,
    utils::rpc_utils::{rpc_await_oneshot, rpc_err, rpc_navigate_reserved_app_err},
};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    RpcModule,
};
use ripple_sdk::{
    api::{
        apps::{AppError, AppManagerResponse, AppMethod, AppRequest, AppResponse},
        config::Config,
        firebolt::fb_discovery::DISCOVERY_EVENT_ON_NAVIGATE_TO,
    },
    extn::extn_client_message::ExtnResponse,
    log::{error, info},
    tokio::sync::oneshot,
};
use ripple_sdk::{
    api::{
        device::entertainment_data::*,
        firebolt::{
            fb_general::{ListenRequest, ListenerResponse},
            provider::ExternalProviderResponse,
        },
        gateway::rpc_gateway_api::CallContext,
    },
    utils::serde_utils::{optional_date_time_str_serde, progress_value_deserialize},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::state::platform_state::PlatformState;
pub const EVENT_POLICY_CHANGED: &'static str = "discovery.onPolicyChanged";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Context {
    pub source: String,
}

impl Context {
    fn new(source: &str) -> Context {
        return Context {
            source: source.to_string(),
        };
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NavigationIntent {
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    pub context: Context,
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
            context: Context::new("device"),
        }
    }
}

impl PartialEq for NavigationIntent {
    fn eq(&self, other: &Self) -> bool {
        self.action.eq(&other.action) && self.context.eq(&other.context)
    }
}

#[derive(Deserialize, Clone, Debug)]
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

/// A list of identifiers that represent what content is consumable for the subscriber.
///
/// Excluding entitlements will cause no change to the entitlements that are stored for this subscriber.
/// Providing an empty array will clear the subscriber's entitlements"
/// # Examples
///
/// ```
/// let entitlement = EntitlementData {
///
/// };
/// `
#[derive(Debug, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct EntitlementData {
    pub entitlement_id: String,
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
    pub identifiers: Option<ContentIdentifiers>,
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
                context: Context { source },
            }),
        }
    }
}
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
/// A list of identifiers that represent what content is discoverable for the subscriber.
///
/// Excluding availabilities will cause no change to the availabilities that are stored for this subscriber.
/// Providing an empty array will clear the subscriber's availabilities"
/// # Examples
///
/// ```
/// let availabilty = Availability {
///     _type: ContentType::ProgramLineup,
///     id: "partner.com/availability/123".to_string(),
///     start_time: Some("2021-04-23T18:25:43.511Z".to_string()),
///     end_time: Some("2021-04-23T18:25:43.511Z".to_string()),
/// };
/// ```
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

#[rpc(server)]
pub trait Discovery {
    #[method(name = "discovery.entitlements")]
    async fn entitlements(
        &self,
        ctx: CallContext,
        entitlements_info: EntitlementsInfo,
    ) -> RpcResult<bool>;
    #[method(name = "discovery.signIn")]
    async fn sign_in(&self, ctx: CallContext, sign_in_info: SignInInfo) -> RpcResult<bool>;
    #[method(name = "discovery.signOut")]
    async fn sign_out(&self, ctx: CallContext) -> RpcResult<bool>;
    #[method(name = "discovery.watched")]
    async fn watched(&self, ctx: CallContext, watched_info: WatchedInfo) -> RpcResult<bool>;
    #[method(name = "discovery.watchNext")]
    async fn watch_next(&self, ctx: CallContext, watch_next_info: WatchNextInfo)
        -> RpcResult<bool>;
    #[method(name = "discovery.launch")]
    async fn launch(&self, ctx: CallContext, request: LaunchRequest) -> RpcResult<bool>;
    #[method(name = "discovery.onPullEntityInfo")]
    async fn on_pull_entity_info(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "content.entity")]
    async fn get_entity(
        &self,
        ctx: CallContext,
        entity_request: ContentEntityRequest,
    ) -> RpcResult<ContentEntityResponse>;
    #[method(name = "discovery.entityInfo")]
    async fn handle_entity_info_result(
        &self,
        ctx: CallContext,
        entity_info: ExternalProviderResponse<Option<EntityInfoResult>>,
    ) -> RpcResult<bool>;
    #[method(name = "discovery.onPullPurchasedContent")]
    async fn on_pull_purchased_content(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "content.purchases")]
    async fn get_purchases(
        &self,
        ctx: CallContext,
        entity_request: ProvidedPurchaseContentRequest,
    ) -> RpcResult<ProvidedPurchasedContentResult>;
    #[method(name = "discovery.purchasedContent")]
    async fn handle_purchased_content_result(
        &self,
        ctx: CallContext,
        entity_info: ExternalProviderResponse<PurchasedContentResult>,
    ) -> RpcResult<bool>;
    #[method(name = "content.providers")]
    async fn get_providers(&self, ctx: CallContext) -> RpcResult<Vec<ContentProvider>>;
    #[method(name = "discovery.onNavigateTo")]
    async fn on_navigate_to(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "discovery.policy")]
    async fn get_content_policy_rpc(&self, ctx: CallContext) -> RpcResult<ContentPolicy>;

    #[method(name = "discovery.onPolicyChanged")]
    async fn on_policy_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "discovery.contentAccess")]
    async fn discovery_content_access(
        &self,
        ctx: CallContext,
        request: ContentAccessRequest,
    ) -> RpcResult<()>;
    #[method(name = "discovery.clearContentAccess")]
    async fn discovery_clear_content_access(&self, ctx: CallContext) -> RpcResult<()>;
}

pub struct DiscoveryImpl {
    pub platform_state: PlatformState,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    entity_id: String,
}

//TODO: Have to check if this can be ported.
pub async fn get_content_partner_id(
    platform_state: &PlatformState,
    ctx: &CallContext,
) -> RpcResult<String> {
    let mut content_partner_id = ctx.app_id.to_owned();
    let (app_resp_tx, app_resp_rx) = oneshot::channel::<Result<AppManagerResponse, AppError>>();

    let app_request = AppRequest::new(
        AppMethod::GetAppContentCatalog(ctx.app_id.clone()),
        app_resp_tx,
    );
    if let Err(e) = platform_state.get_client().send_app_request(app_request) {
        error!("Send error for set_state {:?}", e);
        return Err(rpc_err("Unable send app request"));
    }
    let resp = rpc_await_oneshot(app_resp_rx).await?;

    if let AppManagerResponse::AppContentCatalog(content_catalog) = resp? {
        content_partner_id = content_catalog.map_or(ctx.app_id.to_owned(), |x| x.to_owned())
    }
    Ok(content_partner_id)
}

impl DiscoveryImpl {
    fn convert_provider_result(&self, provider_result: ProviderResult) -> Vec<ContentProvider> {
        let mut content_providers = Vec::new();
        for (key, values) in provider_result.entries {
            let apis: Vec<String> = values
                .into_iter()
                .filter(|val| val.contains("discovery.onPull"))
                .map(|elem| {
                    if elem == "discovery.onPullPurchasedContent" {
                        String::from("purchases")
                    } else if elem == "discovery.onPullEntityInfo" {
                        String::from("entity")
                    } else {
                        String::from("")
                    }
                })
                .filter(|val| val.len() > 0)
                .collect();
            if apis.len() > 0 {
                content_providers.push(ContentProvider { id: key, apis });
            }
        }
        content_providers
    }

    pub async fn get_content_policy(
        ctx: &CallContext,
        state: &PlatformState,
        app_id: &str,
    ) -> RpcResult<ContentPolicy> {
        let mut content_policy: ContentPolicy = Default::default();
        content_policy.enable_recommendations = false; // TODO: Need to replace with PrivacyImpl
        content_policy.share_watch_history = false; // TODO: Need to replace with PrivacyImpl
        content_policy.remember_watched_programs = false; // TODO: Need to replace with PrivacyImpl
        Ok(content_policy)
    }

    fn get_share_watch_history() -> bool {
        false
    }

    async fn get_titles_from_localized_string(
        &self,
        title: &LocalizedString,
    ) -> HashMap<String, String> {
        let mut title_map = HashMap::new();
        let result = self
            .platform_state
            .get_client()
            .send_extn_request(Config::DefaultLanguage)
            .await;
        let def_lang = match result {
            Ok(extn_message) => {
                match extn_message
                    .payload
                    .extract()
                    .unwrap_or(ExtnResponse::String("en".to_owned()))
                {
                    ExtnResponse::String(value) => value.to_owned(),
                    _ => "en".to_owned(),
                }
            }
            Err(_) => "en".to_owned(),
        };
        match title {
            LocalizedString::Simple(value) => {
                title_map.insert(def_lang, value.to_string());
            }
            LocalizedString::Locale(value) => {
                for (locale, description) in value.iter() {
                    title_map.insert(locale.to_string(), description.to_string());
                }
            }
        }
        title_map
    }
}

#[async_trait]
impl DiscoveryServer for DiscoveryImpl {
    async fn entitlements(
        &self,
        ctx: CallContext,
        entitlements_info: EntitlementsInfo,
    ) -> RpcResult<bool> {
        todo!()
    }

    async fn sign_in(&self, ctx: CallContext, sign_in_info: SignInInfo) -> RpcResult<bool> {
        todo!()
    }
    async fn sign_out(&self, ctx: CallContext) -> RpcResult<bool> {
        todo!()
    }
    async fn watched(&self, ctx: CallContext, watched_info: WatchedInfo) -> RpcResult<bool> {
        todo!()
    }
    async fn watch_next(
        &self,
        ctx: CallContext,
        watch_next_info: WatchNextInfo,
    ) -> RpcResult<bool> {
        todo!()
    }
    async fn launch(&self, _ctx: CallContext, request: LaunchRequest) -> RpcResult<bool> {
        let app_defaults_configuration = self
            .platform_state
            .get_device_manifest()
            .applications
            .defaults;

        if let Some(reserved_app_id) =
            app_defaults_configuration.get_reserved_application_id(&request.app_id)
        {
            if reserved_app_id.is_empty() {
                return Err(rpc_navigate_reserved_app_err(
                    format!(
                        "Discovery.launch: Cannot find a valid reserved app id for {}",
                        request.app_id
                    )
                    .as_str(),
                ));
            }

            // Not validating the intent, pass-through to app as is.
            if !AppEvents::is_app_registered_for_event(
                &self.platform_state,
                reserved_app_id.to_string(),
                DISCOVERY_EVENT_ON_NAVIGATE_TO,
            ) {
                return Err(rpc_navigate_reserved_app_err(
                    format!("Discovery.launch: reserved app id {} is not registered for discovery.onNavigateTo event",
                    reserved_app_id).as_str(),
                ));
            }
            // emit EVENT_ON_NAVIGATE_TO to the reserved app.
            AppEvents::emit_to_app(
                &self.platform_state,
                reserved_app_id.to_string(),
                DISCOVERY_EVENT_ON_NAVIGATE_TO,
                &serde_json::to_value(request.intent).unwrap(),
            )
            .await;
            info!(
                "emit_to_app called for app {} event {}",
                reserved_app_id.to_string(),
                DISCOVERY_EVENT_ON_NAVIGATE_TO
            );
            return Ok(true);
        }
        let (app_resp_tx, app_resp_rx) = oneshot::channel::<AppResponse>();

        let app_request = AppRequest::new(AppMethod::Launch(request.clone()), app_resp_tx);

        if let Ok(_) = self
            .platform_state
            .get_client()
            .send_app_request(app_request)
        {
            if let Ok(_) = app_resp_rx.await {
                return Ok(true);
            }
        }

        Err(jsonrpsee::core::Error::Custom(String::from(
            "Discovery.launch: some failure",
        )))
    }

    async fn on_pull_entity_info(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        todo!()
    }
    async fn get_entity(
        &self,
        ctx: CallContext,
        entity_request: ContentEntityRequest,
    ) -> RpcResult<ContentEntityResponse> {
        todo!()
    }
    async fn handle_entity_info_result(
        &self,
        ctx: CallContext,
        entity_info: ExternalProviderResponse<Option<EntityInfoResult>>,
    ) -> RpcResult<bool> {
        todo!()
    }
    async fn on_pull_purchased_content(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        todo!()
    }
    async fn get_purchases(
        &self,
        ctx: CallContext,
        entity_request: ProvidedPurchaseContentRequest,
    ) -> RpcResult<ProvidedPurchasedContentResult> {
        todo!()
    }
    async fn handle_purchased_content_result(
        &self,
        ctx: CallContext,
        entity_info: ExternalProviderResponse<PurchasedContentResult>,
    ) -> RpcResult<bool> {
        todo!()
    }
    async fn get_providers(&self, ctx: CallContext) -> RpcResult<Vec<ContentProvider>> {
        todo!()
    }
    async fn on_navigate_to(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        todo!()
    }
    async fn get_content_policy_rpc(&self, ctx: CallContext) -> RpcResult<ContentPolicy> {
        todo!()
    }
    async fn on_policy_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        todo!()
    }
    async fn discovery_content_access(
        &self,
        ctx: CallContext,
        request: ContentAccessRequest,
    ) -> RpcResult<()> {
        todo!()
    }
    async fn discovery_clear_content_access(&self, ctx: CallContext) -> RpcResult<()> {
        todo!()
    }
}

#[derive(Debug, Clone)]
pub struct DiscoveryRpcProvider;
impl RippleRPCProvider<DiscoveryImpl> for DiscoveryRpcProvider {
    fn provide(platform_state: PlatformState) -> RpcModule<DiscoveryImpl> {
        (DiscoveryImpl { platform_state }).into_rpc()
    }
}
