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

use crate::{
    firebolt::rpc::RippleRPCProvider,
    service::apps::provider_broker::{ProviderBroker, ProviderBrokerRequest},
    state::platform_state::PlatformState,
};
use jsonrpsee::{
    core::{Error, RpcResult},
    proc_macros::rpc,
    RpcModule,
};
use ripple_sdk::{api::firebolt::fb_keyboard::KeyboardRequestEmail, async_trait::async_trait};
use ripple_sdk::{
    api::{
        firebolt::{
            fb_general::{ListenRequest, ListenerResponse},
            fb_keyboard::{
                KeyboardProviderResponse, KeyboardRequest, KeyboardRequestPassword,
                KeyboardSessionRequest, KeyboardSessionResponse, KeyboardType, EMAIL_EVENT_PREFIX,
                KEYBOARD_PROVIDER_CAPABILITY, PASSWORD_EVENT_PREFIX, STANDARD_EVENT_PREFIX,
            },
            provider::{FocusRequest, ProviderRequestPayload, ProviderResponsePayload},
        },
        gateway::rpc_gateway_api::CallContext,
    },
    tokio::sync::oneshot,
};

#[rpc(server)]
pub trait Keyboard {
    #[method(name = "keyboard.onRequestStandard")]
    async fn on_request_standard(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "keyboard.onRequestEmail")]
    async fn on_request_email(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "keyboard.onRequestPassword")]
    async fn on_request_password(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "keyboard.standardFocus")]
    async fn standard_focus(
        &self,
        ctx: CallContext,
        request: FocusRequest,
    ) -> RpcResult<Option<()>>;

    #[method(name = "keyboard.standardResponse")]
    async fn standard_response(
        &self,
        ctx: CallContext,
        resp: KeyboardProviderResponse,
    ) -> RpcResult<Option<()>>;

    #[method(name = "keyboard.emailFocus")]
    async fn email_focus(&self, ctx: CallContext, request: FocusRequest) -> RpcResult<Option<()>>;

    #[method(name = "keyboard.emailResponse")]
    async fn email_response(
        &self,
        ctx: CallContext,
        resp: KeyboardProviderResponse,
    ) -> RpcResult<Option<()>>;

    #[method(name = "keyboard.passwordFocus")]
    async fn password_focus(
        &self,
        ctx: CallContext,
        request: FocusRequest,
    ) -> RpcResult<Option<()>>;

    #[method(name = "keyboard.passwordResponse")]
    async fn password_response(
        &self,
        ctx: CallContext,
        resp: KeyboardProviderResponse,
    ) -> RpcResult<Option<()>>;

    #[method(name = "keyboard.standard")]
    async fn standard(&self, ctx: CallContext, request: KeyboardRequest) -> RpcResult<String>;

    #[method(name = "keyboard.email")]
    async fn email(&self, ctx: CallContext, request: KeyboardRequestEmail) -> RpcResult<String>;

    #[method(name = "keyboard.password")]
    async fn password(
        &self,
        ctx: CallContext,
        request: KeyboardRequestPassword,
    ) -> RpcResult<String>;
}

pub struct KeyboardImpl {
    pub platform_state: PlatformState,
}

#[async_trait]
impl KeyboardServer for KeyboardImpl {
    async fn on_request_standard(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_session(ctx, request, KeyboardType::Standard, STANDARD_EVENT_PREFIX)
            .await
    }
    async fn on_request_email(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_session(ctx, request, KeyboardType::Email, EMAIL_EVENT_PREFIX)
            .await
    }
    async fn on_request_password(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_session(ctx, request, KeyboardType::Password, PASSWORD_EVENT_PREFIX)
            .await
    }

    async fn standard_response(
        &self,
        _ctx: CallContext,
        resp: KeyboardProviderResponse,
    ) -> RpcResult<Option<()>> {
        let msg = resp.to_provider_response();
        ProviderBroker::provider_response(&self.platform_state, msg).await;
        Ok(None)
    }

    async fn standard_focus(
        &self,
        ctx: CallContext,
        request: FocusRequest,
    ) -> RpcResult<Option<()>> {
        ProviderBroker::focus(
            &self.platform_state,
            ctx,
            KEYBOARD_PROVIDER_CAPABILITY.to_string(),
            request,
        )
        .await;
        Ok(None)
    }

    async fn email_response(
        &self,
        _ctx: CallContext,
        resp: KeyboardProviderResponse,
    ) -> RpcResult<Option<()>> {
        let msg = resp.to_provider_response();
        ProviderBroker::provider_response(&self.platform_state, msg).await;
        Ok(None)
    }

    async fn email_focus(&self, ctx: CallContext, request: FocusRequest) -> RpcResult<Option<()>> {
        ProviderBroker::focus(
            &self.platform_state,
            ctx,
            KEYBOARD_PROVIDER_CAPABILITY.to_string(),
            request,
        )
        .await;
        Ok(None)
    }

    async fn password_response(
        &self,
        _ctx: CallContext,
        resp: KeyboardProviderResponse,
    ) -> RpcResult<Option<()>> {
        let msg = resp.to_provider_response();
        ProviderBroker::provider_response(&self.platform_state, msg).await;
        Ok(None)
    }

    async fn password_focus(
        &self,
        ctx: CallContext,
        request: FocusRequest,
    ) -> RpcResult<Option<()>> {
        ProviderBroker::focus(
            &self.platform_state,
            ctx,
            KEYBOARD_PROVIDER_CAPABILITY.to_string(),
            request,
        )
        .await;
        Ok(None)
    }

    async fn standard(&self, ctx: CallContext, request: KeyboardRequest) -> RpcResult<String> {
        Ok(self
            .call_keyboard_provider(ctx, request, KeyboardType::Standard)
            .await?
            .text)
    }

    async fn email(&self, ctx: CallContext, request: KeyboardRequestEmail) -> RpcResult<String> {
        let req = KeyboardRequest {
            message: request.message.unwrap_or_default(),
        };
        Ok(self
            .call_keyboard_provider(ctx, req, KeyboardType::Email)
            .await?
            .text)
    }

    async fn password(
        &self,
        ctx: CallContext,
        request: KeyboardRequestPassword,
    ) -> RpcResult<String> {
        let req = KeyboardRequest {
            message: request.message.unwrap_or_default(),
        };
        Ok(self
            .call_keyboard_provider(ctx, req, KeyboardType::Password)
            .await?
            .text)
    }
}

impl KeyboardImpl {
    async fn call_keyboard_provider(
        &self,
        ctx: CallContext,
        request: KeyboardRequest,
        typ: KeyboardType,
    ) -> RpcResult<KeyboardSessionResponse> {
        let method = String::from(typ.to_provider_method());
        let session = KeyboardSessionRequest {
            _type: typ,
            ctx: ctx.clone(),
            message: request.message,
        };
        let (session_tx, session_rx) = oneshot::channel::<ProviderResponsePayload>();
        let pr_msg = ProviderBrokerRequest {
            // TODO which capability this rpc method providers should come from firebolt spec
            capability: KEYBOARD_PROVIDER_CAPABILITY.to_string(),
            method,
            caller: ctx.into(),
            request: ProviderRequestPayload::KeyboardSession(session),
            tx: session_tx,
            app_id: None,
        };
        ProviderBroker::invoke_method(&self.platform_state, pr_msg).await;
        match session_rx.await {
            Ok(result) => match result.as_keyboard_result() {
                Some(res) => Ok(res),
                None => Err(Error::Custom(String::from(
                    "Invalid response back from provider",
                ))),
            },
            Err(_) => Err(Error::Custom(String::from(
                "Error returning back from keyboard provider",
            ))),
        }
    }

    async fn on_request_session(
        &self,
        ctx: CallContext,
        request: ListenRequest,
        typ: KeyboardType,
        event_name: &'static str,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        let method = String::from(typ.to_provider_method());
        // TODO which capability this rpc method providers should come from firebolt spec
        ProviderBroker::register_or_unregister_provider(
            &self.platform_state,
            String::from(KEYBOARD_PROVIDER_CAPABILITY),
            method,
            String::from(event_name),
            ctx,
            request,
        )
        .await;
        Ok(ListenerResponse {
            listening: listen,
            event: event_name.into(),
        })
    }
}

pub struct KeyboardRPCProvider;

impl RippleRPCProvider<KeyboardImpl> for KeyboardRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<KeyboardImpl> {
        (KeyboardImpl {
            platform_state: state,
        })
        .into_rpc()
    }
}
