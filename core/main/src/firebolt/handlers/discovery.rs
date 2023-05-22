use jsonrpsee::{
    core::{async_trait, Error, RpcResult},
    proc_macros::rpc,
    RpcModule,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, time::Duration};
use tokio::{sync::oneshot, time::timeout};
use tracing::{error, info};

use crate::{
    api::rpc::rpc_gateway::{CallContext, RPCProvider},
    apps::{
        app_events::{
            AppEventDecorationError, AppEventDecorator, AppEvents, ListenRequest, ListenerResponse,
        },
        app_mgr::{AppError, AppManagerResponse, AppMethod, AppRequest, EVENT_ON_NAVIGATE_TO},
        provider_broker::{
            self, ExternalProviderResponse, ProviderBroker, ProviderRequestPayload,
            ProviderResponse, ProviderResponsePayload,
        },
    },
    helpers::{
        error_util::{
            rpc_await_oneshot, rpc_downstream_service_err, rpc_navigate_reserved_app_err,
        },
        ripple_helper::{IRippleHelper, RippleHelper, RippleHelperFactory, RippleHelperType},
        rpc_util::rpc_err,
        serde_utils::{
            link_action_deserialize, link_type_deserialize, optional_date_time_str_serde,
            progress_unit_deserialize, progress_value_deserialize,
        },
        session_util::{dab_to_dpab, get_distributor_session_from_platform_state},
    },
    managers::{
        capability_manager::{
            CapClassifiedRequest, FireboltCap, IGetLoadedCaps, RippleHandlerCaps,
        },
        data_governance::{DataGovernance, DataType},
        storage::storage_property::EVENT_DISCOVERY_POLICY_CHANGED,
    },
    platform_state::PlatformState,
};
use dab::core::utils::{convert_time_stamp_to_epoch_time, convert_timestamp_to_iso8601};
use dpab::core::{
    message::{DpabRequestPayload, DpabResponsePayload},
    model::discovery::{
        AccountLaunchpad, ClearContentSetParams, ContentAccessAvailability,
        ContentAccessEntitlement, ContentAccessInfo, ContentAccessListSetParams,
        DiscoveryAccountLinkRequest, DiscoveryRequest, WatchHistoryAccountLinkRequestParams,
        MediaEvent, MediaEventsAccountLinkRequestParams, ProgressUnit, SessionParams,
        SignInRequestParams, ACCOUNT_LINK_ACTION_CREATE, ACCOUNT_LINK_ACTION_DELETE,
        ACCOUNT_LINK_TYPE_LAUNCH_PAD,
    },
};

pub const BADGER_MEDIA_EVENT_PERCENT: &'static str = "percent";
pub const BADGER_MEDIA_EVENT_SECONDS: &'static str = "seconds";
pub const BADGER_ENTITLEMENT_SIGNIN: &'static str = "signIn";
pub const BADGER_ENTITLEMENT_SIGNOUT: &'static str = "signOut";
pub const BADGER_ENTITLEMENT_APPLAUNCH: &'static str = "appLaunch";
pub const BADGER_ENTITLEMENTS_UPDATE: &'static str = "entitlementsUpdate";
pub const BADGER_ENTITLEMENTS_ACCOUNTLINK: &'static str = "accountLink";

#[derive(Default, Serialize, Debug)]
pub struct BadgerEmptyResult {
    //Empty object to take care of OTTX-28709
}

impl PartialEq for BadgerEmptyResult {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}
type EmptyResult = BadgerEmptyResult;

impl Default for NavigationIntent {
    fn default() -> Self {
        NavigationIntent::Home(HomeIntent::default())
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

impl From<SubscriptionEntitlements> for EntitlementData {
    fn from(sub_entitlement: SubscriptionEntitlements) -> Self {
        let mut entitlement_data: EntitlementData = Default::default();
        entitlement_data.entitlement_id = sub_entitlement.id.clone();
        if let Some(start_date_timestamp) = sub_entitlement.start_date {
            entitlement_data.start_time = Some(convert_timestamp_to_iso8601(start_date_timestamp));
        }
        if let Some(end_date_timestamp) = sub_entitlement.end_date {
            entitlement_data.end_time = Some(convert_timestamp_to_iso8601(end_date_timestamp));
        }
        entitlement_data
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
pub use crate::api::handlers::entertainment_data::*;

use super::privacy::PrivacyImpl;

#[derive(Deserialize, Clone, Debug)]
pub struct NavigateCompanyPageRequest {
    #[serde(rename = "companyId")]
    pub company_id: String,
}

// $Badger Data strutcures for account link
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionEntitlements {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<i64>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BadgerEntitlementsAccountLinkRequest {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "link_action_deserialize"
    )]
    pub action: Option<String>, /*signIn, signOut, appLaunch,  */
    #[serde(
        default,
        rename = "type",
        skip_serializing_if = "Option::is_none",
        deserialize_with = "link_type_deserialize"
    )]
    pub link_type: Option<String>, /* entitlementsUpdate */
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscription_entitlements: Option<Vec<SubscriptionEntitlements>>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BadgerLaunchpadTileData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub titles: Option<LocalizedString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration: Option<i64>,
    pub content_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<HashMap<String, HashMap<String, String>>>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BadgerLaunchpadTile {
    pub launchpad_tile: BadgerLaunchpadTileData,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BadgerMediaEventData {
    pub content_id: String,
    pub completed: bool,
    #[serde(default, deserialize_with = "progress_value_deserialize")]
    pub progress: f32,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "progress_unit_deserialize"
    )]
    pub progress_units: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BadgerMediaEvent {
    pub event: BadgerMediaEventData,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ContentAccessRequest {
    pub ids: ContentAccessIdentifiers,
}

pub const ENTITY_INFO_EVENT: &'static str = "discovery.onPullEntityInfo";
pub const ENTITY_INFO_CAPABILITY: &'static str = "discovery:entity-info";
pub const PURCHASED_CONTENT_EVENT: &'static str = "discovery.onPullPurchasedContent";
pub const PURCHASED_CONTENT_CAPABILITY: &'static str = "discovery:purchased-content";

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
    #[method(name = "badger.navigateToCompanyPage")]
    async fn badger_navigate_to_company_page(
        &self,
        ctx: CallContext,
        req: NavigateCompanyPageRequest,
    ) -> RpcResult<BadgerEmptyResult>;
    #[method(name = "discovery.policy")]
    async fn get_content_policy_rpc(&self, ctx: CallContext) -> RpcResult<ContentPolicy>;
    #[method(name = "badger.entitlementsAccountLink")]
    async fn badger_entitlements_account_link(
        &self,
        ctx: CallContext,
        request: BadgerEntitlementsAccountLinkRequest,
    ) -> RpcResult<BadgerEmptyResult>;
    #[method(name = "badger.launchpadAccountLink")]
    async fn badger_launch_pad_account_link(
        &self,
        ctx: CallContext,
        launch_pad_tile_data: BadgerLaunchpadTile,
    ) -> RpcResult<BadgerEmptyResult>;
    #[method(name = "badger.mediaEventAccountLink")]
    async fn badger_media_event_account_link(
        &self,
        ctx: CallContext,
        badger_media_event: BadgerMediaEvent,
    ) -> RpcResult<BadgerEmptyResult>;
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

pub struct DiscoveryImpl<IRippleHelper> {
    pub helper: Box<IRippleHelper>,
    pub platform_state: PlatformState,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    entity_id: String,
}
pub async fn get_content_partner_id(
    ripple_helper: &RippleHelper,
    ctx: &CallContext,
) -> RpcResult<String> {
    let mut content_partner_id = ctx.app_id.to_owned();
    let (app_resp_tx, app_resp_rx) = oneshot::channel::<Result<AppManagerResponse, AppError>>();

    let app_request = AppRequest {
        method: AppMethod::GetAppContentCatalog(ctx.app_id.clone()),
        resp_tx: Some(app_resp_tx),
    };
    ripple_helper.send_app_request(app_request).await?;

    let resp = rpc_await_oneshot(app_resp_rx).await?;

    if let AppManagerResponse::AppContentCatalog(content_catalog) = resp? {
        content_partner_id = content_catalog.map_or(ctx.app_id.to_owned(), |x| x.to_owned())
    }
    Ok(content_partner_id)
}

impl DiscoveryImpl<RippleHelper> {
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
        content_policy.enable_recommendations =
            PrivacyImpl::get_allow_personalization(state, &app_id).await;
        content_policy.share_watch_history =
            PrivacyImpl::get_share_watch_history(ctx, state, &app_id).await;
        content_policy.remember_watched_programs =
            PrivacyImpl::get_allow_watch_history(state, &app_id).await;
        Ok(content_policy)
    }

    fn get_share_watch_history() -> bool {
        false
    }
    fn get_titles_from_localized_string(&self, title: &LocalizedString) -> HashMap<String, String> {
        let mut title_map = HashMap::new();

        match title {
            LocalizedString::Simple(value) => {
                title_map.insert(
                    self.helper.get_config().get_default_language().to_owned(),
                    value.to_string(),
                );
            }
            LocalizedString::Locale(value) => {
                for (locale, description) in value.iter() {
                    title_map.insert(locale.to_string(), description.to_string());
                }
            }
        }
        title_map
    }
    async fn process_sign_in_request(
        &self,
        ctx: CallContext,
        is_signed_in: bool,
    ) -> RpcResult<bool> {
        let session = get_distributor_session_from_platform_state(&self.platform_state).await?;

        let payload = DpabRequestPayload::AccountLink(DiscoveryAccountLinkRequest::SignIn(
            SignInRequestParams {
                session_info: SessionParams {
                    app_id: ctx.clone().app_id.to_owned(),
                    dist_session: dab_to_dpab(session).unwrap(),
                },
                is_signed_in,
            },
        ));

        let resp = self.helper.clone().send_dpab(payload).await;
        if let Err(_) = resp {
            error!("Error: Notifying SignIn info to the platform: {:?}", resp);
            return Err(rpc_downstream_service_err(
                "Error: Notifying SignIn info to the platform",
            ));
        }

        match resp.unwrap() {
            DpabResponsePayload::None => Ok(true),
            _ => Err(rpc_downstream_service_err(
                "Did not receive a valid resposne from platform when notifying SignIn info",
            )),
        }
    }

    async fn process_entitlement_update(
        &self,
        ctx: CallContext,
        subscription_entitlements: Option<Vec<SubscriptionEntitlements>>,
    ) -> RpcResult<EmptyResult> {
        let mut resp = Ok(EmptyResult::default());

        if subscription_entitlements.is_some() {
            let ent_data = subscription_entitlements
                .unwrap()
                .into_iter()
                .map(|item| item.into())
                .collect();
            resp = self
                .content_access(
                    ctx.clone(),
                    EntitlementsInfo {
                        entitlements: ent_data,
                    }
                    .into(),
                )
                .await;
        }
        resp
    }

    async fn content_access(
        &self,
        ctx: CallContext,
        request: ContentAccessRequest,
    ) -> RpcResult<EmptyResult> {
        let session = get_distributor_session_from_platform_state(&self.platform_state).await?;

        // If both entitlement & availability are None return EmptyResult
        if request.ids.availabilities.is_none() && request.ids.entitlements.is_none() {
            return Ok(EmptyResult::default());
        }

        let payload = DpabRequestPayload::Discovery(DiscoveryRequest::SetContentAccess(
            ContentAccessListSetParams {
                session_info: SessionParams {
                    app_id: ctx.app_id.to_owned(),
                    dist_session: dab_to_dpab(session).unwrap(),
                },
                content_access_info: ContentAccessInfo {
                    availabilities: request.ids.availabilities.map(|availability_vec| {
                        availability_vec
                            .into_iter()
                            .map(|x| ContentAccessAvailability {
                                _type: x._type.as_string().to_owned(),
                                id: x.id,
                                catalog_id: x.catalog_id,
                                start_time: x.start_time,
                                end_time: x.end_time,
                            })
                            .collect()
                    }),
                    entitlements: request.ids.entitlements.map(|entitlement_vec| {
                        entitlement_vec
                            .into_iter()
                            .map(|x| ContentAccessEntitlement {
                                entitlement_id: x.entitlement_id,
                                start_time: x.start_time,
                                end_time: x.end_time,
                            })
                            .collect()
                    }),
                },
            },
        ));
        let resp = self.helper.clone().send_dpab(payload).await;
        if let Err(_) = resp {
            error!(
                "Error: Notifying Content AccessList to the platform: {:?}",
                resp
            );
            return Err(rpc_downstream_service_err(
                "Could not notify Content AccessList to the platform",
            ));
        }

        match resp.unwrap() {
            DpabResponsePayload::ContentAccess(_obj) => Ok(EmptyResult::default()),
            _ => Err(rpc_downstream_service_err(
                "Did not receive a valid resposne from platform when notifying Content Access List",
            )),
        }
    }

    async fn clear_content_access(&self, ctx: CallContext) -> RpcResult<EmptyResult> {
        let session = get_distributor_session_from_platform_state(&self.platform_state).await?;

        let payload =
            DpabRequestPayload::Discovery(DiscoveryRequest::ClearContent(ClearContentSetParams {
                session_info: SessionParams {
                    app_id: ctx.app_id.to_owned(),
                    dist_session: dab_to_dpab(session).unwrap(),
                },
            }));
        let resp = self.helper.clone().send_dpab(payload).await;
        if let Err(_) = resp {
            error!(
                "Error: Clearing the Content AccessList to the platform: {:?}",
                resp
            );
            return Err(rpc_downstream_service_err(
                "Could not clear Content AccessList from the platform",
            ));
        }
        match resp.unwrap() {
            DpabResponsePayload::ContentAccess(_obj) => Ok(EmptyResult::default()),
            _ => Err(rpc_downstream_service_err(
                "Did not receive a valid resposne from platform when clearing the Content Access List",
            )),
        }
    }
}

#[derive(Clone)]
struct DiscoveryPolicyEventDecorator {}

#[async_trait]
impl AppEventDecorator for DiscoveryPolicyEventDecorator {
    async fn decorate(
        &self,
        ps: &PlatformState,
        ctx: &CallContext,
        _event_name: &str,
        val_in: &Value,
    ) -> Result<Value, AppEventDecorationError> {
        match DiscoveryImpl::get_content_policy(&ctx, &ps, &ctx.app_id).await {
            Ok(cp) => Ok(serde_json::to_value(cp).unwrap()),
            Err(_) => Err(AppEventDecorationError {}),
        }
    }
    fn dec_clone(&self) -> Box<dyn AppEventDecorator + Send + Sync> {
        Box::new(self.clone())
    }
}

#[async_trait]
impl DiscoveryServer for DiscoveryImpl<RippleHelper> {
    // #[instrument(skip(self))]
    async fn on_policy_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        AppEvents::add_listener_with_decorator(
            &&self.platform_state.app_events_state,
            EVENT_DISCOVERY_POLICY_CHANGED.to_string(),
            ctx,
            request,
            Some(Box::new(DiscoveryPolicyEventDecorator {})),
        );

        Ok(ListenerResponse {
            listening: listen,
            event: EVENT_DISCOVERY_POLICY_CHANGED,
        })
    }
    async fn entitlements(
        &self,
        ctx: CallContext,
        entitlements_info: EntitlementsInfo,
    ) -> RpcResult<bool> {
        info!("Discovery.entitlements");

        let resp = self.content_access(ctx, entitlements_info.into()).await;
        match resp {
            Ok(_) => Ok(true),
            Err(e) => Err(e),
        }
    }

    async fn sign_in(&self, ctx: CallContext, sign_in_info: SignInInfo) -> RpcResult<bool> {
        info!("Discovery.signIn");

        let mut resp = Ok(EmptyResult::default());
        let fut = self.process_sign_in_request(ctx.clone(), true);

        if sign_in_info.entitlements.is_some() {
            resp = self.content_access(ctx, sign_in_info.into()).await;
        }

        let mut sign_in_resp = fut.await;

        // Return Ok if both dpap calls are successful.
        if sign_in_resp.is_ok() && resp.is_ok() {
            sign_in_resp = Ok(true);
        } else {
            sign_in_resp = Err(rpc_downstream_service_err("Received error from Server"));
        }

        sign_in_resp
    }
    async fn sign_out(&self, ctx: CallContext) -> RpcResult<bool> {
        info!("Discovery.signOut");
        // Note : Do NOT issue clearContentAccess for Firebolt SignOut case.
        self.process_sign_in_request(ctx.clone(), false).await
    }
    async fn watched(&self, ctx: CallContext, watched_info: WatchedInfo) -> RpcResult<bool> {
        info!("Discovery.watched");
        let (data_tags, drop_data) =
            DataGovernance::resolve_tags(&self.platform_state, DataType::Watched).await;
        if drop_data {
            return Ok(false);
        }
        let session = get_distributor_session_from_platform_state(&self.platform_state).await?;

        let progress = watched_info.progress;

        let payload =
            DpabRequestPayload::AccountLink(DiscoveryAccountLinkRequest::MediaEventAccountLink(
                MediaEventsAccountLinkRequestParams {
                    media_event: MediaEvent {
                        content_id: watched_info.entity_id.to_owned(),
                        completed: watched_info.completed.unwrap_or(true),
                        progress: if progress > 1.0 {
                            progress
                        } else {
                            progress * 100.0
                        },
                        progress_unit: if progress > 1.0 {
                            ProgressUnit::Seconds
                        } else {
                            ProgressUnit::Percent
                        },
                        watched_on: watched_info.watched_on.clone(),
                        app_id: ctx.clone().app_id.to_owned(),
                    },
                    content_partner_id: get_content_partner_id(&self.helper, &ctx)
                        .await
                        .unwrap_or(ctx.app_id.to_owned()),
                    client_supports_opt_out: DiscoveryImpl::get_share_watch_history(),
                    dist_session: dab_to_dpab(session).unwrap(),
                    data_tags,
                },
            ));
        let resp = self.helper.clone().send_dpab(payload).await;
        if let Err(_) = resp {
            error!("Error: Notifying watched info to the platform: {:?}", resp);
            return Err(rpc_err(
                "Could not notify the platform that content was partially or completely watched",
            ));
        }

        match resp.unwrap() {
            DpabResponsePayload::MediaEventsAccountLink(_obj) => Ok(true),
            _ => Err(rpc_err(
                "Did not receive a valid resposne from platform when notifying watched info",
            )),
        }
    }
    async fn watch_next(
        &self,
        ctx: CallContext,
        watch_next_info: WatchNextInfo,
    ) -> RpcResult<bool> {
        info!("Discovery.watchNext");
        let session = get_distributor_session_from_platform_state(&self.platform_state).await?;
        let watched_info = WatchedInfo {
            entity_id: watch_next_info.identifiers.entity_id,
            progress: 1.0,
            completed: Some(false),
            watched_on: None,
        };
        return self.watched(ctx, watched_info.clone()).await;
    }

    async fn get_content_policy_rpc(&self, ctx: CallContext) -> RpcResult<ContentPolicy> {
        DiscoveryImpl::get_content_policy(&ctx, &self.platform_state, &ctx.app_id).await
    }

    async fn launch(&self, _ctx: CallContext, request: LaunchRequest) -> RpcResult<bool> {
        let app_defaults_configuration = self.helper.get_config().get_app_defaults_configuration();

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
                EVENT_ON_NAVIGATE_TO,
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
                EVENT_ON_NAVIGATE_TO,
                &serde_json::to_value(request.intent).unwrap(),
            )
            .await;
            info!(
                "emit_to_app called for app {} event {}",
                reserved_app_id.to_string(),
                EVENT_ON_NAVIGATE_TO
            );
            return Ok(true);
        }
        let (app_resp_tx, app_resp_rx) = oneshot::channel::<Result<AppManagerResponse, AppError>>();
        let app_request = AppRequest {
            method: AppMethod::Launch(request.clone()),
            resp_tx: Some(app_resp_tx),
        };

        if let Ok(_) = self.helper.send_app_request(app_request).await {
            if let Ok(_) = app_resp_rx.await {
                return Ok(true);
            }
        }

        Err(jsonrpsee::core::Error::Custom(String::from(
            "Discovery.launch: some failure",
        )))
    }
    async fn on_navigate_to(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        AppEvents::add_listener(
            &&self.platform_state.app_events_state,
            EVENT_ON_NAVIGATE_TO.to_string(),
            ctx,
            request,
        );

        Ok(ListenerResponse {
            listening: listen,
            event: EVENT_ON_NAVIGATE_TO,
        })
    }

    async fn badger_navigate_to_company_page(
        &self,
        _ctx: CallContext,
        req: NavigateCompanyPageRequest,
    ) -> RpcResult<BadgerEmptyResult> {
        if let Some(lookup_app) = self.helper.get_config().get_default_app() {
            let data = SectionIntentData {
                section_name: format!("company:{}", req.company_id),
            };
            let company_page_intent = NavigationIntent::Section(SectionIntent {
                data: data,
                context: Context {
                    source: String::from("system"),
                },
            });

            AppEvents::emit_to_app(
                &self.platform_state,
                lookup_app.app_id,
                "discovery.onNavigateTo",
                &serde_json::to_value(company_page_intent).unwrap(),
            )
            .await;
            Ok(BadgerEmptyResult::default())
        } else {
            Err(Error::Custom(String::from("No default app")))
        }
    }

    async fn on_pull_entity_info(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listening = request.listen;
        ProviderBroker::register_or_unregister_provider(
            &self.platform_state,
            FireboltCap::Short(ENTITY_INFO_CAPABILITY.into()).as_str(),
            String::from("entityInfo"),
            ENTITY_INFO_EVENT,
            ctx,
            request,
        )
        .await;
        Ok(ListenerResponse {
            listening,
            event: ENTITY_INFO_EVENT,
        })
    }

    async fn get_entity(
        &self,
        ctx: CallContext,
        entity_request: ContentEntityRequest,
    ) -> RpcResult<ContentEntityResponse> {
        let parameters = entity_request.parameters;
        let federated_options = entity_request.options.unwrap_or_default();
        let (session_tx, session_rx) = oneshot::channel::<ProviderResponsePayload>();
        let pr_msg = provider_broker::Request {
            app_id: Some(entity_request.provider.to_owned()),
            capability: FireboltCap::Short(ENTITY_INFO_CAPABILITY.into()).as_str(),
            method: String::from("entityInfo"),
            caller: ctx,
            request: ProviderRequestPayload::EntityInfoRequest(parameters),
            tx: session_tx,
        };
        ProviderBroker::invoke_method(&self.platform_state, pr_msg).await;
        let channel_result = timeout(
            Duration::from_millis(federated_options.timeout.into()),
            session_rx,
        )
        .await
        .map_err(|_| Error::Custom(String::from("Didn't receive response within time")))?;
        /*handle channel response*/
        let result = channel_result.map_err(|_| {
            Error::Custom(String::from(
                "Error returning back from entity response provider",
            ))
        })?;
        match result.as_entity_info_result() {
            Some(res) => Ok(ContentEntityResponse {
                provider: entity_request.provider.to_owned(),
                data: res.clone(),
            }),
            None => Err(Error::Custom(String::from(
                "Invalid response back from provider",
            ))),
        }
    }

    async fn handle_entity_info_result(
        &self,
        _ctx: CallContext,
        entity_info: ExternalProviderResponse<Option<EntityInfoResult>>,
    ) -> RpcResult<bool> {
        let response = ProviderResponse {
            correlation_id: entity_info.correlation_id,
            result: ProviderResponsePayload::EntityInfoResponse(entity_info.result),
        };
        ProviderBroker::provider_response(&self.platform_state, response).await;
        Ok(true)
    }

    async fn on_pull_purchased_content(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listening = request.listen;
        ProviderBroker::register_or_unregister_provider(
            &self.platform_state,
            FireboltCap::Short(PURCHASED_CONTENT_CAPABILITY.into()).as_str(),
            String::from("purchasedContent"),
            PURCHASED_CONTENT_EVENT,
            ctx,
            request,
        )
        .await;

        Ok(ListenerResponse {
            listening,
            event: ENTITY_INFO_EVENT,
        })
    }

    async fn get_providers(&self, _ctx: CallContext) -> RpcResult<Vec<ContentProvider>> {
        let res = ProviderBroker::get_provider_methods(&self.platform_state);
        let provider_list = self.convert_provider_result(res);
        Ok(provider_list)
    }

    async fn get_purchases(
        &self,
        ctx: CallContext,
        purchase_content_request: ProvidedPurchaseContentRequest,
    ) -> RpcResult<ProvidedPurchasedContentResult> {
        let parameters = purchase_content_request.parameters;
        let federated_options = purchase_content_request.options.unwrap_or_default();
        let (session_tx, session_rx) = oneshot::channel::<ProviderResponsePayload>();
        let pr_msg = provider_broker::Request {
            app_id: Some(purchase_content_request.provider.to_owned()),
            capability: FireboltCap::Short(PURCHASED_CONTENT_CAPABILITY.into()).as_str(),
            method: String::from("purchasedContent"),
            caller: ctx,
            request: ProviderRequestPayload::PurchasedContentRequest(parameters),
            tx: session_tx,
        };
        ProviderBroker::invoke_method(&self.platform_state, pr_msg).await;
        let channel_result = timeout(
            Duration::from_millis(federated_options.timeout.into()),
            session_rx,
        )
        .await
        .map_err(|_| Error::Custom(String::from("Didn't receive response within time")))?;
        /*handle channel response*/
        let result = channel_result.map_err(|_| {
            Error::Custom(String::from(
                "Error returning back from entity response provider",
            ))
        })?;
        match result.as_purchased_content_result() {
            Some(res) => Ok(ProvidedPurchasedContentResult {
                provider: purchase_content_request.provider.to_owned(),
                data: res.clone(),
            }),
            None => Err(Error::Custom(String::from(
                "Invalid response back from provider",
            ))),
        }
    }

    async fn handle_purchased_content_result(
        &self,
        _ctx: CallContext,
        purchased_content_result: ExternalProviderResponse<PurchasedContentResult>,
    ) -> RpcResult<bool> {
        let response = ProviderResponse {
            correlation_id: purchased_content_result.correlation_id,
            result: ProviderResponsePayload::PurchasedContentResponse(
                purchased_content_result.result,
            ),
        };
        ProviderBroker::provider_response(&self.platform_state, response).await;
        Ok(true)
    }

    async fn badger_entitlements_account_link(
        &self,
        ctx: CallContext,
        request: BadgerEntitlementsAccountLinkRequest,
    ) -> RpcResult<BadgerEmptyResult> {
        let mut sign_in_resp = Ok(true);
        let mut resp = Ok(EmptyResult::default());
        let mut entitlements_updated = false;

        let action = request
            .action
            .as_ref()
            .map(|x| x.as_str())
            .unwrap_or_default();

        match action {
            BADGER_ENTITLEMENT_SIGNIN => {
                // Sign In
                let fut = self.process_sign_in_request(ctx.clone(), true);
                // Entitlement update
                entitlements_updated = true;
                resp = self
                    .process_entitlement_update(
                        ctx.clone(),
                        request.subscription_entitlements.clone(),
                    )
                    .await;
                sign_in_resp = fut.await;
            }

            BADGER_ENTITLEMENT_SIGNOUT => {
                // Badger clearing entitlement before signing out.
                let fut = self.clear_content_access(ctx.clone());
                // Sign Out
                sign_in_resp = self.process_sign_in_request(ctx.clone(), false).await;
                resp = fut.await;
            }

            BADGER_ENTITLEMENT_APPLAUNCH => {
                // Entitlement update
                entitlements_updated = true;
                resp = self
                    .process_entitlement_update(
                        ctx.clone(),
                        request.subscription_entitlements.clone(),
                    )
                    .await;
            }
            _ => {}
        };

        if request.link_type.is_some() && !entitlements_updated {
            resp = self
                .process_entitlement_update(ctx.clone(), request.subscription_entitlements.clone())
                .await;
        }

        if sign_in_resp.is_ok() && resp.is_ok() {
            resp = Ok(BadgerEmptyResult::default());
        } else {
            resp = Err(rpc_downstream_service_err("Received error from Server"));
        }
        resp
    }

    async fn badger_launch_pad_account_link(
        &self,
        ctx: CallContext,
        launch_pad_tile_data: BadgerLaunchpadTile,
    ) -> RpcResult<BadgerEmptyResult> {
        let watch_next_info = WatchNextInfo {
            title: launch_pad_tile_data.launchpad_tile.titles.clone(),
            url: launch_pad_tile_data.launchpad_tile.url.clone(),
            identifiers: ContentIdentifiers {
                asset_id: None,
                entity_id: launch_pad_tile_data.launchpad_tile.content_id.clone(),
                season_id: None,
                series_id: None,
                app_content_data: None,
            },
            expires: launch_pad_tile_data
                .launchpad_tile
                .expiration
                .map(|x| convert_timestamp_to_iso8601(x)),
            images: launch_pad_tile_data.launchpad_tile.images.clone(),
        };

        let resp = self.watch_next(ctx, watch_next_info.clone()).await;

        match resp {
            Ok(_n) => Ok(BadgerEmptyResult::default()),
            Err(e) => Err(e),
        }
    }

    async fn badger_media_event_account_link(
        &self,
        ctx: CallContext,
        badger_media_event: BadgerMediaEvent,
    ) -> RpcResult<BadgerEmptyResult> {
        let progress = match (badger_media_event.event.progress_units).as_deref() {
            Some(BADGER_MEDIA_EVENT_PERCENT) => badger_media_event.event.progress / 100.00,
            Some(BADGER_MEDIA_EVENT_SECONDS) => badger_media_event.event.progress,
            _ => 0.0,
        };
        let watched_info = WatchedInfo {
            entity_id: badger_media_event.event.content_id.to_owned(),
            progress,
            completed: Some(badger_media_event.event.completed),
            watched_on: None,
        };

        let resp = self.watched(ctx, watched_info.clone()).await;

        match resp {
            Ok(_n) => Ok(BadgerEmptyResult::default()),
            Err(e) => Err(e),
        }
    }

    async fn discovery_content_access(
        &self,
        ctx: CallContext,
        request: ContentAccessRequest,
    ) -> RpcResult<()> {
        let resp = self.content_access(ctx, request).await;
        match resp {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    async fn discovery_clear_content_access(&self, ctx: CallContext) -> RpcResult<()> {
        let resp = self.clear_content_access(ctx).await;
        match resp {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }
}

pub struct DiscoveryRippleProvider;

pub struct DiscoveryCapHandler;

impl IGetLoadedCaps for DiscoveryCapHandler {
    fn get_loaded_caps(&self) -> RippleHandlerCaps {
        RippleHandlerCaps {
            caps: Some(vec![CapClassifiedRequest::Supported(vec![
                FireboltCap::Short("discovery:watched".into()),
                FireboltCap::Short("lifecycle:launch".into()),
                FireboltCap::Short("discovery:policy".into()),
                FireboltCap::Short("discovery:watch-next".into()),
                FireboltCap::Short("discovery:entitlements".into()),
                FireboltCap::Short("discovery:navigate-to".into()),
                FireboltCap::Short("discovery:sign-in-status".into()),
                FireboltCap::Short(ENTITY_INFO_CAPABILITY.into()),
                FireboltCap::Short(PURCHASED_CONTENT_CAPABILITY.into()),
            ])]),
        }
    }
}

impl RPCProvider<DiscoveryImpl<RippleHelper>, DiscoveryCapHandler> for DiscoveryRippleProvider {
    fn provide(
        self,
        rhf: Box<RippleHelperFactory>,
        platform_state: PlatformState,
    ) -> (RpcModule<DiscoveryImpl<RippleHelper>>, DiscoveryCapHandler) {
        let a = DiscoveryImpl {
            helper: rhf.clone().get(self.get_helper_variant()),
            platform_state: platform_state,
        };
        (a.into_rpc(), DiscoveryCapHandler)
    }

    fn get_helper_variant(self) -> Vec<RippleHelperType> {
        vec![
            RippleHelperType::AppManager,
            RippleHelperType::Cap,
            RippleHelperType::Dab,
            RippleHelperType::Dpab,
        ]
    }
}

#[cfg(test)]
mod tests {

    use serde_json::Value;
    use tokio::{
        sync::{mpsc, oneshot},
        task::JoinHandle,
    };

    use super::*;
    use crate::api::rpc::firebolt_gateway::tests::TestGateway;
    use crate::api::rpc::{
        api_messages::{ApiMessage, ApiProtocol},
        firebolt_gateway::tests::CallContextBuilder,
    };
    use crate::apps::app_auth_sessions::AppAuthSessions;
    use crate::apps::app_events::ListenRequest;
    use crate::apps::provider_broker::{ExternalProviderRequest, ExternalProviderResponse};
    use crate::device_manifest::DeviceManifest;
    use crate::helpers::ripple_helper::mock_ripple_helper::*;
    use crate::managers::config_manager::ConfigManager;
    use dab::core::{message::DabResponsePayload, model::distributor::DistributorSession};
    use dpab::core::{
        message::{DpabRequestPayload, DpabResponsePayload},
        model::discovery::{ContentAccessResponse, EntitlementsAccountLinkResponse},
    };
    use std::sync::{Arc, Mutex};
    use tracing::{debug, info};

    #[derive(Clone)]
    enum Events {
        EntityInfoEvent(EntityInfoResult),
        PurchasedContentEvent(PurchasedContentResult),
    }

    struct SampleApp {
        helper: Box<RippleHelper>,
        supported_events: Vec<Events>,
    }

    impl SampleApp {
        fn new(helper: Box<RippleHelper>, supported_events: Vec<Events>) -> Self {
            SampleApp {
                helper,
                supported_events,
            }
        }
        fn get_entity_info_result(hash: &Vec<Events>) -> Option<EntityInfoResult> {
            let mut ret_val: Option<EntityInfoResult> = None;
            for item in hash {
                if let Events::EntityInfoEvent(result) = item {
                    ret_val = Some(result.clone());
                }
            }
            ret_val
        }
        fn get_purchased_content_result(hash: &Vec<Events>) -> Option<PurchasedContentResult> {
            let mut ret_val: Option<PurchasedContentResult> = None;
            for item in hash {
                if let Events::PurchasedContentEvent(result) = item {
                    ret_val = Some(result.clone());
                }
            }
            ret_val
        }
        async fn start(
            &self,
            mut session_rx: mpsc::Receiver<ApiMessage>,
            platform_state: PlatformState,
        ) {
            let (provider_rdy_tx, provider_rdy_rx) = oneshot::channel();
            let provider_handler = DiscoveryImpl {
                helper: self.helper.clone(),
                platform_state: platform_state,
            };
            let ctx_provider = TestGateway::provider_call();
            let supported_events = self.supported_events.clone();
            tokio::task::spawn(async move {
                //Register for events
                for events in &supported_events {
                    match events {
                        Events::EntityInfoEvent(_) => {
                            let _listen_response = provider_handler
                                .on_pull_entity_info(
                                    ctx_provider.clone(),
                                    ListenRequest { listen: true },
                                )
                                .await
                                .unwrap();
                        }
                        Events::PurchasedContentEvent(_) => {
                            let _listen_response = provider_handler
                                .on_pull_purchased_content(
                                    ctx_provider.clone(),
                                    ListenRequest { listen: true },
                                )
                                .await
                                .unwrap();
                        }
                    }
                }
                let _ = provider_rdy_tx.send(());
                while let Some(message) = session_rx.recv().await {
                    let jsonrpc: Value =
                        serde_json::from_str(message.jsonrpc_msg.as_str()).unwrap();
                    if let Ok(req) = serde_json::from_value::<
                        ExternalProviderRequest<EntityInfoParameters>,
                    >(jsonrpc.get("result").unwrap().clone())
                    {
                        let result = SampleApp::get_entity_info_result(&supported_events).unwrap();

                        provider_handler
                            .handle_entity_info_result(
                                ctx_provider.clone(),
                                ExternalProviderResponse {
                                    correlation_id: req.correlation_id,
                                    result: Some(result),
                                },
                            )
                            .await
                            .unwrap();
                    } else if let Ok(req) = serde_json::from_value::<
                        ExternalProviderRequest<PurchasedContentParameters>,
                    >(
                        jsonrpc.get("result").unwrap().clone()
                    ) {
                        let result =
                            SampleApp::get_purchased_content_result(&supported_events).unwrap();

                        provider_handler
                            .handle_purchased_content_result(
                                ctx_provider.clone(),
                                ExternalProviderResponse {
                                    correlation_id: req.correlation_id,
                                    result: result,
                                },
                            )
                            .await
                            .unwrap();
                    }
                }
            });
            provider_rdy_rx.await.unwrap();
        }
    }

    #[tokio::test]
    async fn entity_info_test() {
        let pst = PlatformState::default();
        let test_gateway = TestGateway::start(pst.clone()).await;
        let mut result_set = Vec::new();
        let mut entity_info_result: EntityInfoResult = Default::default();
        let mut entity: EntityInfo = Default::default();
        entity.entity_type = "program".to_owned();
        entity.episode_number = Some(6);
        entity.season_number = Some(4);
        let mut content_identifiers: ContentIdentifiers = Default::default();
        content_identifiers.asset_id = Some("asset_001".to_owned());
        content_identifiers.entity_id = "entity_001".to_owned();
        entity.identifiers = content_identifiers;

        let mut ways_to_watch: WaysToWatch = Default::default();
        let mut content_identifiers2: ContentIdentifiers = Default::default();
        content_identifiers2.asset_id = Some("asset_002".to_owned());
        content_identifiers2.entity_id = "entity_002".to_owned();
        ways_to_watch.identifiers = content_identifiers2;
        //ways_to_watch.has_ads = Some(true);
        entity.ways_to_watch = Some(vec![ways_to_watch]);

        entity_info_result.entity = entity;
        entity_info_result.expires = "SomeExpiryDate".to_owned();
        result_set.push(Events::EntityInfoEvent(entity_info_result.clone()));

        let sample_app1 = SampleApp::new(test_gateway.pin_helper.clone(), result_set.clone());
        sample_app1
            .start(test_gateway.prov_session_rx, pst.clone())
            .await;

        let handler = DiscoveryImpl {
            helper: test_gateway.pin_helper,
            platform_state: pst,
        };
        let ctx_cons = TestGateway::consumer_call();
        let entity_request = ContentEntityRequest {
            provider: "app_provider".to_owned(),
            parameters: EntityInfoParameters {
                entity_id: "abcd1234".to_owned(),
                asset_id: None,
            },
            options: Some(FederationOptions { timeout: 1000 }),
        };
        let resp = handler
            .get_entity(ctx_cons.clone(), entity_request)
            .await
            .unwrap();
        debug!("entity_info_test: {:?}", serde_json::to_string(&resp));
        let resp_data = resp.data.unwrap();
        assert_eq!(
            resp_data.entity.episode_number,
            entity_info_result.entity.episode_number
        );
        assert_eq!(
            resp_data.entity.season_number,
            entity_info_result.entity.season_number
        );
        assert_eq!(
            resp_data.entity.identifiers.asset_id,
            entity_info_result.entity.identifiers.asset_id
        );
    }

    #[tokio::test]
    async fn purchased_content_test() {
        let pst = PlatformState::default();
        let test_gateway = TestGateway::start(pst.clone()).await;
        let mut result_set = Vec::new();
        let mut purchased_content_result: PurchasedContentResult = Default::default();
        purchased_content_result.expires = "ExpiryDate".to_owned();
        purchased_content_result.total_count = 1;
        let mut entity_info: EntityInfo = Default::default();
        entity_info.identifiers = Default::default();
        entity_info.identifiers.asset_id = Some("abcd1234".to_owned());
        entity_info.entity_type = "program".to_owned();
        entity_info.episode_number = Some(4);
        purchased_content_result.entries = vec![entity_info];
        result_set.push(Events::PurchasedContentEvent(
            purchased_content_result.clone(),
        ));

        let sample_app1 = SampleApp::new(test_gateway.pin_helper.clone(), result_set.clone());
        sample_app1
            .start(test_gateway.prov_session_rx, pst.clone())
            .await;

        let handler = DiscoveryImpl {
            helper: test_gateway.pin_helper.clone(),
            platform_state: pst,
        };
        let ctx_cons = TestGateway::consumer_call();
        let purchased_content_request = ProvidedPurchaseContentRequest {
            provider: "app_provider".to_owned(),
            parameters: PurchasedContentParameters {
                limit: 10,
                offering_type: None,
                program_type: None,
            },
            options: None,
        };
        let resp = handler
            .get_purchases(ctx_cons.clone(), purchased_content_request)
            .await
            .unwrap();
        assert_eq!(resp.data.total_count, purchased_content_result.total_count);
        assert_eq!(resp.data.entries.len(), 1);
        assert_eq!(
            resp.data.entries[0].episode_number,
            purchased_content_result.entries[0].episode_number
        );
    }

    #[tokio::test]
    async fn content_providers_test() {
        let pst = PlatformState::default();
        let test_gateway = TestGateway::start(pst.clone()).await;
        let mut result_set = Vec::new();
        let mut purchased_content_result: PurchasedContentResult = Default::default();
        purchased_content_result.expires = "ExpiryDate".to_owned();
        purchased_content_result.total_count = 1;
        let mut entity_info: EntityInfo = Default::default();
        entity_info.identifiers = Default::default();
        entity_info.identifiers.asset_id = Some("abcd1234".to_owned());
        entity_info.entity_type = "program".to_owned();
        entity_info.episode_number = Some(4);
        purchased_content_result.entries = vec![entity_info];
        result_set.push(Events::PurchasedContentEvent(
            purchased_content_result.clone(),
        ));
        let mut entity_info_result: EntityInfoResult = Default::default();
        let mut entity: EntityInfo = Default::default();
        entity.entity_type = "program".to_owned();
        entity.episode_number = Some(6);
        entity.season_number = Some(4);
        let mut content_identifiers: ContentIdentifiers = Default::default();
        content_identifiers.asset_id = Some("asset_001".to_owned());
        content_identifiers.entity_id = "entity_001".to_owned();
        entity.identifiers = content_identifiers;
        entity_info_result.entity = entity;
        entity_info_result.expires = "SomeExpiryDate".to_owned();
        result_set.push(Events::EntityInfoEvent(entity_info_result.clone()));

        let sample_app1 = SampleApp::new(test_gateway.pin_helper.clone(), result_set.clone());
        sample_app1
            .start(test_gateway.prov_session_rx, pst.clone())
            .await;

        let handler = DiscoveryImpl {
            helper: test_gateway.pin_helper,
            platform_state: pst,
        };
        let ctx_cons = TestGateway::consumer_call();
        let resp = handler.get_providers(ctx_cons).await.unwrap();
        assert_eq!(resp[0].apis.len(), 2);
        assert!(resp[0].apis.contains(&String::from("purchases")));
        assert!(resp[0].apis.contains(&String::from("entity")));
    }

    #[tokio::test]
    async fn test_entitlements() {
        let recorder = Arc::new(Mutex::new(RecordedCalls::default()));
        let (ripple_helper, state) = custom_setup(recorder.clone());

        let handler = DiscoveryImpl {
            helper: Box::new(ripple_helper.clone()),
            platform_state: state,
        };
        let ctx_cons = TestGateway::consumer_call();
        let entitlements_info = EntitlementsInfo {
            entitlements: vec![EntitlementData {
                entitlement_id: String::from("partner.com/entitlement/123"),
                start_time: Some(String::from("2021-04-23T18:25:43.511Z")),
                end_time: Some(String::from("2021-04-23T18:25:43.511Z")),
            }],
        };

        let resp = handler
            .entitlements(ctx_cons.clone(), entitlements_info.clone())
            .await
            .unwrap_or_default();
        info!("entitlements {:?}", resp);
        assert_eq!(recorder.lock().unwrap().dpab_calls.len(), 1);
        assert!(
            matches!(recorder.clone().lock().unwrap().dpab_calls.get(0), Some(payload) if matches!(&payload, DpabRequestPayload::Discovery(cmd) if matches!(&cmd, DiscoveryRequest::SetContentAccess(_))))
        );
    }

    fn custom_setup(recorder: Arc<Mutex<RecordedCalls>>) -> (RippleHelper, PlatformState) {
        let ripple_helper = MockRippleBuilder::new()
            .with_dab(vec![(
                &|_| {
                    Ok(DabResponsePayload::DistributorSessionResponse(
                        DistributorSession {
                            id: Some("someId".into()),
                            token: Some("someToken".into()),
                            account_id: Some("someAccountid".into()),
                            device_id: Some("someDeviceid".into()),
                        },
                    ))
                },
                1,
            )])
            .with_dpab(vec![(
                &|payload: DpabRequestPayload| match payload {
                    DpabRequestPayload::AccountLink(_) => {
                        Ok(DpabResponsePayload::EntitlementsAccountLink(
                            EntitlementsAccountLinkResponse {},
                        ))
                    }
                    DpabRequestPayload::Discovery(_) => {
                        Ok(DpabResponsePayload::ContentAccess(ContentAccessResponse {}))
                    }
                    _ => panic!(
                        "Not expected to handle requests other than [Account link, and discovery]"
                    ),
                },
                2,
            )])
            .with_recorder(recorder.clone())
            .build();
        let mut state = PlatformState::new(ripple_helper.clone());
        state.app_auth_sessions = AppAuthSessions::new_with_helper(ripple_helper.clone());
        (ripple_helper, state)
    }
    #[tokio::test]
    async fn test_sign_in() {
        let recorder = Arc::new(Mutex::new(RecordedCalls::default()));
        let (ripple_helper, state) = custom_setup(recorder.clone());
        let handler = DiscoveryImpl {
            helper: Box::new(ripple_helper.clone()),
            platform_state: state,
        };
        let ctx_cons = TestGateway::consumer_call();
        let sign_in_info = SignInInfo {
            entitlements: Some(vec![EntitlementData {
                entitlement_id: String::from("partner.com/entitlement/123"),
                start_time: Some(String::from("2021-04-23T18:25:43.511Z")),
                end_time: Some(String::from("2021-04-23T18:25:43.511Z")),
            }]),
        };

        let resp = handler
            .sign_in(ctx_cons.clone(), sign_in_info.clone())
            .await
            .unwrap_or_default();
        info!("sign_in {:?}", resp);
        assert_eq!(recorder.lock().unwrap().dpab_calls.len(), 2);
    }

    #[tokio::test]
    async fn test_sign_out() {
        let recorder = Arc::new(Mutex::new(RecordedCalls::default()));
        let (ripple_helper, state) = custom_setup(recorder.clone());
        let handler = DiscoveryImpl {
            helper: Box::new(ripple_helper.clone()),
            platform_state: state,
        };
        let ctx_cons = TestGateway::consumer_call();

        let resp = handler.sign_out(ctx_cons.clone()).await.unwrap_or_default();
        info!("sign_out {:?}", resp);
        assert_eq!(recorder.lock().unwrap().dpab_calls.len(), 1);
        assert!(
            matches!(recorder.clone().lock().unwrap().dpab_calls.get(0), Some(payload) if matches!(&payload, DpabRequestPayload::AccountLink(cmd) if matches!(&cmd, DiscoveryAccountLinkRequest::SignIn(_))))
        );
    }

    #[tokio::test]
    async fn test_watched() {
        let state = PlatformState::default();
        let test_gateway = TestGateway::start(state.clone()).await;

        let result_set = Vec::new();
        let test_app = SampleApp::new(test_gateway.pin_helper.clone(), result_set.clone());
        test_app
            .start(test_gateway.prov_session_rx, state.clone())
            .await;

        let handler = DiscoveryImpl {
            helper: test_gateway.pin_helper,
            platform_state: state,
        };
        let ctx_cons = TestGateway::consumer_call();
        let watched_info = WatchedInfo {
            entity_id: "partner.com/entity/123".to_owned(),
            progress: 1.25,
            completed: Some(true),
            watched_on: Some("2022-04-23T18:25:43.511Z".to_owned()),
        };

        let resp = handler
            .watched(ctx_cons.clone(), watched_info.clone())
            .await
            .unwrap_or_default();
        info!("watched {:?}", resp);
    }

    #[tokio::test]
    async fn test_watch_next() {
        let state = PlatformState::default();
        let test_gateway = TestGateway::start(state.clone()).await;

        let result_set = Vec::new();
        let test_app = SampleApp::new(test_gateway.pin_helper.clone(), result_set.clone());
        test_app
            .start(test_gateway.prov_session_rx, state.clone())
            .await;

        let handler = DiscoveryImpl {
            helper: test_gateway.pin_helper,
            platform_state: state,
        };
        let ctx_cons = TestGateway::consumer_call();
        let watch_next_info = WatchNextInfo {
            title: Some(LocalizedString::Simple(String::from("A Cool Show"))),
            url: None,
            identifiers: ContentIdentifiers {
                asset_id: None,
                entity_id: "entity_001".to_owned(),
                season_id: None,
                series_id: None,
                app_content_data: None,
            },
            expires: None,
            images: None,
        };

        let resp = handler
            .watch_next(ctx_cons.clone(), watch_next_info.clone())
            .await
            .unwrap_or_default();
        info!("watched {:?}", resp);
    }
    #[tokio::test]
    async fn test_get_content_partner_id() {
        let state = PlatformState::default();
        let test_gateway = TestGateway::start(state.clone()).await;

        let result_set = Vec::new();
        let test_app = SampleApp::new(test_gateway.pin_helper.clone(), result_set.clone());
        test_app
            .start(test_gateway.prov_session_rx, state.clone())
            .await;

        let ctx_cons = TestGateway::consumer_call();
        let ripple_helper = MockRippleBuilder::new()
            .with_dab(vec![])
            .with_caps(vec![])
            .build();

        let content_partner_id = get_content_partner_id(&ripple_helper.clone(), &ctx_cons)
            .await
            .unwrap_or_default();
        assert_eq!(content_partner_id, "".to_string());
    }
    #[tokio::test]
    async fn test_badger_entitlements_account_link_sign_in() {
        let recorder = Arc::new(Mutex::new(RecordedCalls::default()));
        let (ripple_helper, state) = custom_setup(recorder.clone());

        let handler = DiscoveryImpl {
            helper: Box::new(ripple_helper.clone()),
            platform_state: state,
        };
        let ctx_cons = TestGateway::consumer_call();
        let request = BadgerEntitlementsAccountLinkRequest {
            action: Some("signIn".to_string()),
            link_type: None,
            subscription_entitlements: Some(vec![SubscriptionEntitlements {
                id: String::from("Prime"),
                start_date: Some(123456789),
                end_date: Some(132456789),
            }]),
        };

        let resp = handler
            .badger_entitlements_account_link(ctx_cons.clone(), request.clone())
            .await
            .unwrap_or_default();
        info!("badger_entitlements_account_link {:?}", resp);
        assert_eq!(recorder.lock().unwrap().dpab_calls.len(), 2);
    }

    #[tokio::test]
    async fn test_badger_entitlements_account_link_sign_out() {
        let recorder = Arc::new(Mutex::new(RecordedCalls::default()));
        let (ripple_helper, state) = custom_setup(recorder.clone());

        let handler = DiscoveryImpl {
            helper: Box::new(ripple_helper.clone()),
            platform_state: state,
        };
        let ctx_cons = TestGateway::consumer_call();
        let request = BadgerEntitlementsAccountLinkRequest {
            action: Some("signOut".to_string()),
            link_type: None,
            subscription_entitlements: None,
        };

        let resp = handler
            .badger_entitlements_account_link(ctx_cons.clone(), request.clone())
            .await
            .unwrap_or_default();
        info!("badger_entitlements_account_link {:?}", resp);
        assert_eq!(recorder.lock().unwrap().dpab_calls.len(), 2);
    }

    #[tokio::test]
    async fn test_badger_entitlements_account_link_app_launch() {
        let recorder = Arc::new(Mutex::new(RecordedCalls::default()));
        let (ripple_helper, state) = custom_setup(recorder.clone());

        let handler = DiscoveryImpl {
            helper: Box::new(ripple_helper.clone()),
            platform_state: state,
        };
        let ctx_cons = TestGateway::consumer_call();
        let request = BadgerEntitlementsAccountLinkRequest {
            action: Some("appLaunch".to_string()),
            link_type: None,
            subscription_entitlements: Some(vec![SubscriptionEntitlements {
                id: String::from("Prime"),
                start_date: None,
                end_date: None,
            }]),
        };

        let resp = handler
            .badger_entitlements_account_link(ctx_cons.clone(), request.clone())
            .await
            .unwrap_or_default();
        info!("badger_entitlements_account_link {:?}", resp);
        assert_eq!(recorder.lock().unwrap().dpab_calls.len(), 1);
        assert!(
            matches!(recorder.clone().lock().unwrap().dpab_calls.get(0), Some(payload) if matches!(&payload, DpabRequestPayload::Discovery(cmd) if matches!(&cmd, DiscoveryRequest::SetContentAccess(_))))
        );
    }

    #[tokio::test]
    async fn test_badger_entitlements_account_link_entitlements_update() {
        let recorder = Arc::new(Mutex::new(RecordedCalls::default()));
        let (ripple_helper, state) = custom_setup(recorder.clone());

        let handler = DiscoveryImpl {
            helper: Box::new(ripple_helper.clone()),
            platform_state: state,
        };
        let ctx_cons = TestGateway::consumer_call();
        let request = BadgerEntitlementsAccountLinkRequest {
            action: None,
            link_type: Some("entitlementsUpdate".to_string()),
            subscription_entitlements: Some(vec![
                SubscriptionEntitlements {
                    id: String::from("Content_id_1"),
                    start_date: Some(123456789),
                    end_date: None,
                },
                SubscriptionEntitlements {
                    id: String::from("Content_id_2"),
                    start_date: Some(123456789),
                    end_date: None,
                },
                SubscriptionEntitlements {
                    id: String::from("Content_id_3"),
                    start_date: Some(123456789),
                    end_date: None,
                },
            ]),
        };

        let resp = handler
            .badger_entitlements_account_link(ctx_cons.clone(), request.clone())
            .await
            .unwrap_or_default();
        info!("badger_entitlements_account_link {:?}", resp);
        assert_eq!(recorder.lock().unwrap().dpab_calls.len(), 1);
        assert!(
            matches!(recorder.clone().lock().unwrap().dpab_calls.get(0), Some(payload) if matches!(&payload, DpabRequestPayload::Discovery(cmd) if matches!(&cmd, DiscoveryRequest::SetContentAccess(_))))
        );
    }

    #[tokio::test]
    async fn test_badger_launch_pad_account_link_create() {
        let state = PlatformState::default();
        let test_gateway = TestGateway::start(state.clone()).await;

        let result_set = Vec::new();
        let test_app = SampleApp::new(test_gateway.pin_helper.clone(), result_set.clone());
        test_app
            .start(test_gateway.prov_session_rx, state.clone())
            .await;

        let handler = DiscoveryImpl {
            helper: test_gateway.pin_helper,
            platform_state: state,
        };
        let ctx_cons = TestGateway::consumer_call();
        let launch_pad_tile_data = BadgerLaunchpadTile {
            launchpad_tile: BadgerLaunchpadTileData {
                titles: Some(LocalizedString::Simple(String::from("A Cool Show"))),
                url: Some("https://www.youtube.com/watch?v=b8fjxn8Kgg4".to_string()),
                expiration: None,
                content_id: "entity_001".to_owned(),
                images: None,
            },
        };

        let resp = handler
            .badger_launch_pad_account_link(ctx_cons.clone(), launch_pad_tile_data.clone())
            .await
            .unwrap_or_default();
        info!("badger_launch_pad_account_link {:?}", resp);
    }

    #[tokio::test]
    async fn test_badger_launch_pad_account_link_delete() {
        let state = PlatformState::default();
        let test_gateway = TestGateway::start(state.clone()).await;

        let result_set = Vec::new();
        let test_app = SampleApp::new(test_gateway.pin_helper.clone(), result_set.clone());
        test_app
            .start(test_gateway.prov_session_rx, state.clone())
            .await;

        let handler = DiscoveryImpl {
            helper: test_gateway.pin_helper,
            platform_state: state,
        };
        let ctx_cons = TestGateway::consumer_call();
        let launch_pad_tile_data = BadgerLaunchpadTile {
            launchpad_tile: BadgerLaunchpadTileData {
                titles: None,
                url: None,
                expiration: None,
                content_id: "entity_001".to_owned(),
                images: None,
            },
        };

        let resp = handler
            .badger_launch_pad_account_link(ctx_cons.clone(), launch_pad_tile_data.clone())
            .await
            .unwrap_or_default();
        info!("badger_launch_pad_account_link {:?}", resp);
    }

    #[tokio::test]
    async fn test_badger_media_event_account_link() {
        let state = PlatformState::default();
        let test_gateway = TestGateway::start(state.clone()).await;

        let empty_object = BadgerEmptyResult::default();
        let v = serde_json::to_value(&empty_object).unwrap();
        info!("v {:?}", v);

        let result_set = Vec::new();
        let test_app = SampleApp::new(test_gateway.pin_helper.clone(), result_set.clone());
        test_app
            .start(test_gateway.prov_session_rx, state.clone())
            .await;

        let handler = DiscoveryImpl {
            helper: test_gateway.pin_helper,
            platform_state: state,
        };
        let ctx_cons = TestGateway::consumer_call();
        let badger_media_event = BadgerMediaEvent {
            event: BadgerMediaEventData {
                content_id: String::from("12345678"),
                completed: true,
                progress: 5400 as f32,
                progress_units: Some("seconds".to_string()),
            },
        };

        let resp = handler
            .badger_media_event_account_link(ctx_cons.clone(), badger_media_event.clone())
            .await
            .unwrap_or_default();
        info!("badger_media_event_account_link {:?}", resp);
    }

    #[tokio::test]
    async fn test_badger_navigate_to_company_page() {
        let recorder = Arc::new(Mutex::new(RecordedCalls::default()));
        let (ripple_helper, state) = custom_setup(recorder.clone());

        let handler = DiscoveryImpl {
            helper: Box::new(ripple_helper.clone()),
            platform_state: state,
        };
        let ctx_cons = TestGateway::consumer_call();
        let req = NavigateCompanyPageRequest {
            company_id: "123".to_string(),
        };
        let resp = handler
            .badger_navigate_to_company_page(ctx_cons.clone(), req.clone())
            .await
            .unwrap_or_default();
        info!("badger_navigate_to_company_page {:?} ", resp);
        assert_eq!(resp, BadgerEmptyResult::default());
    }

    #[tokio::test]
    async fn test_content_access() {
        let rh = MockRippleBuilder::new()
            .with_dab(vec![(
                &|_| {
                    Ok(DabResponsePayload::DistributorSessionResponse(
                        DistributorSession {
                            id: Some("Id".into()),
                            token: Some("Token".into()),
                            account_id: Some("Accountid".into()),
                            device_id: Some("Deviceid".into()),
                        },
                    ))
                },
                1,
            )])
            .with_dpab(vec![(
                &|_| Ok(DpabResponsePayload::ContentAccess(ContentAccessResponse {})),
                1,
            )])
            .build();

        let request = ContentAccessRequest {
            ids: ContentAccessIdentifiers {
                availabilities: Some(vec![Availability {
                    _type: ContentType::ProgramLineup,
                    id: "partner.com/availability/123".to_string(),
                    catalog_id: None,
                    start_time: Some("2021-04-23T18:25:43.511Z".to_string()),
                    end_time: Some("2021-04-23T18:25:43.511Z".to_string()),
                }]),
                entitlements: Some(vec![EntitlementData {
                    entitlement_id: "test".to_string(),
                    start_time: None,
                    end_time: None,
                }]),
            },
        };

        let mut ps = PlatformState::new(rh.clone());
        ps.app_auth_sessions = AppAuthSessions::new_with_helper(rh.clone());

        let resp = DiscoveryImpl {
            helper: Box::new(rh),
            platform_state: ps,
        }
        .discovery_content_access(
            TestGateway::consumer_call_for_method("device.version"),
            request,
        )
        .await;
        assert!(&resp.is_ok());
    }
    #[tokio::test]
    async fn test_clear_content_access() {
        let rh = MockRippleBuilder::new()
            .with_dab(vec![(
                &|_| {
                    Ok(DabResponsePayload::DistributorSessionResponse(
                        DistributorSession {
                            id: Some("Id".into()),
                            token: Some("Token".into()),
                            account_id: Some("Accountid".into()),
                            device_id: Some("Deviceid".into()),
                        },
                    ))
                },
                1,
            )])
            .with_dpab(vec![(
                &|_| Ok(DpabResponsePayload::ContentAccess(ContentAccessResponse {})),
                1,
            )])
            .build();

        let mut ps = PlatformState::new(rh.clone());
        ps.app_auth_sessions = AppAuthSessions::new_with_helper(rh.clone());

        let resp = DiscoveryImpl {
            helper: Box::new(rh),
            platform_state: ps,
        }
        .discovery_clear_content_access(TestGateway::consumer_call())
        .await;
        assert!(&resp.is_ok());
    }

    async fn test_start_message_handler(
        mut session_rx: mpsc::Receiver<ApiMessage>,
    ) -> JoinHandle<()> {
        let join_handle = tokio::spawn(async move {
            debug!("Waiting for message");
            loop {
                let result = session_rx.recv().await;
                match result {
                    Some(message) => {
                        debug!("Got message {:?}", message.jsonrpc_msg);
                        let _jsonrpc: Value =
                            serde_json::from_str(message.jsonrpc_msg.as_str()).unwrap();
                        dbg!(&_jsonrpc);
                    }
                    None => {
                        debug!("Exited from message loop");
                        break;
                    }
                }
            }
        });
        join_handle
    }

    #[tokio::test]
    async fn test_launch_using_reserved_apps() {
        let mut manifest =
            DeviceManifest::load("src/apps/test/test-device-manifest.json".into()).unwrap();
        let rh = MockRippleBuilder::new()
            .with_config_manager(*ConfigManager::new(manifest.1, vec![]))
            .build();
        let ps = PlatformState::new(rh.clone());

        let discovery = DiscoveryImpl {
            helper: Box::new(rh),
            platform_state: ps.clone(),
        };

        let test_ctx = CallContextBuilder::new().app_id("test_player").build();
        let session_id = test_ctx.session_id.clone();
        let (session_tx, session_rx) = mpsc::channel::<ApiMessage>(32);
        let join_handle = test_start_message_handler(session_rx).await;

        AppEvents::register_session(&ps.clone().app_events_state, session_id.clone(), session_tx);

        let _resp = discovery
            .on_navigate_to(test_ctx.clone(), ListenRequest { listen: true })
            .await;

        let resp = discovery
            .launch(
                test_ctx.clone(),
                LaunchRequest {
                    app_id: "xrn:firebolt:application-type:player".to_owned(),
                    intent: Some(NavigationIntent::Playback(PlaybackIntent {
                        data: PlaybackIntentData::Movie(MovieEntity {
                            base_entity: BaseEntity {
                                entity_type: ProgramEntityType::default(),
                                entity_id: "222".to_owned(),
                                asset_id: None,
                                app_content_data: None,
                            },
                        }),
                        context: Context {
                            source: "voice".to_owned(),
                        },
                    })),
                },
            )
            .await;
        assert!(resp.is_ok() && resp.unwrap());

        // Cleanup.
        AppEvents::remove_session(&ps.clone().app_events_state, session_id.clone());
        join_handle.await.unwrap_or_default();
    }

    #[tokio::test]
    async fn test_launch_with_home_intent() {
        let mut manifest =
            DeviceManifest::load("src/apps/test/test-device-manifest.json".into()).unwrap();
        let rh = MockRippleBuilder::new()
            .with_config_manager(*ConfigManager::new(manifest.1, vec![]))
            .build();
        let ps = PlatformState::new(rh.clone());

        let discovery = DiscoveryImpl {
            helper: Box::new(rh),
            platform_state: ps.clone(),
        };
        let test_ctx = CallContextBuilder::new().app_id("test_player").build();
        let session_id = test_ctx.session_id.clone();

        let (session_tx, session_rx) = mpsc::channel::<ApiMessage>(32);
        let join_handle = test_start_message_handler(session_rx).await;

        AppEvents::register_session(&ps.clone().app_events_state, session_id.clone(), session_tx);

        let _resp = discovery
            .on_navigate_to(test_ctx.clone(), ListenRequest { listen: true })
            .await;

        let resp = discovery
            .launch(
                test_ctx.clone(),
                LaunchRequest {
                    app_id: "xrn:firebolt:application-type:player".to_owned(),
                    intent: Some(NavigationIntent::Home(HomeIntent {
                        context: Context {
                            source: "voice".into(),
                        },
                    })),
                },
            )
            .await;
        assert!(resp.is_ok() && resp.unwrap());

        // Cleanup.
        AppEvents::remove_session(&ps.clone().app_events_state, session_id.clone());
        join_handle.await.unwrap_or_default();
    }
}
