use chrono::{DateTime, Utc};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    RpcModule,
};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
use tokio::sync::oneshot;
use tracing::instrument;

use crate::{
    api::{
        permissions::user_grants::{GrantEntry, UserGrantStateUtils},
        rpc::rpc_gateway::{CallContext, RPCProvider},
    },
    apps::app_mgr::{AppError, AppManagerResponse, AppMethod, AppRequest},
    helpers::{
        error_util::rpc_await_oneshot,
        ripple_helper::{IRippleHelper, RippleHelper, RippleHelperFactory, RippleHelperType},
        rpc_util::rpc_err,
    },
    managers::capability_manager::{CapClassifiedRequest, IGetLoadedCaps, RippleHandlerCaps},
    platform_state::PlatformState,
};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GrantInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    app: Option<AppInfo>, //None in case of device
    state: String,
    capability: String,
    role: String,
    lifespan: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires: Option<String>, // Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GetUserGrantsByAppRequest {
    pub app_id: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GetUserGrantsByCapabilityRequest {
    pub capability: String,
}

#[rpc(server)]
pub trait UserGrants {
    #[method(name = "usergrants.app")]
    async fn usergrants_app(
        &self,
        ctx: CallContext,
        request: GetUserGrantsByAppRequest,
    ) -> RpcResult<Vec<GrantInfo>>;
    #[method(name = "usergrants.device")]
    async fn usergrants_device(&self, ctx: CallContext) -> RpcResult<Vec<GrantInfo>>;
    #[method(name = "usergrants.capability")]
    async fn usergrants_capability(
        &self,
        ctx: CallContext,
        request: GetUserGrantsByCapabilityRequest,
    ) -> RpcResult<Vec<GrantInfo>>;
}

#[derive(Debug)]
pub struct UserGrantsImpl<IRippleHelper> {
    pub helper: Box<IRippleHelper>,
    pub platform_state: PlatformState,
}
impl UserGrantsImpl<RippleHelper> {
    async fn get_app_title(&self, app_id: &str) -> RpcResult<Option<String>> {
        let (app_resp_tx, app_resp_rx) = oneshot::channel::<Result<AppManagerResponse, AppError>>();

        let app_request = AppRequest {
            method: AppMethod::GetAppName(app_id.into()),
            resp_tx: Some(app_resp_tx),
        };

        self.helper.send_app_request(app_request).await?;
        let resp = rpc_await_oneshot(app_resp_rx).await?;

        if let AppManagerResponse::AppName(app_title) = resp? {
            return Ok(app_title);
        }

        Err(rpc_err(format!(
            "Failed to get App Title for {}",
            app_id.to_owned()
        )))
    }

    async fn create_grantinfo_from_grant_entry_list(
        &self,
        app_id: Option<String>,
        grant_entries: &Vec<GrantEntry>,
    ) -> Vec<GrantInfo> {
        let app_name = match app_id.clone() {
            Some(id) => self.get_app_title(&id).await.ok().flatten(),
            None => None,
        };
        grant_entries
            .into_iter()
            .map(move |x| UserGrantsImpl::transform(app_id.clone(), app_name.clone(), &x))
            .collect()
    }

    //Transform GrantEntry to GrantInfo.  app_id None is for device.
    fn transform(
        app_id: Option<String>,
        app_name: Option<String>,
        entry: &GrantEntry,
    ) -> GrantInfo {
        GrantInfo {
            app: app_id.map(|x| AppInfo {
                id: x.to_owned(),
                title: app_name,
            }),
            state: entry.status.as_ref().unwrap().as_string().to_owned(),
            capability: entry.capability.to_owned(),
            role: entry.role.as_string().to_owned(),
            lifespan: entry.lifespan.as_ref().unwrap().as_string().to_owned(),
            expires: (|| {
                entry.lifespan_ttl_in_secs.map(|ttl_secs| {
                    let expiry_system_time: SystemTime = SystemTime::UNIX_EPOCH
                        + entry.last_modified_time
                        + Duration::from_secs(ttl_secs);
                    let expiry_date_time: DateTime<Utc> = DateTime::from(expiry_system_time);
                    expiry_date_time.to_rfc3339()
                })
            })(),
        }
    }
}

#[async_trait]
impl UserGrantsServer for UserGrantsImpl<RippleHelper> {
    #[instrument(skip(self))]
    async fn usergrants_app(
        &self,
        _ctx: CallContext,
        request: GetUserGrantsByAppRequest,
    ) -> RpcResult<Vec<GrantInfo>> {
        let grant_enrties = UserGrantStateUtils::get_grant_entries_for_app_id(
            &self.platform_state.grant_state,
            Some(request.app_id.clone()),
        );

        Ok(self
            .create_grantinfo_from_grant_entry_list(Some(request.app_id), &grant_enrties)
            .await)
    }
    #[instrument(skip(self))]
    async fn usergrants_device(&self, _ctx: CallContext) -> RpcResult<Vec<GrantInfo>> {
        let grant_enrties = UserGrantStateUtils::get_grant_entries_for_app_id(
            &self.platform_state.grant_state,
            None,
        );

        Ok(self
            .create_grantinfo_from_grant_entry_list(None, &grant_enrties)
            .await)
    }
    #[instrument(skip(self))]
    async fn usergrants_capability(
        &self,
        _ctx: CallContext,
        request: GetUserGrantsByCapabilityRequest,
    ) -> RpcResult<Vec<GrantInfo>> {
        let grant_enrtry_map = UserGrantStateUtils::get_grant_entries_for_capability(
            &self.platform_state.grant_state,
            &request.capability,
        );
        let mut combined_grant_entries: Vec<GrantInfo> = Vec::new();
        for (app_id, app_entries) in grant_enrtry_map.iter() {
            combined_grant_entries.extend(
                self.create_grantinfo_from_grant_entry_list(app_id.clone(), app_entries)
                    .await,
            );
        }
        Ok(combined_grant_entries)
    }
}

pub struct UserGrantsRippleProvider;
pub struct UserGrantsCapHandler;

impl IGetLoadedCaps for UserGrantsCapHandler {
    fn get_loaded_caps(&self) -> RippleHandlerCaps {
        RippleHandlerCaps {
            caps: Some(vec![CapClassifiedRequest::Supported(vec![])]),
        }
    }
}

impl RPCProvider<UserGrantsImpl<RippleHelper>, UserGrantsCapHandler> for UserGrantsRippleProvider {
    fn provide(
        self,
        rhf: Box<RippleHelperFactory>,
        platform_state: PlatformState,
    ) -> (
        RpcModule<UserGrantsImpl<RippleHelper>>,
        UserGrantsCapHandler,
    ) {
        let a = UserGrantsImpl {
            helper: rhf.clone().get(self.get_helper_variant()),
            platform_state,
        };
        (a.into_rpc(), UserGrantsCapHandler)
    }

    fn get_helper_variant(self) -> Vec<RippleHelperType> {
        vec![RippleHelperType::Dab, RippleHelperType::AppManager]
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        api::{
            handlers::user_grants::{
                GetUserGrantsByAppRequest, GetUserGrantsByCapabilityRequest, UserGrantsImpl,
                UserGrantsServer,
            },
            permissions::user_grants::tests,
            rpc::{api_messages::ApiProtocol, rpc_gateway::CallContext},
        },
        platform_state::PlatformState,
    };

    fn grant_call_ctx() -> CallContext {
        CallContext {
            app_id: "someapp".into(),
            method: "somemethod".into(),
            request_id: "somerequest".into(),
            session_id: "session".into(),
            call_id: 10,
            protocol: ApiProtocol::JsonRpc,
        }
    }

    #[tokio::test]
    pub async fn test_grant_query_apis() {
        let ps = PlatformState::default();
        tests::test_add_user_grant_entries(&ps).await;
        let user_grants_test = UserGrantsImpl {
            helper: Box::new(ps.clone().services),
            platform_state: ps.clone(),
        };

        let result = user_grants_test
            .usergrants_app(
                grant_call_ctx(),
                GetUserGrantsByAppRequest {
                    app_id: "Hulu".to_owned(),
                },
            )
            .await;
        assert!(result.is_ok() && !result.unwrap().is_empty());

        // No Entry for Xfinity Stream.
        let result = user_grants_test
            .usergrants_app(
                grant_call_ctx(),
                GetUserGrantsByAppRequest {
                    app_id: "Xfinity Stream".to_owned(),
                },
            )
            .await;
        assert!(result.is_ok() && result.unwrap().is_empty());

        let result = user_grants_test.usergrants_device(grant_call_ctx()).await;
        assert!(result.is_ok() && !result.unwrap().is_empty());

        let result = user_grants_test
            .usergrants_capability(
                grant_call_ctx(),
                GetUserGrantsByCapabilityRequest {
                    capability: "xrn:firebolt:capability:localization:postal-code".to_owned(),
                },
            )
            .await;
        assert!(result.is_ok() && !result.unwrap().is_empty());
    }
}
