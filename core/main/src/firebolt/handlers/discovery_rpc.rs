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

use std::time::Duration;

use crate::{
    firebolt::handlers::privacy_rpc::PrivacyImpl,
    firebolt::rpc::RippleRPCProvider,
    service::apps::{
        app_events::{AppEventDecorationError, AppEventDecorator, AppEvents},
        provider_broker::{self, ProviderBroker},
    },
    utils::rpc_utils::{rpc_await_oneshot, rpc_err, rpc_navigate_reserved_app_err},
};
use jsonrpsee::{
    core::{async_trait, Error, RpcResult},
    proc_macros::rpc,
    RpcModule,
};

use ripple_sdk::{
    api::{
        apps::{AppError, AppManagerResponse, AppMethod, AppRequest, AppResponse},
        firebolt::{
            fb_capabilities::FireboltCap,
            fb_discovery::{
                LaunchRequest, DISCOVERY_EVENT_ON_NAVIGATE_TO, ENTITY_INFO_CAPABILITY,
                ENTITY_INFO_EVENT, EVENT_DISCOVERY_POLICY_CHANGED, PURCHASED_CONTENT_CAPABILITY,
                PURCHASED_CONTENT_EVENT,
            },
            provider::{ProviderRequestPayload, ProviderResponse, ProviderResponsePayload},
        },
    },
    log::{error, info},
    tokio::{sync::oneshot, time::timeout},
};
use ripple_sdk::{
    api::{
        device::entertainment_data::*,
        firebolt::{
            fb_capabilities::JSON_RPC_STANDARD_ERROR_INVALID_PARAMS,
            fb_general::{ListenRequest, ListenerResponse},
            provider::ExternalProviderResponse,
        },
        gateway::rpc_gateway_api::CallContext,
        manifest::device_manifest::IntentValidation,
    },
    utils::rpc_utils::rpc_error_with_code_result,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::state::platform_state::PlatformState;

#[rpc(server)]
pub trait Discovery {
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
        error!("Send error for AppMethod::GetAppContentCatalog {:?}", e);
        return Err(rpc_err("Unable send app request"));
    }
    let resp = rpc_await_oneshot(app_resp_rx).await?;

    if let AppManagerResponse::AppContentCatalog(content_catalog) = resp? {
        content_partner_id = content_catalog.map_or(ctx.app_id.to_owned(), |x| x)
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
                .filter(|val| !val.is_empty())
                .collect();
            if !apis.is_empty() {
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
        let content_policy = ContentPolicy {
            enable_recommendations: PrivacyImpl::get_allow_personalization(state, app_id).await,
            share_watch_history: PrivacyImpl::get_share_watch_history(ctx, state, app_id).await,
            remember_watched_programs: PrivacyImpl::get_allow_watch_history(state, app_id).await,
        };
        Ok(content_policy)
    }

    pub fn get_share_watch_history() -> bool {
        false
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
        match DiscoveryImpl::get_content_policy(ctx, ps, &ctx.app_id).await {
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

    async fn get_content_policy_rpc(&self, ctx: CallContext) -> RpcResult<ContentPolicy> {
        DiscoveryImpl::get_content_policy(&ctx, &self.state, &ctx.app_id).await
    }

    async fn launch(&self, ctx: CallContext, request: LaunchRequest) -> RpcResult<bool> {
        let app_defaults_configuration = self.state.get_device_manifest().applications.defaults;

        let intent_validation_config = self
            .state
            .get_device_manifest()
            .get_features()
            .intent_validation;
        validate_navigation_intent(intent_validation_config, request.intent.clone()).await?;

        let req_updated_source = update_intent_source(ctx.app_id.clone(), request.clone());

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
                &serde_json::to_value(req_updated_source.intent).unwrap(),
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

        let app_request =
            AppRequest::new(AppMethod::Launch(req_updated_source.clone()), app_resp_tx);

        if self
            .state
            .get_client()
            .send_app_request(app_request)
            .is_ok()
            && app_resp_rx.await.is_ok()
        {
            return Ok(true);
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
            &self.state,
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
            String::from(ENTITY_INFO_EVENT),
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
            caller: ctx.into(),
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
                data: res,
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
            String::from(PURCHASED_CONTENT_EVENT),
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
            caller: ctx.into(),
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
                data: res,
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
}
fn update_intent_source(source_app_id: String, request: LaunchRequest) -> LaunchRequest {
    let source = format!("xrn:firebolt:application:{}", source_app_id);
    match request.intent.clone() {
        Some(NavigationIntent::NavigationIntentStrict(navigation_intent)) => {
            let updated_navigation_intent = match navigation_intent {
                NavigationIntentStrict::Home(mut home_intent) => {
                    home_intent.context.source = source;
                    NavigationIntentStrict::Home(home_intent)
                }
                NavigationIntentStrict::Launch(mut launch_intent) => {
                    launch_intent.context.source = source;
                    NavigationIntentStrict::Launch(launch_intent)
                }
                NavigationIntentStrict::Entity(mut entity_intent) => {
                    entity_intent.context.source = source;
                    NavigationIntentStrict::Entity(entity_intent)
                }
                NavigationIntentStrict::Playback(mut playback_intent) => {
                    playback_intent.context.source = source;
                    NavigationIntentStrict::Playback(playback_intent)
                }
                NavigationIntentStrict::Search(mut search_intent) => {
                    search_intent.context.source = source;
                    NavigationIntentStrict::Search(search_intent)
                }
                NavigationIntentStrict::Section(mut section_intent) => {
                    section_intent.context.source = source;
                    NavigationIntentStrict::Section(section_intent)
                }
                NavigationIntentStrict::Tune(mut tune_intent) => {
                    tune_intent.context.source = source;
                    NavigationIntentStrict::Tune(tune_intent)
                }
                NavigationIntentStrict::ProviderRequest(mut provider_request_intent) => {
                    provider_request_intent.context.source = source;
                    NavigationIntentStrict::ProviderRequest(provider_request_intent)
                }
                NavigationIntentStrict::PlayEntity(mut p) => {
                    p.context.source = source;
                    NavigationIntentStrict::PlayEntity(p)
                }
                NavigationIntentStrict::PlayQuery(mut p) => {
                    p.context.source = source;
                    NavigationIntentStrict::PlayQuery(p)
                }
            };

            LaunchRequest {
                app_id: request.app_id,
                intent: Some(NavigationIntent::NavigationIntentStrict(
                    updated_navigation_intent,
                )),
            }
        }
        Some(NavigationIntent::NavigationIntentLoose(mut loose_intent)) => {
            loose_intent.context.source = source;
            LaunchRequest {
                app_id: request.app_id,
                intent: Some(NavigationIntent::NavigationIntentLoose(loose_intent)),
            }
        }
        _ => request,
    }
}

pub async fn validate_navigation_intent(
    intent_validation_config: IntentValidation,
    intent: Option<NavigationIntent>,
) -> RpcResult<()> {
    match &intent {
        Some(NavigationIntent::NavigationIntentLoose(_)) => {
            let request_intent = serde_json::to_string(&intent).unwrap_or_default();
            if let Err(err) = serde_json::from_str::<NavigationIntentStrict>(&request_intent) {
                if intent_validation_config == IntentValidation::Fail {
                    return rpc_error_with_code_result::<()>(
                        format!("{:?} ", err),
                        JSON_RPC_STANDARD_ERROR_INVALID_PARAMS,
                    );
                } else {
                    ripple_sdk::log::warn!("Intents do not match the spec : {:?} ", err);
                }
            }
        }
        _ => {
            info!(
                "Intents match the spec : {:?} ",
                serde_json::to_string(&intent)
            );
        }
    }
    Ok(())
}
pub struct DiscoveryRPCProvider;
impl RippleRPCProvider<DiscoveryImpl> for DiscoveryRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<DiscoveryImpl> {
        (DiscoveryImpl { state }).into_rpc()
    }
}
