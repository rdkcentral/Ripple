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

use std::{collections::HashMap, time::Duration};

use crate::{
    firebolt::rpc::RippleRPCProvider,
    service::{
        apps::{
            app_events::{AppEventDecorationError, AppEventDecorator, AppEvents},
            provider_broker::{self, ProviderBroker},
        },
        data_governance::DataGovernance,
    },
    utils::rpc_utils::{
        rpc_await_oneshot, rpc_downstream_service_err, rpc_err, rpc_navigate_reserved_app_err,
    },
};
use jsonrpsee::{
    core::{async_trait, Error, RpcResult},
    proc_macros::rpc,
    RpcModule,
};

use ripple_sdk::{
    api::{
        apps::{AppError, AppManagerResponse, AppMethod, AppRequest, AppResponse},
        config::Config,
        firebolt::{
            fb_capabilities::FireboltCap,
            fb_discovery::{
                ClearContentSetParams, ContentAccessRequest, EntitlementsInfo, LaunchRequest,
                LocalizedString, SignInInfo, SignInRequestParams, WatchNextInfo, WatchedInfo,
                DISCOVERY_EVENT_ON_NAVIGATE_TO, ENTITY_INFO_CAPABILITY, ENTITY_INFO_EVENT,
                EVENT_DISCOVERY_POLICY_CHANGED, PURCHASED_CONTENT_CAPABILITY,
                PURCHASED_CONTENT_EVENT,
            },
            provider::{ProviderRequestPayload, ProviderResponse, ProviderResponsePayload},
        },
    },
    extn::extn_client_message::ExtnResponse,
    log::{error, info},
    tokio::{sync::oneshot, time::timeout},
};
use ripple_sdk::{
    api::{
        device::entertainment_data::*,
        distributor::{
            distributor_discovery::{DiscoveryRequest, MediaEventRequest},
            distributor_privacy::DataEventType,
        },
        firebolt::{
            fb_discovery::{
                ContentAccessAvailability, ContentAccessEntitlement, ContentAccessInfo,
                ContentAccessListSetParams, MediaEvent, MediaEventsAccountLinkRequestParams,
                ProgressUnit, SessionParams,
            },
            fb_general::{ListenRequest, ListenerResponse},
            provider::ExternalProviderResponse,
        },
        gateway::rpc_gateway_api::CallContext,
    },
    log::debug,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::state::platform_state::PlatformState;

#[derive(Default, Serialize, Debug)]
pub struct DiscoveryEmptyResult {
    //Empty object to take care of OTTX-28709
}

impl PartialEq for DiscoveryEmptyResult {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}
type EmptyResult = DiscoveryEmptyResult;

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
    pub state: PlatformState,
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
        _ctx: &CallContext,
        _state: &PlatformState,
        _app_id: &str,
    ) -> RpcResult<ContentPolicy> {
        let mut content_policy: ContentPolicy = Default::default();
        content_policy.enable_recommendations = false; // TODO: Need to replace with PrivacyImpl
        content_policy.share_watch_history = false; // TODO: Need to replace with PrivacyImpl
        content_policy.remember_watched_programs = false; // TODO: Need to replace with PrivacyImpl
        Ok(content_policy)
    }

    pub fn get_share_watch_history() -> bool {
        false
    }

    async fn _get_titles_from_localized_string(
        &self,
        title: &LocalizedString,
    ) -> HashMap<String, String> {
        let mut title_map = HashMap::new();
        let result = self
            .state
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

    async fn process_sign_in_request(
        &self,
        ctx: CallContext,
        is_signed_in: bool,
    ) -> RpcResult<bool> {
        let session = self.state.session_state.get_account_session().unwrap();

        let payload = DiscoveryRequest::SignIn(SignInRequestParams {
            session_info: SessionParams {
                app_id: ctx.clone().app_id.to_owned(),
                dist_session: session,
            },
            is_signed_in,
        });

        let resp = self.state.get_client().send_extn_request(payload).await;

        if let Err(_) = resp {
            error!("Error: Notifying SignIn info to the platform: {:?}", resp);
            return Err(rpc_downstream_service_err(
                "Error: Notifying SignIn info to the platform",
            ));
        }

        match resp.unwrap().payload.extract().unwrap() {
            ExtnResponse::None(()) => Ok(true),
            _ => Err(rpc_downstream_service_err(
                "Did not receive a valid resposne from platform when notifying Content Access List",
            )),
        }
    }

    async fn content_access(
        &self,
        ctx: CallContext,
        request: ContentAccessRequest,
    ) -> RpcResult<EmptyResult> {
        let session = self.state.session_state.get_account_session().unwrap();

        // If both entitlement & availability are None return EmptyResult
        if request.ids.availabilities.is_none() && request.ids.entitlements.is_none() {
            return Ok(EmptyResult::default());
        }

        let payload = DiscoveryRequest::SetContentAccess(ContentAccessListSetParams {
            session_info: SessionParams {
                app_id: ctx.app_id.to_owned(),
                dist_session: session,
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
        });

        let resp = self.state.get_client().send_extn_request(payload).await;
        if let Err(_) = resp {
            error!(
                "Error: Notifying Content AccessList to the platform: {:?}",
                resp
            );
            return Err(rpc_downstream_service_err(
                "Could not notify Content AccessList to the platform",
            ));
        }

        match resp.unwrap().payload.extract().unwrap() {
            ExtnResponse::None(()) => Ok(EmptyResult::default()),
            _ => Err(rpc_downstream_service_err(
                "Did not receive a valid resposne from platform when notifying Content Access List",
            )),
        }
    }

    async fn clear_content_access(&self, ctx: CallContext) -> RpcResult<EmptyResult> {
        let session = self.state.session_state.get_account_session().unwrap();

        let payload = DiscoveryRequest::ClearContent(ClearContentSetParams {
            session_info: SessionParams {
                app_id: ctx.app_id.to_owned(),
                dist_session: session,
            },
        });

        let resp = self.state.get_client().send_extn_request(payload).await;

        if let Err(_) = resp {
            error!(
                "Error: Clearing the Content AccessList to the platform: {:?}",
                resp
            );
            return Err(rpc_downstream_service_err(
                "Could not clear Content AccessList from the platform",
            ));
        }

        //TODO:ExtnResppnse
        match resp.unwrap().payload.extract().unwrap() {
            ExtnResponse::None(()) => Ok(EmptyResult::default()),
            _ => Err(rpc_downstream_service_err(
                "Did not receive a valid resposne from platform when clearing the Content Access List",
            )),
                }

        // match resp.unwrap() {
        //     ExtnRespons::ContentAccess(_obj) => Ok(EmptyResult::default()),
        //     _ => Err(rpc_downstream_service_err(
        //         "Did not receive a valid resposne from platform when clearing the Content Access List",
        //     )),
        // }
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
        _val_in: &Value,
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
impl DiscoveryServer for DiscoveryImpl {
    async fn on_policy_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        AppEvents::add_listener_with_decorator(
            &self.state,
            EVENT_DISCOVERY_POLICY_CHANGED.to_string(),
            ctx,
            request,
            Some(Box::new(DiscoveryPolicyEventDecorator {})),
        );

        Ok(ListenerResponse {
            listening: listen,
            event: EVENT_DISCOVERY_POLICY_CHANGED.to_string(),
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
            DataGovernance::resolve_tags(&self.state, ctx.app_id.clone(), DataEventType::Watched)
                .await;
        debug!("drop_all={:?} data_tags={:?}", drop_data, data_tags);
        if drop_data {
            return Ok(false);
        }
        let progress = watched_info.progress;
        if let Some(dist_session) = self.state.session_state.get_account_session() {
            let request =
                MediaEventRequest::MediaEventAccountLink(MediaEventsAccountLinkRequestParams {
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
                    content_partner_id: get_content_partner_id(&self.state, &ctx)
                        .await
                        .unwrap_or(ctx.app_id.to_owned()),
                    client_supports_opt_out: DiscoveryImpl::get_share_watch_history(),
                    dist_session,
                    data_tags,
                });

            if let Ok(resp) = self.state.get_client().send_extn_request(request).await {
                if let Some(_) = resp.payload.extract::<ExtnResponse>() {
                    return Ok(true);
                }
            }
        }

        Err(rpc_err(
            "Did not receive a valid resposne from platform when notifying watched info",
        ))
    }
    async fn watch_next(
        &self,
        ctx: CallContext,
        watch_next_info: WatchNextInfo,
    ) -> RpcResult<bool> {
        info!("Discovery.watchNext");
        let watched_info = WatchedInfo {
            entity_id: watch_next_info.identifiers.entity_id.unwrap(),
            progress: 1.0,
            completed: Some(false),
            watched_on: None,
        };
        return self.watched(ctx, watched_info.clone()).await;
    }

    async fn get_content_policy_rpc(&self, ctx: CallContext) -> RpcResult<ContentPolicy> {
        DiscoveryImpl::get_content_policy(&ctx, &self.state, &ctx.app_id).await
    }

    async fn launch(&self, _ctx: CallContext, request: LaunchRequest) -> RpcResult<bool> {
        let app_defaults_configuration = self.state.get_device_manifest().applications.defaults;

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
                &self.state,
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
                &self.state,
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

        if let Ok(_) = self.state.get_client().send_app_request(app_request) {
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
            &&self.state,
            DISCOVERY_EVENT_ON_NAVIGATE_TO.into(),
            ctx,
            request,
        );
        Ok(ListenerResponse {
            listening: listen,
            event: DISCOVERY_EVENT_ON_NAVIGATE_TO.into(),
        })
    }

    async fn on_pull_entity_info(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listening = request.listen;
        ProviderBroker::register_or_unregister_provider(
            &self.state,
            FireboltCap::Short(ENTITY_INFO_CAPABILITY.into()).as_str(),
            String::from("entityInfo"),
            ENTITY_INFO_EVENT,
            ctx,
            request,
        )
        .await;
        Ok(ListenerResponse {
            listening,
            event: ENTITY_INFO_EVENT.to_string(),
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
        let pr_msg = provider_broker::ProviderBrokerRequest {
            app_id: Some(entity_request.provider.to_owned()),
            capability: FireboltCap::Short(ENTITY_INFO_CAPABILITY.into()).as_str(),
            method: String::from("entityInfo"),
            caller: ctx,
            request: ProviderRequestPayload::EntityInfoRequest(parameters),
            tx: session_tx,
        };
        ProviderBroker::invoke_method(&self.state, pr_msg).await;
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
        ProviderBroker::provider_response(&self.state, response).await;
        Ok(true)
    }

    async fn on_pull_purchased_content(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listening = request.listen;
        ProviderBroker::register_or_unregister_provider(
            &self.state,
            FireboltCap::Short(PURCHASED_CONTENT_CAPABILITY.into()).as_str(),
            String::from("purchasedContent"),
            PURCHASED_CONTENT_EVENT,
            ctx,
            request,
        )
        .await;

        Ok(ListenerResponse {
            listening,
            event: ENTITY_INFO_EVENT.to_string(),
        })
    }

    async fn get_providers(&self, _ctx: CallContext) -> RpcResult<Vec<ContentProvider>> {
        let res = ProviderBroker::get_provider_methods(&self.state);
        let provider_list = self.convert_provider_result(ProviderResult::new(res.entries));
        Ok(provider_list)
    }

    async fn get_purchases(
        &self,
        ctx: CallContext,
        entity_request: ProvidedPurchaseContentRequest,
    ) -> RpcResult<ProvidedPurchasedContentResult> {
        let parameters = entity_request.parameters;
        let federated_options = entity_request.options.unwrap_or_default();
        let (session_tx, session_rx) = oneshot::channel::<ProviderResponsePayload>();
        let pr_msg = provider_broker::ProviderBrokerRequest {
            app_id: Some(entity_request.provider.to_owned()),
            capability: FireboltCap::Short(PURCHASED_CONTENT_CAPABILITY.into()).as_str(),
            method: String::from("purchasedContent"),
            caller: ctx,
            request: ProviderRequestPayload::PurchasedContentRequest(parameters),
            tx: session_tx,
        };
        ProviderBroker::invoke_method(&self.state, pr_msg).await;
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
                provider: entity_request.provider.to_owned(),
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
        entity_info: ExternalProviderResponse<PurchasedContentResult>,
    ) -> RpcResult<bool> {
        let response = ProviderResponse {
            correlation_id: entity_info.correlation_id,
            result: ProviderResponsePayload::PurchasedContentResponse(entity_info.result),
        };
        ProviderBroker::provider_response(&self.state, response).await;
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

pub struct DiscoveryRPCProvider;
impl RippleRPCProvider<DiscoveryImpl> for DiscoveryRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<DiscoveryImpl> {
        (DiscoveryImpl { state }).into_rpc()
    }
}
