use crate::{
    api::rpc::rpc_gateway::{CallContext, RPCProvider},
    apps::app_mgr::{AppError, AppManagerResponse, AppMethod, AppRequest},
    helpers::{
        crypto_util::{extract_app_ref, salt_using_app_scope},
        error_util::rpc_await_oneshot,
        ripple_helper::{IRippleHelper, RippleHelper, RippleHelperFactory, RippleHelperType},
        rpc_util::rpc_err,
    },
    managers::capability_manager::{
        Availability, CapAvailability, CapAvailabilityProvider, CapClassifiedRequest, CapRequest,
        FireboltCap, IGetLoadedCaps, RippleHandlerCaps,
    },
    platform_state::PlatformState,
};
use dab::core::{
    message::{DabRequestPayload, DabResponsePayload},
    model::distributor::{AccessTokenRequest, DistributorRequest, DistributorSession},
};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    RpcModule,
};
use tokio::sync::oneshot;
use tracing::{error, instrument};

#[rpc(server)]
pub trait Account {
    #[method(name = "account.session")]
    async fn session(&self, ctx: CallContext, a_t_r: AccessTokenRequest) -> RpcResult<()>;
    #[method(name = "account.id")]
    async fn id_rpc(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "account.uid")]
    async fn uid_rpc(&self, ctx: CallContext) -> RpcResult<String>;
}

#[derive(Debug, Clone)]
pub struct AccountImpl<IRippleHelper> {
    pub helper: Box<IRippleHelper>,
    pub platform_state: PlatformState,
}

#[derive(Debug, Clone)]
pub struct AccountSessionCamProvider {
    pub helper: Box<RippleHelper>,
}

pub async fn id(helper: Box<dyn IRippleHelper + Send + Sync>) -> RpcResult<String> {
    let resp = helper
        .send_dab(DabRequestPayload::Distributor(DistributorRequest::Session))
        .await;
    if let Err(_) = resp {
        return Err(rpc_err("Could not get distributor session from the device"));
    }
    if let None = resp.as_ref().unwrap().as_dist_session() {
        return Err(rpc_err("Device returned an invalid type for dist session"));
    }
    let session: DistributorSession = resp.unwrap().as_dist_session().unwrap();
    if let Some(account_id) = session.account_id {
        Ok(account_id)
    } else {
        Err(rpc_err("Account.uid: some failure"))
    }
}

pub async fn uid(
    account_id: String,
    helper: Box<dyn IRippleHelper + Send + Sync>,
    ctx: CallContext,
) -> RpcResult<String> {
    let (app_resp_tx, app_resp_rx) = oneshot::channel::<Result<AppManagerResponse, AppError>>();

    let app_request = AppRequest {
        method: AppMethod::GetStartPage(ctx.app_id.clone()),
        resp_tx: Some(app_resp_tx),
    };
    helper.send_app_request(app_request).await?;
    let resp = rpc_await_oneshot(app_resp_rx).await?;
    if let AppManagerResponse::StartPage(start_page) = resp? {
        let app_scope = match start_page {
            Some(sp) => extract_app_ref(sp),
            None => ctx.app_id,
        };
        return Ok(salt_using_app_scope(
            app_scope,
            account_id,
            helper.get_config().get_default_id_salt(),
        ));
    }

    Err(rpc_err("Account.uid: some failure"))
}

#[async_trait::async_trait]
impl CapAvailabilityProvider for AccountSessionCamProvider {
    type Cap = FireboltCap;
    fn get(self) -> FireboltCap {
        FireboltCap::Short("account:session".into())
    }
    async fn update_cam(self, ca: Availability) {
        if let Err(_) = self
            .helper
            .update_cap(CapRequest::UpdateAvailability(
                CapAvailability {
                    c: FireboltCap::Short("account:session".into()),
                    a: ca,
                },
                None,
            ))
            .await
        {
            error!("Updating cam")
        }
    }
}

#[async_trait]
impl AccountServer for AccountImpl<RippleHelper> {
    #[instrument(skip(self))]
    async fn session(&self, _ctx: CallContext, a_t_r: AccessTokenRequest) -> RpcResult<()> {
        let resp = self
            .helper
            .send_dab(DabRequestPayload::Distributor(
                DistributorRequest::SetSession(a_t_r),
            ))
            .await;

        // clear the cached distributor session
        self.platform_state
            .app_auth_sessions
            .clear_device_auth_session()
            .await;

        let scp = Box::new(AccountSessionCamProvider {
            helper: self.helper.clone(),
        });
        match resp {
            Ok(dab_payload) => match dab_payload {
                DabResponsePayload::None => {
                    // let the Capability Availability manager know that Session is now available
                    scp.update_cam(Availability::Ready).await;
                    Ok(())
                }
                _ => {
                    scp.update_cam(Availability::Failed).await;
                    Err(jsonrpsee::core::Error::Custom(String::from(
                        "Provision Status error response TBD",
                    )))
                }
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "Provision status error response TBD",
            ))),
        }
    }

    #[instrument(skip(self))]
    async fn id_rpc(&self, _ctx: CallContext) -> RpcResult<String> {
        id(self.helper.clone()).await
    }

    #[instrument(skip(self))]
    async fn uid_rpc(&self, ctx: CallContext) -> RpcResult<String> {
        if let Ok(account_id) = id(self.helper.clone()).await {
            uid(account_id, self.helper.clone(), ctx).await
        } else {
            Err(rpc_err("Account.uid: some failure"))
        }
    }
}

pub struct AccountRippleProvider;
#[derive(Debug)]
pub struct AccountCapHandler;

#[async_trait]
impl IGetLoadedCaps for AccountCapHandler {
    fn get_loaded_caps(&self) -> RippleHandlerCaps {
        RippleHandlerCaps {
            caps: Some(vec![
                CapClassifiedRequest::Supported(vec![
                    FireboltCap::Short("token:account".into()),
                    FireboltCap::Short("account:id".into()),
                    FireboltCap::Short("account:uid".into()),
                ]),
                // For Session when the device starts up there is no session then it becomes available from provider
                CapClassifiedRequest::NotAvailable(vec![FireboltCap::Short(
                    "token:account".into(),
                )]),
            ]),
        }
    }
}

impl RPCProvider<AccountImpl<RippleHelper>, AccountCapHandler> for AccountRippleProvider {
    fn provide(
        self,
        rhf: Box<RippleHelperFactory>,
        platform_state: PlatformState,
    ) -> (RpcModule<AccountImpl<RippleHelper>>, AccountCapHandler) {
        let a = AccountImpl {
            helper: rhf.get(self.get_helper_variant()),
            platform_state,
        };
        (a.into_rpc(), AccountCapHandler)
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

