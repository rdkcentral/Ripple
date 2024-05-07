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
    firebolt::{handlers::discovery_rpc::get_content_partner_id, rpc::RippleRPCProvider},
    state::platform_state::PlatformState,
};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    types::error::CallError,
    RpcModule,
};
use ripple_sdk::{
    api::{
        distributor::{
            distributor_platform::{PlatformTokenContext, PlatformTokenRequest},
            distributor_token::{DistributorTokenContext, DistributorTokenRequest},
        },
        firebolt::{
            fb_authentication::{TokenRequest, TokenResult},
            fb_capabilities::{FireboltCap, CAPABILITY_NOT_SUPPORTED},
        },
        gateway::rpc_gateway_api::CallContext,
        session::{SessionTokenRequest, TokenContext, TokenType},
    },
    extn::extn_client_message::ExtnResponse,
};

#[rpc(server)]
pub trait Authentication {
    #[method(name = "authentication.token", param_kind = map)]
    async fn token(&self, ctx: CallContext, x: TokenRequest) -> RpcResult<TokenResult>;
    #[method(name = "authentication.root", param_kind = map)]
    async fn root(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "authentication.device", param_kind = map)]
    async fn device(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "authentication.session")]
    async fn session(&self, ctx: CallContext) -> RpcResult<String>;
}

pub struct AuthenticationImpl {
    pub platform_state: PlatformState,
}

#[async_trait]
impl AuthenticationServer for AuthenticationImpl {
    async fn token(&self, ctx: CallContext, token_request: TokenRequest) -> RpcResult<TokenResult> {
        match token_request._type {
            TokenType::Platform => {
                let cap = FireboltCap::Short("token:platform".into());
                let supported_caps = self
                    .platform_state
                    .get_device_manifest()
                    .get_supported_caps();
                if supported_caps.contains(&cap) {
                    self.token(TokenType::Platform, ctx).await
                } else {
                    return Err(jsonrpsee::core::Error::Call(CallError::Custom {
                        code: CAPABILITY_NOT_SUPPORTED,
                        message: format!("{} is not supported", cap.as_str()),
                        data: None,
                    }));
                }
            }
            TokenType::Root => self.get_root_token().await,
            TokenType::Device => self.token(TokenType::Device, ctx).await,
            TokenType::Distributor => {
                let cap = FireboltCap::Short("token:session".into());
                let supported_caps = self
                    .platform_state
                    .get_device_manifest()
                    .get_supported_caps();
                if supported_caps.contains(&cap) {
                    self.token(TokenType::Distributor, ctx).await
                } else {
                    return Err(jsonrpsee::core::Error::Call(CallError::Custom {
                        code: CAPABILITY_NOT_SUPPORTED,
                        message: format!("{} is not supported", cap.as_str()),
                        data: None,
                    }));
                }
            }
        }
    }

    async fn root(&self, _ctx: CallContext) -> RpcResult<String> {
        match self.get_root_token().await {
            Ok(r) => Ok(r.value),
            Err(e) => Err(e),
        }
    }

    async fn device(&self, ctx: CallContext) -> RpcResult<String> {
        if !self.platform_state.supports_device_tokens() {
            return Err(Self::send_dist_token_not_supported());
        }
        match self.token(TokenType::Device, ctx).await {
            Ok(r) => Ok(r.value),
            Err(e) => Err(e),
        }
    }

    async fn session(&self, ctx: CallContext) -> RpcResult<String> {
        match self.token(TokenType::Distributor, ctx).await {
            Ok(r) => Ok(r.value),
            Err(e) => Err(e),
        }
    }
}

impl AuthenticationImpl {
    fn send_dist_token_not_supported() -> jsonrpsee::core::Error {
        jsonrpsee::core::Error::Call(CallError::Custom {
            code: CAPABILITY_NOT_SUPPORTED,
            message: "capability xrn:firebolt:capability:token:session is not supported"
                .to_string(),
            data: None,
        })
    }

    async fn token(&self, token_type: TokenType, ctx: CallContext) -> RpcResult<TokenResult> {
        if let TokenType::Distributor = &token_type {
            if !self.platform_state.supports_distributor_session() {
                return Err(Self::send_dist_token_not_supported());
            }
        }

        if let TokenType::Device = &token_type {
            if !self.platform_state.supports_device_tokens() {
                return Err(Self::send_dist_token_not_supported());
            }
        }

        let cp_id = get_content_partner_id(&self.platform_state, &ctx)
            .await
            .unwrap_or(ctx.app_id.clone());

        let dist_session = match self.platform_state.session_state.get_account_session() {
            Some(session) => session,
            None => {
                return Err(jsonrpsee::core::Error::Custom(String::from(
                    "Account session is not available",
                )));
            }
        };

        let resp = match &token_type {
            TokenType::Platform => {
                let context = PlatformTokenContext {
                    app_id: ctx.app_id,
                    content_provider: cp_id,
                    device_session_id: (&self.platform_state.device_session_id).into(),
                    app_session_id: ctx.session_id.clone(),
                    dist_session,
                };
                self.platform_state
                    .get_client()
                    .send_extn_request(PlatformTokenRequest {
                        options: Vec::new(),
                        context,
                    })
                    .await
            }
            TokenType::Distributor => {
                let context = DistributorTokenContext {
                    app_id: ctx.app_id,
                    dist_session,
                };
                self.platform_state
                    .get_client()
                    .send_extn_request(DistributorTokenRequest { context })
                    .await
            }
            _ => {
                let context = TokenContext {
                    distributor_id: dist_session.id,
                    app_id: ctx.app_id,
                };
                self.platform_state
                    .get_client()
                    .send_extn_request(SessionTokenRequest {
                        token_type,
                        options: Vec::new(),
                        context: Some(context),
                    })
                    .await
            }
        };
        match resp {
            Ok(payload) => match payload.payload.extract().unwrap() {
                ExtnResponse::Token(t) => Ok(TokenResult {
                    value: t.value,
                    expires: t.expires,
                    _type: token_type,
                    scope: None,
                    expires_in: None,
                    token_type: None,
                }),
                e => Err(jsonrpsee::core::Error::Custom(format!(
                    "unknown error getting {:?} token {:?}",
                    token_type, e
                ))),
            },

            Err(_e) => Err(jsonrpsee::core::Error::Custom(format!(
                "Ripple Error getting {:?} token",
                token_type
            ))),
        }
    }

    async fn get_root_token(&self) -> RpcResult<TokenResult> {
        let token_type = TokenType::Root;
        let resp = self
            .platform_state
            .get_client()
            .send_extn_request(SessionTokenRequest {
                token_type,
                options: Vec::new(),
                context: None,
            })
            .await;

        match resp {
            Ok(payload) => match payload.payload.extract().unwrap() {
                ExtnResponse::Token(t) => Ok(TokenResult {
                    value: t.value,
                    expires: t.expires,
                    _type: token_type,
                    scope: None,
                    expires_in: None,
                    token_type: None,
                }),
                e => Err(jsonrpsee::core::Error::Custom(format!(
                    "unknown error getting {:?} token {:?}",
                    token_type, e
                ))),
            },

            Err(_e) => Err(jsonrpsee::core::Error::Custom(format!(
                "Ripple Error getting {:?} token",
                token_type
            ))),
        }
    }
}

pub struct AuthRPCProvider;
impl RippleRPCProvider<AuthenticationImpl> for AuthRPCProvider {
    fn provide(platform_state: PlatformState) -> RpcModule<AuthenticationImpl> {
        (AuthenticationImpl { platform_state }).into_rpc()
    }
}
