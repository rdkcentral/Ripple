use crate::{
    api::rpc::rpc_gateway::{CallContext, RPCProvider},
    helpers::{
        error_util::CAPABILITY_NOT_SUPPORTED,
        ripple_helper::{IRippleHelper, RippleHelper, RippleHelperFactory, RippleHelperType},
        session_util::{dab_to_dpab, get_distributor_session_from_platform_state},
    },
    managers::capability_manager::{
        CapClassifiedRequest, FireboltCap, IGetLoadedCaps, RippleHandlerCaps,
    },
    platform_state::PlatformState,
};
use dab::core::model::secure_storage::{
    GetRequest, RemoveRequest, SetRequest, StorageOptions, StorageScope,
};
use dpab::core::{
    message::{DpabRequestPayload, DpabResponsePayload},
    model::secure_storage::{
        SecureStorageGetRequest, SecureStorageRemoveRequest, SecureStorageResponse,
        SecureStorageSetRequest, StorageScope as DpabStorageScope,
        StorageSetOptions as DpabStorageSetOptions,
    },
};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    RpcModule,
};
use tracing::{error, info};
#[rpc(server)]
pub trait SecureStorage {
    #[method(name = "securestorage.get")]
    async fn get(&self, ctx: CallContext, request: GetRequest) -> RpcResult<String>;
    #[method(name = "securestorage.set")]
    async fn set(&self, ctx: CallContext, request: SetRequest) -> RpcResult<()>;
    #[method(name = "securestorage.remove")]
    async fn remove(&self, ctx: CallContext, request: RemoveRequest) -> RpcResult<()>;
}
pub struct SecureStorageImpl<IRippleHelper> {
    pub ripple_helper: Box<IRippleHelper>,
    pub platform_state: PlatformState,
}
fn dab_scope_to_dpab_scope(dab_scope: &StorageScope) -> DpabStorageScope {
    match dab_scope {
        StorageScope::Device => DpabStorageScope::Device,
        StorageScope::Account => DpabStorageScope::Account,
    }
}
fn dab_options_to_dpab_options(dab_storage_options: &StorageOptions) -> DpabStorageSetOptions {
    DpabStorageSetOptions {
        ttl: dab_storage_options.ttl,
    }
}

#[async_trait]
impl SecureStorageServer for SecureStorageImpl<RippleHelper> {
    async fn get(&self, ctx: CallContext, request: GetRequest) -> RpcResult<String> {
        match self
            .ripple_helper
            .send_dpab(dpab::core::message::DpabRequestPayload::SecureStorage(
                dpab::core::model::secure_storage::SecureStorageRequest::Get(
                    SecureStorageGetRequest {
                        app_id: ctx.app_id,
                        scope: dab_scope_to_dpab_scope(&request.scope),
                        key: request.key,
                        distributor_session: dab_to_dpab(
                            get_distributor_session_from_platform_state(&self.platform_state)
                                .await?,
                        )
                        .unwrap(),
                    },
                ),
            ))
            .await
        {
            Ok(ok) => {
                if let Some(response) = ok.as_secure_storage_response() {
                    if let Some(get_value) = response.as_get_response() {
                        Ok(get_value.clone().value.unwrap_or(String::from("")))
                    } else {
                        Err(jsonrpsee::core::Error::Custom(
                            "Error getting value".to_owned(),
                        ))
                    }
                } else {
                    Err(jsonrpsee::core::Error::Custom(
                        "Error getting value".to_owned(),
                    ))
                }
            }
            Err(err) => {
                error!("error={:?}", err);
                Err(jsonrpsee::core::Error::Custom(
                    "Error getting value".to_owned(),
                ))
            }
        }
    }

    async fn set(&self, ctx: CallContext, request: SetRequest) -> RpcResult<()> {
        match self
            .ripple_helper
            .send_dpab(dpab::core::message::DpabRequestPayload::SecureStorage(
                dpab::core::model::secure_storage::SecureStorageRequest::Set(
                    SecureStorageSetRequest {
                        app_id: ctx.app_id,
                        value: request.value,
                        options: dab_options_to_dpab_options(&request.options),
                        scope: dab_scope_to_dpab_scope(&request.scope),
                        key: request.key,
                        distributor_session: dab_to_dpab(
                            get_distributor_session_from_platform_state(&self.platform_state)
                                .await?,
                        )
                        .unwrap(),
                    },
                ),
            ))
            .await
        {
            Ok(_) => Ok(()),
            Err(err) => {
                error!("error={:?}", err);
                Err(jsonrpsee::core::Error::Custom(
                    "Error setting value".to_owned(),
                ))
            }
        }
    }
    async fn remove(&self, ctx: CallContext, request: RemoveRequest) -> RpcResult<()> {
        match self
            .ripple_helper
            .send_dpab(dpab::core::message::DpabRequestPayload::SecureStorage(
                dpab::core::model::secure_storage::SecureStorageRequest::Remove(
                    SecureStorageRemoveRequest {
                        app_id: ctx.app_id,
                        scope: dab_scope_to_dpab_scope(&request.scope),
                        key: request.key,
                        distributor_session: dab_to_dpab(
                            get_distributor_session_from_platform_state(&self.platform_state)
                                .await?,
                        )
                        .unwrap(),
                    },
                ),
            ))
            .await
        {
            Ok(_) => Ok(()),
            Err(err) => {
                error!("error={:?}", err);
                Err(jsonrpsee::core::Error::Custom(
                    "Error setting value".to_owned(),
                ))
            }
        }
    }
}

pub struct SecureStorageRippleProvider;

pub struct SecureStorageCapHandler;

impl IGetLoadedCaps for SecureStorageCapHandler {
    fn get_loaded_caps(&self) -> RippleHandlerCaps {
        RippleHandlerCaps {
            caps: Some(vec![CapClassifiedRequest::Supported(vec![
                FireboltCap::Short("accessory:pair".into()),
                FireboltCap::Short("accessory:list".into()),
            ])]),
        }
    }
}

impl RPCProvider<SecureStorageImpl<RippleHelper>, SecureStorageCapHandler>
    for SecureStorageRippleProvider
{
    fn provide(
        self,
        rhf: Box<RippleHelperFactory>,
        platform_state: PlatformState,
    ) -> (
        RpcModule<SecureStorageImpl<RippleHelper>>,
        SecureStorageCapHandler,
    ) {
        let a = SecureStorageImpl {
            ripple_helper: rhf.get(self.get_helper_variant()),
            platform_state: platform_state.clone(),
        };
        (a.into_rpc(), SecureStorageCapHandler)
    }

    fn get_helper_variant(self) -> Vec<RippleHelperType> {
        vec![
            RippleHelperType::Dab,
            RippleHelperType::Cap,
            RippleHelperType::Dpab,
        ]
    }
}
