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

use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    types::error::CallError,
    RpcModule,
};
use ripple_sdk::{
    api::{
        firebolt::{
            fb_authentication::{TokenRequest, TokenResult},
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
                    message: "capability xrn:firebolt:capability:token:session is not supported".to_string(),
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
            message: "authentication.session is not supported".to_string(),
            data: None,
        }))
    }
}

impl AuthenticationImpl {
    async fn platform_token(&self, _ctx: CallContext) -> RpcResult<TokenResult> {
        let resp = self
            .platform_state
            .get_client()
            .send_extn_request(SessionTokenRequest {
                token_type: TokenType::Platform,
                options: Vec::new(),
                context: None,
            })
            .await;

        match resp {
            Ok(payload) => match payload.payload.extract().unwrap() {
                ExtnResponse::Token(t) => Ok(TokenResult {
                    value: t.value,
                    expires: t.expires,
                    _type: TokenType::Platform,
                }),
                e => Err(jsonrpsee::core::Error::Custom(format!(
                    "unknown error getting platform token {:?}",
                    e
                ))),
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
                e => Err(jsonrpsee::core::Error::Custom(format!(
                    "unknown error getting root token {:?}",
                    e
                ))),
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
                    e => Err(jsonrpsee::core::Error::Custom(format!(
                        "unknown rror getting device token {:?}",
                        e
                    ))),
                },

                Err(_e) => {
                    // TODO: What do error responses look like?
                    Err(jsonrpsee::core::Error::Custom(String::from(
                        "Ripple Error getting device token",
                    )))
                }
            }
        } else {
            Err(rpc_err("Distributor not available"))
        }
    }
}

pub struct AuthRPCProvider;
impl RippleRPCProvider<AuthenticationImpl> for AuthRPCProvider {
    fn provide(platform_state: PlatformState) -> RpcModule<AuthenticationImpl> {
        (AuthenticationImpl { platform_state }).into_rpc()
    }
}
