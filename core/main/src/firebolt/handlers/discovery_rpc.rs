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
        DiscoveryAccountLinkRequest, DiscoveryRequest, MediaEvent,
        MediaEventsAccountLinkRequestParams, ProgressUnit, SessionParams, SignInRequestParams,
        WatchHistoryAccountLinkRequestParams, ACCOUNT_LINK_ACTION_CREATE,
        ACCOUNT_LINK_ACTION_DELETE, ACCOUNT_LINK_TYPE_LAUNCH_PAD,
    },
};

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

