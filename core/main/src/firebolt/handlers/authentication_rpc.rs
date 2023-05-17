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

use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    types::error::CallError,
    RpcModule,
};
use ripple_sdk::{
    api::{
        firebolt::{
            fb_authentication::{AuthRequest, GetPlatformTokenParams, TokenRequest, TokenResult},
            fb_capabilities::{FireboltCap, CAPABILITY_NOT_AVAILABLE, CAPABILITY_NOT_SUPPORTED},
        },
        gateway::rpc_gateway_api::CallContext,
        session::{SessionTokenRequest, TokenType},
    },
    extn::extn_client_message::ExtnResponse,
};

use crate::{
    firebolt::rpc::RippleRPCProvider, state::platform_state::PlatformState,
    utils::rpc_utils::rpc_err,
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
                let cap = FireboltCap::Short("token:account".into());
                let supported_caps = self
                    .platform_state
                    .get_device_manifest()
                    .get_supported_caps();
                if supported_caps.contains(&cap) {
                    self.platform_token(ctx).await
                } else {
                    return Err(jsonrpsee::core::Error::Call(CallError::Custom {
                        code: CAPABILITY_NOT_AVAILABLE,
                        message: format!("{} is not available", cap.as_str()),
                        data: None,
                    }));
                }
            }
            TokenType::Root => self.root_token(ctx).await,
            TokenType::Device => {
                let feats = self.platform_state.get_device_manifest().get_features();
                // let feats = self.helper.get_config().get_features();
                if feats.app_scoped_device_tokens {
                    self.device_token(ctx).await
                } else {
                    self.root_token(ctx).await
                }
            }
            TokenType::Distributor => {
                //error!("distributor token type is unsupported");
                return Err(jsonrpsee::core::Error::Call(CallError::Custom {
                    code: CAPABILITY_NOT_SUPPORTED,
                    message: format!(
                        "capability xrn:firebolt:capability:token:session is not supported"
                    ),
                    data: None,
                }));
            }
        }
    }

    async fn root(&self, ctx: CallContext) -> RpcResult<String> {
        let r = self.root_token(ctx).await;
        if r.is_ok() {
            return Ok(r.unwrap().value);
        } else {
            return Err(r.err().unwrap());
        }
    }

    async fn device(&self, ctx: CallContext) -> RpcResult<String> {
        let feats = self.platform_state.get_device_manifest().get_features();
        let r = if feats.app_scoped_device_tokens {
            self.device_token(ctx).await
        } else {
            self.root_token(ctx).await
        };
        if r.is_ok() {
            return Ok(r.unwrap().value);
        } else {
            return Err(r.err().unwrap());
        }
    }

    async fn session(&self, _ctx: CallContext) -> RpcResult<String> {
        Err(jsonrpsee::core::Error::Call(CallError::Custom {
            code: CAPABILITY_NOT_SUPPORTED,
            message: format!("authentication.session is not supported"),
            data: None,
        }))
    }
}

impl AuthenticationImpl {
    async fn platform_token(&self, ctx: CallContext) -> RpcResult<TokenResult> {
        let session = self
            .platform_state
            .session_state
            .get_account_session()
            .unwrap();

        let payload = AuthRequest::GetPlatformToken(GetPlatformTokenParams {
            app_id: ctx.app_id.clone(),
            dist_session: session,
            // content_provider: get_content_partner_id(&self.helper, &ctx)
            //.await
            //.unwrap_or(ctx.app_id.clone()),
            content_provider: String::from("xumo"),
            device_session_id: ctx.session_id.clone(),
            // device_session_id: (&self.platform_state.device_session_id).into(),
            app_session_id: ctx.session_id.clone(),
        });
        let resp = self
            .platform_state
            .get_client()
            .send_extn_request(payload)
            .await;

        if let Err(_) = resp {
            return Err(rpc_err("Failed to fetch token from distribution platform"));
        }

        match resp {
            Ok(payload) => match payload.payload.extract().unwrap() {
                ExtnResponse::Token(t) => Ok(TokenResult {
                    value: t.value,
                    expires: t.expires,
                    _type: TokenType::Root,
                }),
                _ => Err(rpc_err("Distribution platform returned invalid format")),
            },

            Err(_e) => {
                // TODO: What do error responses look like?
                Err(jsonrpsee::core::Error::Custom(String::from(
                    "Ripple Error getting platform token",
                )))
            }
        }
    }

    async fn root_token(&self, _ctx: CallContext) -> RpcResult<TokenResult> {
        let resp = self
            .platform_state
            .get_client()
            .send_extn_request(SessionTokenRequest {
                token_type: TokenType::Root,
                options: Vec::new(),
                context: None,
            })
            .await;

        match resp {
            Ok(payload) => match payload.payload.extract().unwrap() {
                ExtnResponse::Token(t) => Ok(TokenResult {
                    value: t.value,
                    expires: t.expires,
                    _type: TokenType::Root,
                }),
                e => Err(jsonrpsee::core::Error::Custom(String::from(format!(
                    "unknown error getting root token {:?}",
                    e
                )))),
            },

            Err(_e) => {
                // TODO: What do error responses look like?
                Err(jsonrpsee::core::Error::Custom(String::from(
                    "Ripple Error getting root token",
                )))
            }
        }
    }

    async fn device_token(&self, _ctx: CallContext) -> RpcResult<TokenResult> {
        if let Some(_sess) = self.platform_state.session_state.get_account_session() {
            let resp = self
                .platform_state
                .get_client()
                .send_extn_request(SessionTokenRequest {
                    token_type: TokenType::Device,
                    options: Vec::new(),
                    context: None,
                })
                .await;

            match resp {
                Ok(payload) => match payload.payload.extract().unwrap() {
                    // let expires = t.expires
                    ExtnResponse::Token(t) => Ok(TokenResult {
                        value: t.value,
                        expires: None,
                        _type: TokenType::Device,
                    }),
                    e => Err(jsonrpsee::core::Error::Custom(String::from(format!(
                        "unknown rror getting device token {:?}",
                        e
                    )))),
                },

                Err(_e) => {
                    // TODO: What do error responses look like?
                    Err(jsonrpsee::core::Error::Custom(String::from(
                        "Ripple Error getting device token",
                    )))
                }
            }
        } else {
            return Err(rpc_err("Distributor not available"));
        }
    }
}

pub struct AuthRPCrovider;
impl RippleRPCProvider<AuthenticationImpl> for AuthRPCrovider {
    fn provide(platform_state: PlatformState) -> RpcModule<AuthenticationImpl> {
        (AuthenticationImpl { platform_state }).into_rpc()
    }
}
