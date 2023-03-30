use jsonrpsee::{proc_macros::rpc, core::RpcResult};
use ripple_sdk::api::{gateway::rpc_gateway_api::CallContext, firebolt::fb_user_grants::{GetUserGrantsByAppRequest, GetUserGrantsByCapabilityRequest, GrantInfo}};

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