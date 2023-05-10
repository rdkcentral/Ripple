use crate::api::handlers::discovery::get_content_partner_id;

use crate::helpers::error_util::CAPABILITY_NOT_SUPPORTED;
use crate::{
    api::rpc::rpc_gateway::{CallContext, RPCProvider},
    helpers::{
        error_util::CAPABILITY_NOT_AVAILABLE,
        ripple_helper::{IRippleHelper, RippleHelper, RippleHelperFactory, RippleHelperType},
        rpc_util::rpc_err,
        session_util::{dab_to_dpab, get_distributor_session_from_platform_state},
    },
    managers::capability_manager::{
        CapClassifiedRequest, FireboltCap, IGetLoadedCaps, RippleHandlerCaps,
    },
    platform_state::PlatformState,
};
use dab::core::{
    message::{DabRequestPayload, DabResponsePayload},
    model::authentication::{TokenContext, TokenRequest as ModelTokenRequest, TokenType},
    utils::timestamp_2_iso8601,
};

use dpab::core::{
    message::DpabRequestPayload,
    model::auth::{AuthRequest, GetPlatformTokenParams},
};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    types::error::CallError,
    RpcModule,
};
use serde::{Deserialize, Serialize};
use std::convert::From;
use tracing::{error, instrument};

#[rpc(server)]
pub trait Authentication {
    #[method(name = "authentication.token", param_kind = map)]
    async fn token(&self, ctx: CallContext, x: TokenRequest) -> RpcResult<TokenResult>;
    #[method(name = "badger.refreshPlatformAuthToken", param_kind = map)]
    async fn badger_refresh_platform_auth_token(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "badger.getXact")]
    async fn badger_get_xact(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "authentication.root", param_kind = map)]
    async fn root(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "authentication.device", param_kind = map)]
    async fn device(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "authentication.session")]
    async fn session(&self, ctx: CallContext) -> RpcResult<String>;
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenResult {
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,
    #[serde(rename = "type")]
    pub _type: TokenType,
}

pub struct AuthenticationImpl<IRippleHelper> {
    pub helper: Box<IRippleHelper>,
    pub platform_state: PlatformState,
}

#[derive(Deserialize, Debug)]
pub struct TokenRequest {
    #[serde(rename = "type")]
    pub _type: TokenType,
}

#[async_trait]
impl AuthenticationServer for AuthenticationImpl<RippleHelper> {
    #[instrument(skip(self))]
    async fn token(&self, ctx: CallContext, token_request: TokenRequest) -> RpcResult<TokenResult> {
        match token_request._type {
            TokenType::Platform => {
                let cap = FireboltCap::Short("token:account".into());
                if self.helper.is_cap_available(cap.clone()).await {
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
                let feats = self.helper.get_config().get_features();
                if feats.app_scoped_device_tokens {
                    self.device_token(ctx).await
                } else {
                    self.root_token(ctx).await
                }
            }
            TokenType::Distributor => {
                error!("distributor token type is unsupported");
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

    #[instrument(skip(self))]
    async fn badger_refresh_platform_auth_token(&self, ctx: CallContext) -> RpcResult<String> {
        Ok(self.platform_token(ctx).await?.value)
    }
    async fn badger_get_xact(&self, ctx: CallContext) -> RpcResult<String> {
        Ok(self.root_token(ctx).await?.value)
    }

    #[instrument(skip(self))]
    async fn root(&self, ctx: CallContext) -> RpcResult<String> {
        let r = self.root_token(ctx).await;
        if r.is_ok() {
            return Ok(r.unwrap().value);
        } else {
            return Err(r.err().unwrap());
        }
    }

    #[instrument(skip(self))]
    async fn device(&self, ctx: CallContext) -> RpcResult<String> {
        let feats = self.helper.get_config().get_features();
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
    #[instrument(skip(self))]
    async fn session(&self, _ctx: CallContext) -> RpcResult<String> {
        Err(jsonrpsee::core::Error::Call(CallError::Custom {
            code: CAPABILITY_NOT_SUPPORTED,
            message: format!("authentication.session is not supported"),
            data: None,
        }))
    }
}

impl AuthenticationImpl<RippleHelper> {
    async fn platform_token(&self, ctx: CallContext) -> RpcResult<TokenResult> {
        let cm_c = self.helper.get_config();
        let session = get_distributor_session_from_platform_state(&self.platform_state).await?;

        let payload =
            DpabRequestPayload::Auth(AuthRequest::GetPlatformToken(GetPlatformTokenParams {
                app_id: ctx.app_id.clone(),
                dist_session: dab_to_dpab(session).unwrap(),
                content_provider: get_content_partner_id(&self.helper, &ctx)
                    .await
                    .unwrap_or(ctx.app_id.clone()),
                device_session_id: (&self.platform_state.device_session_id).into(),
                app_session_id: ctx.session_id.clone(),
            }));

        let resp = self.helper.clone().send_dpab(payload).await;

        if let Err(_) = resp {
            return Err(rpc_err("Failed to fetch token from distribution platform"));
        }
        let payload = resp.unwrap().as_string();
        if let None = payload {
            return Err(rpc_err("Distribution platform returned invalid format"));
        }
        Ok(TokenResult {
            value: payload.unwrap(),
            expires: None,
            _type: TokenType::Platform,
        })
    }
    async fn root_token(&self, _ctx: CallContext) -> RpcResult<TokenResult> {
        let resp = self
            .helper
            .send_dab(DabRequestPayload::Token(ModelTokenRequest {
                token_type: TokenType::Root,
                options: Vec::new(),
                context: None,
            }))
            .await;

        match resp {
            Ok(payload) => match payload {
                DabResponsePayload::RootToken(t) => Ok(TokenResult {
                    value: t.token_container.token_data,
                    expires: Some(timestamp_2_iso8601(t.token_container.expires)),
                    _type: TokenType::Root,
                }),
                e => Err(jsonrpsee::core::Error::Custom(String::from(format!(
                    "unknown DABrror getting root token {:?}",
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

    async fn device_token(&self, ctx: CallContext) -> RpcResult<TokenResult> {
        let sess = get_distributor_session_from_platform_state(&self.platform_state).await;
        if sess.is_err() {
            return Err(rpc_err("Distributor not available"));
        }
        let did_opt = sess.unwrap().id;
        if did_opt.is_none() {
            return Err(rpc_err("Distributor not available"));
        }

        let resp = self
            .helper
            .send_dab(DabRequestPayload::Token(ModelTokenRequest {
                token_type: TokenType::Device,
                options: Vec::new(),
                context: Some(TokenContext {
                    app_id: ctx.app_id.clone(),
                    distributor_id: did_opt.unwrap(),
                }),
            }))
            .await;

        match resp {
            Ok(p) => match p {
                DabResponsePayload::String(s) => Ok(TokenResult {
                    value: s,
                    expires: None,
                    _type: TokenType::Device,
                }),
                _ => Err(jsonrpsee::core::Error::Custom(String::from(
                    "Unknown DAB response attempting to get device token",
                ))),
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "Ripple error getting device token",
            ))),
        }
    }
}

pub struct AuthRippleProvider;

pub struct AuthCapHandler;

impl IGetLoadedCaps for AuthCapHandler {
    fn get_loaded_caps(&self) -> RippleHandlerCaps {
        RippleHandlerCaps {
            caps: Some(vec![CapClassifiedRequest::Supported(vec![
                FireboltCap::Short("badger:refreshPlatformAuthToken".into()),
                FireboltCap::Short("badger:getXact".into()),
                FireboltCap::Short("token:platform".into()),
                FireboltCap::Short("token:device".into()),
                FireboltCap::Short("token:root".into()),
            ])]),
        }
    }
}

impl RPCProvider<AuthenticationImpl<RippleHelper>, AuthCapHandler> for AuthRippleProvider {
    fn provide(
        self,
        rhf: Box<RippleHelperFactory>,
        platform_state: PlatformState,
    ) -> (RpcModule<AuthenticationImpl<RippleHelper>>, AuthCapHandler) {
        let a = AuthenticationImpl {
            helper: rhf.get(self.get_helper_variant()),
            platform_state,
        };
        (a.into_rpc(), AuthCapHandler)
    }

    fn get_helper_variant(self) -> Vec<RippleHelperType> {
        vec![
            RippleHelperType::Dab,
            RippleHelperType::Dpab,
            RippleHelperType::Cap,
            RippleHelperType::AppManager,
        ]
    }
}
