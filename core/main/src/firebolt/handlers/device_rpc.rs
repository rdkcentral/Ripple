use crate::{firebolt::rpc::RippleRPCProvider, state::platform_state::PlatformState};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    RpcModule,
};
use ripple_sdk::api::gateway::rpc_gateway_api::CallContext;

#[rpc(server)]
pub trait Device {
    #[method(name = "device.type")]
    async fn typ(&self, ctx: CallContext) -> RpcResult<String>;
}

#[derive(Debug)]
pub struct DeviceImpl {
    pub state: PlatformState,
}

#[async_trait]
impl DeviceServer for DeviceImpl {
    async fn typ(&self, _ctx: CallContext) -> RpcResult<String> {
        Ok(self.state.get_device_manifest().get_form_factor())
    }
}

pub struct DeviceRPCProvider;
impl RippleRPCProvider<DeviceImpl> for DeviceRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<DeviceImpl> {
        (DeviceImpl { state }).into_rpc()
    }
}
