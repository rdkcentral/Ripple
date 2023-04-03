use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use ripple_sdk::api::{
    firebolt::fb_user_grants::{
        GetUserGrantsByAppRequest, GetUserGrantsByCapabilityRequest, GrantInfo,
    },
    gateway::rpc_gateway_api::CallContext,
};

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
