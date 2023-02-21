use crate::{
    firebolt::rpc::RippleRPCProvider, service::extn::ripple_client::RippleClient,
};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    RpcModule,
};
use ripple_sdk::{
    api::{ gateway::rpc_gateway_api::CallContext},
};

#[rpc(server)]
pub trait Device {
    #[method(name = "device.make")]
    async fn make(&self, ctx: CallContext) -> RpcResult<String>;
}

#[derive(Debug)]
pub struct DeviceImpl {
   pub client: RippleClient,
}

#[async_trait]
impl DeviceServer for DeviceImpl {
    async fn make(&self, _ctx: CallContext) -> RpcResult<String> {
        Ok("Device Make".into())
    }
}

pub struct DeviceRPCProvider;
impl RippleRPCProvider<DeviceImpl> for DeviceRPCProvider {
    fn provide(client: RippleClient) -> RpcModule<DeviceImpl> {
        (DeviceImpl { client }).into_rpc()
    }
}
