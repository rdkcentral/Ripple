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

use crate::service::settings_processor::{get_settings_map, subscribe_to_settings};
use crate::utils::rpc_utils::rpc_add_event_listener;
use jsonrpsee::{core::RpcResult, proc_macros::rpc, RpcModule};
use ripple_sdk::{
    api::{
        apps::{AppEvent, AppEventRequest, AppManagerResponse, AppMethod, AppRequest, AppResponse},
        caps::CapsRequest,
        firebolt::{
            fb_discovery::{AgePolicy, PolicyIdentifierAlias},
            fb_general::ListenRequestWithEvent,
            fb_keyboard::{
                KeyboardSessionRequest, KeyboardSessionResponse, KEYBOARD_PROVIDER_CAPABILITY,
            },
            fb_pin::{
                PinChallengeRequestWithContext, PinChallengeResponse, PIN_CHALLENGE_CAPABILITY,
            },
            fb_telemetry::TelemetryPayload,
            provider::{ProviderRequestPayload, ProviderResponsePayload},
        },
        gateway::rpc_gateway_api::CallContext,
        settings::{SettingValue, SettingsRequest, SettingsRequestParam},
    },
    async_trait::async_trait,
    log::{debug, error},
    service::service_event_state::Event,
    tokio::sync::oneshot,
    utils::{error::RippleError, rpc_utils::rpc_err},
};

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::{
    firebolt::rpc::RippleRPCProvider,
    service::{
        apps::{
            app_events::AppEvents,
            provider_broker::{ProviderBroker, ProviderBrokerRequest},
        },
        telemetry_builder::TelemetryBuilder,
    },
    state::platform_state::PlatformState,
    utils::rpc_utils::rpc_await_oneshot,
};
use ripple_sdk::api::firebolt::fb_general::ListenRequest;
use ripple_sdk::api::firebolt::fb_general::ListenerResponse;

const EVENT_POLICY_IDENTIFIER_ALIAS_CHANGED: &str = "account.onPolicyIdentifierAliasChanged";

#[rpc(server)]
pub trait Internal {
    #[method(name = "ripple.sendTelemetry")]
    async fn send_telemetry(&self, ctx: CallContext, payload: TelemetryPayload) -> RpcResult<()>;

    #[method(name = "ripple.setTelemetrySessionId")]
    fn set_telemetry_session_id(&self, ctx: CallContext, session_id: String) -> RpcResult<()>;

    #[method(name = "ripple.sendAppEvent")]
    async fn send_app_event(&self, ctx: CallContext, event: AppEvent) -> RpcResult<()>;

    #[method(name = "ripple.registerAppEvent")]
    async fn register_app_event(
        &self,
        ctx: CallContext,
        request: ListenRequestWithEvent,
    ) -> RpcResult<()>;

    #[method(name = "ripple.getAppCatalogId")]
    async fn get_app_catalog_id(&self, ctx: CallContext, app_id: String) -> RpcResult<String>;

    #[method(name = "ripple.checkCapsRequest")]
    async fn check_caps_request(
        &self,
        ctx: CallContext,
        caps_request: CapsRequest,
    ) -> RpcResult<HashMap<String, bool>>;

    #[method(name = "ripple.getSecondScreenPayload")]
    async fn get_second_screen_payload(&self, ctx: CallContext) -> RpcResult<String>;

    #[method(name = "account.onPolicyIdentifierAliasChanged")]
    async fn on_policy_identifier_alias_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "account.setPolicyIdentifierAlias")]
    async fn set_policy_identifier_alias(
        &self,
        ctx: CallContext,
        params: PolicyIdentifierAlias,
    ) -> RpcResult<()>;

    #[method(name = "account.policyIdentifierAlias")]
    async fn get_policy_identifier_alias(&self, ctx: CallContext) -> RpcResult<Vec<AgePolicy>>;

    #[method(name = "ripple.onContextTokenChangedEvent")]
    async fn subscribe_context_token_changed_event(&self, ctx: CallContext) -> RpcResult<()>;

    #[method(name = "ripple.sendAppEventRequest")]
    async fn send_app_event_request(
        &self,
        ctx: CallContext,
        app_event_request: AppEventRequest,
    ) -> RpcResult<bool>;

    #[method(name = "ripple.promptEmailRequest")]
    async fn prompt_email_request(
        &self,
        ctx: CallContext,
        request: KeyboardSessionRequest,
    ) -> RpcResult<KeyboardSessionResponse>;

    #[method(name = "ripple.showPinOverlay")]
    async fn show_pin_overlay(
        &self,
        ctx: CallContext,
        request: PinChallengeRequestWithContext,
    ) -> RpcResult<PinChallengeResponse>;

    #[method(name = "ripple.getSettingsRequest")]
    async fn get_settings_request(
        &self,
        ctx: CallContext,
        request: SettingsRequest,
    ) -> RpcResult<HashMap<String, SettingValue>>;

    #[method(name = "ripple.subscribeSettings")]
    async fn subscribe_settings(
        &self,
        ctx: CallContext,
        request: SettingsRequestParam,
    ) -> RpcResult<()>;
}

#[derive(Debug, Clone, Default)]
pub struct PolicyState {
    pub policy_identifiers_alias: Arc<RwLock<Vec<AgePolicy>>>,
}

#[derive(Debug)]
pub struct InternalImpl {
    pub state: PlatformState,
}

#[async_trait]
impl InternalServer for InternalImpl {
    async fn send_telemetry(&self, _ctx: CallContext, payload: TelemetryPayload) -> RpcResult<()> {
        let _ = TelemetryBuilder::send_telemetry(&self.state, payload);
        Ok(())
    }

    fn set_telemetry_session_id(&self, _ctx: CallContext, session_id: String) -> RpcResult<()> {
        self.state.metrics.update_session_id(Some(session_id));
        Ok(())
    }

    async fn send_app_event(&self, _ctx: CallContext, event: AppEvent) -> RpcResult<()> {
        debug!("Sending App event {:?}", &event);
        AppEvents::emit_with_context(&self.state, &event.event_name, &event.result, event.context)
            .await;
        Ok(())
    }

    async fn register_app_event(
        &self,
        _ctx: CallContext,
        request: ListenRequestWithEvent,
    ) -> RpcResult<()> {
        debug!("registering App event {:?}", &request);
        let event = request.event.clone();
        AppEvents::add_listener(&self.state, event, request.context.clone(), request.request);
        Ok(())
    }

    async fn get_app_catalog_id(&self, _: CallContext, app_id: String) -> RpcResult<String> {
        let (app_resp_tx, app_resp_rx) = oneshot::channel::<AppResponse>();

        let app_request =
            AppRequest::new(AppMethod::GetAppContentCatalog(app_id.clone()), app_resp_tx);
        if let Err(e) = self.state.get_client().send_app_request(app_request) {
            error!("Send error for AppMethod::GetAppContentCatalog {:?}", e);
        }
        let resp = rpc_await_oneshot(app_resp_rx).await;

        if let Ok(Ok(AppManagerResponse::AppContentCatalog(content_catalog))) = resp {
            return Ok(content_catalog.map_or(app_id.to_owned(), |x| x));
        }

        Ok(app_id)
    }

    async fn check_caps_request(
        &self,
        _ctx: CallContext,
        caps_request: CapsRequest,
    ) -> RpcResult<HashMap<String, bool>> {
        match caps_request {
            CapsRequest::Supported(request) => {
                let result = self.state.cap_state.generic.check_for_processor(request);
                Ok(result)
            }
            CapsRequest::Permitted(app_id, request) => {
                let result = self
                    .state
                    .cap_state
                    .permitted_state
                    .check_multiple(&app_id, request);
                Ok(result)
            }
        }
    }

    async fn set_policy_identifier_alias(
        &self,
        _ctx: CallContext,
        params: PolicyIdentifierAlias,
    ) -> RpcResult<()> {
        debug!("Setting policy identifier alias: {:?}", params);
        let current_age_policies = self.state.get_policy_identifier_alias();
        let new_age_policies = params.policy_identifier_alias.clone();
        if current_age_policies == new_age_policies {
            debug!("Policy identifier alias is the same as existing, no update needed");
            return Ok(());
        }

        self.state.add_policy_identifier_alias(params.clone());
        let value = serde_json::to_value(&params).unwrap_or_default();
        AppEvents::emit(&self.state, EVENT_POLICY_IDENTIFIER_ALIAS_CHANGED, &value).await;
        Ok(())
    }

    async fn get_policy_identifier_alias(&self, _ctx: CallContext) -> RpcResult<Vec<AgePolicy>> {
        Ok(self.state.get_policy_identifier_alias())
    }

    async fn on_policy_identifier_alias_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        debug!(
            "Subscribing to policy identifier alias changed event request: {:?}",
            request
        );
        rpc_add_event_listener(
            &self.state,
            ctx,
            request,
            EVENT_POLICY_IDENTIFIER_ALIAS_CHANGED,
        )
        .await
    }

    async fn get_second_screen_payload(&self, ctx: CallContext) -> RpcResult<String> {
        let (app_resp_tx, app_resp_rx) = oneshot::channel::<AppResponse>();

        let app_request = AppRequest::new(
            AppMethod::GetSecondScreenPayload(ctx.app_id.clone()),
            app_resp_tx,
        );

        if let Err(e) = self.state.get_client().send_app_request(app_request) {
            error!("Send error for GetSecondScreenPayload {:?}", e);
            return Err(rpc_err("Unable to send app request"));
        }

        let resp = rpc_await_oneshot(app_resp_rx).await;
        if let Ok(Ok(AppManagerResponse::SecondScreenPayload(payload))) = resp {
            return Ok(payload);
        }

        // return empty string if the payload is not available
        error!(
            "Failed to get second screen payload for app_id: {}",
            ctx.app_id
        );
        Ok(String::new())
    }

    async fn subscribe_context_token_changed_event(&self, ctx: CallContext) -> RpcResult<()> {
        let context = ctx.context;
        if let Ok(service_message_context) = serde_json::to_value(context) {
            match self
                .state
                .service_controller_state
                .service_event_state
                .subscribe_context_event(
                    Event::RippleContextTokenChangedEvent.to_string().as_str(),
                    Some(service_message_context),
                ) {
                Ok(()) => return Ok(()),
                Err(e) => error!("Failed to subscribe to context token changed event: {}", e),
            }
        }
        Err(jsonrpsee::core::error::Error::Custom(
            "Failed to subscribe to context token changed event".to_owned(),
        ))
    }

    async fn send_app_event_request(
        &self,
        _ctx: CallContext,
        app_event_request: AppEventRequest,
    ) -> RpcResult<bool> {
        match app_event_request.clone() {
            AppEventRequest::Emit(event) => {
                if let Some(app_id) = event.app_id {
                    let event_name = &event.event_name;
                    let result = &event.result;
                    AppEvents::emit_to_app(&self.state, app_id, event_name, result).await;
                } else {
                    AppEvents::emit_with_app_event(&self.state, app_event_request).await;
                }
            }
            AppEventRequest::Register(ctx, event, request) => {
                AppEvents::add_listener(&self.state, event, ctx, request);
            }
        }
        Ok(true)
    }

    async fn prompt_email_request(
        &self,
        _ctx: CallContext,
        request: KeyboardSessionRequest,
    ) -> RpcResult<KeyboardSessionResponse> {
        let method = String::from(request._type.to_provider_method());
        let (session_tx, session_rx) = oneshot::channel::<ProviderResponsePayload>();
        let pr_msg = ProviderBrokerRequest {
            capability: KEYBOARD_PROVIDER_CAPABILITY.to_string(),
            method,
            caller: request.clone().ctx.into(),
            request: ProviderRequestPayload::KeyboardSession(request.clone()),
            tx: session_tx,
            app_id: None,
        };
        ProviderBroker::invoke_method(&self.state, pr_msg).await;
        if let Ok(result) = session_rx.await {
            if let Some(keyboard_response) = result.as_keyboard_result() {
                return Ok(keyboard_response);
            }
        }
        Err(rpc_err("Unpermitted"))
    }

    async fn show_pin_overlay(
        &self,
        _ctx: CallContext,
        request: PinChallengeRequestWithContext,
    ) -> RpcResult<PinChallengeResponse> {
        let (session_tx, session_rx) = oneshot::channel::<ProviderResponsePayload>();
        let pr_msg = ProviderBrokerRequest {
            capability: String::from(PIN_CHALLENGE_CAPABILITY),
            method: String::from("pinchallenge.onRequestChallenge"),
            caller: request.call_ctx.clone().into(),
            request: ProviderRequestPayload::PinChallenge(request.into()),
            tx: session_tx,
            app_id: None,
        };
        ProviderBroker::invoke_method(&self.state, pr_msg).await;
        if let Ok(result) = session_rx.await {
            if let Some(response) = result.as_pin_challenge_response() {
                return Ok(response);
            }
        }
        Err(rpc_err("Unpermitted"))
    }

    async fn get_settings_request(
        &self,
        _ctx: CallContext,
        request: SettingsRequest,
    ) -> RpcResult<HashMap<String, SettingValue>> {
        let settings = match request {
            SettingsRequest::Get(req) => get_settings_map(&self.state, &req).await,
            _ => {
                error!("Unsupported SettingsRequest variant");
                Err(RippleError::InvalidInput)
            }
        };
        match settings {
            Ok(map) => Ok(map),
            Err(e) => {
                error!("Error getting settings: {:?}", e);
                Err(rpc_err("Error getting settings"))
            }
        }
    }

    async fn subscribe_settings(
        &self,
        _ctx: CallContext,
        request: SettingsRequestParam,
    ) -> RpcResult<()> {
        subscribe_to_settings(&self.state, request).await;
        Ok(())
    }
}

pub struct InternalProvider;
impl RippleRPCProvider<InternalImpl> for InternalProvider {
    fn provide(state: PlatformState) -> RpcModule<InternalImpl> {
        (InternalImpl { state }).into_rpc()
    }
}
