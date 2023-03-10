use jsonrpsee::{proc_macros::rpc, core::RpcResult};
use ripple_sdk::api::{gateway::rpc_gateway_api::CallContext, firebolt::{fb_capabilities::{RoleInfo, CapInfoRpcRequest, CapabilityInfo, CapRequestRpcRequest, CapListenRPCRequest, FireboltCap}, fb_general::ListenerResponse}};
use ripple_sdk::async_trait::async_trait;

use crate::state::platform_state::PlatformState;

#[rpc(server)]
pub trait Capability {
    #[method(name = "capabilities.supported")]
    async fn supported(&self, ctx: CallContext, cap: RoleInfo) -> RpcResult<bool>;
    #[method(name = "capabilities.available")]
    async fn available(&self, ctx: CallContext, cap: RoleInfo) -> RpcResult<bool>;
    #[method(name = "capabilities.permitted")]
    async fn permitted(&self, ctx: CallContext, cap: RoleInfo) -> RpcResult<bool>;
    #[method(name = "capabilities.granted")]
    async fn granted(&self, ctx: CallContext, cap: RoleInfo) -> RpcResult<bool>;
    #[method(name = "capabilities.info")]
    async fn info(
        &self,
        ctx: CallContext,
        capabilities: CapInfoRpcRequest,
    ) -> RpcResult<Vec<CapabilityInfo>>;
    #[method(name = "capabilities.request")]
    async fn request(
        &self,
        ctx: CallContext,
        grants: CapRequestRpcRequest,
    ) -> RpcResult<Vec<CapabilityInfo>>;
    #[method(name = "capabilities.onAvailable")]
    async fn on_available(
        &self,
        ctx: CallContext,
        cap: CapListenRPCRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "capabilities.onUnavailable")]
    async fn on_unavailable(
        &self,
        ctx: CallContext,
        cap: CapListenRPCRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "capabilities.onGranted")]
    async fn on_granted(
        &self,
        ctx: CallContext,
        cap: CapListenRPCRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "capabilities.onRevoked")]
    async fn on_revoked(
        &self,
        ctx: CallContext,
        cap: CapListenRPCRequest,
    ) -> RpcResult<ListenerResponse>;
}

pub struct CapabilityImpl{
    state: PlatformState
}

// #[async_trait]
// impl CapabilityServer for CapabilityImpl {
//     async fn supported(&self, ctx: CallContext, cap: RoleInfo) -> RpcResult<bool> {
//         Ok(self.state.cap_state.check_supported(vec![FireboltCap::Full(
//             cap.capability,
//         )]).is_ok())
//     }

//     async fn available(&self, ctx: CallContext, cap: RoleInfo) -> RpcResult<bool> {
//         Ok(self.state.cap_state.check_available(vec![FireboltCap::Full(
//             cap.capability,
//         )]).is_ok())
//     }

//     async fn permitted(&self, ctx: CallContext, cap: RoleInfo) -> RpcResult<bool> {
//         Ok(self.state.permitted_state.check_cap_role(&ctx.app_id, cap))
//     }

//     async fn granted(&self, ctx: CallContext, cap: RoleInfo) -> RpcResult<bool> {
//         // TODO implement along with user grants
//         Ok(true)
//     }

//     async fn info(
//         &self,
//         ctx: CallContext,
//         capabilities: CapInfoRpcRequest,
//     ) -> RpcResult<Vec<CapabilityInfo>> {

//     }
// }