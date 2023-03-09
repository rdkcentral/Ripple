use crate::{
    firebolt::rpc::RippleRPCProvider, state::platform_state::PlatformState,
    utils::rpc_utils::rpc_err,
};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    RpcModule,
};
use ripple_sdk::{
    api::{device::device_info_request::DeviceInfoRequest, gateway::rpc_gateway_api::CallContext},
    extn::extn_client_message::ExtnResponse,
};

#[rpc(server)]
pub trait Device {
    #[method(name = "device.make")]
    async fn make(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "device.model")]
    async fn model(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "device.type")]
    async fn typ(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "device.name")]
    async fn name(&self, ctx: CallContext) -> RpcResult<String>;
}

#[derive(Debug)]
pub struct DeviceImpl {
    pub state: PlatformState,
}

#[async_trait]
impl DeviceServer for DeviceImpl {
    async fn make(&self, _ctx: CallContext) -> RpcResult<String> {
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(DeviceInfoRequest::Make)
            .await
        {
            if let Some(ExtnResponse::String(v)) = response.payload.clone().extract() {
                return Ok(v);
            }
        }
        Err(rpc_err("FB error response TBD"))
    }

    async fn model(&self, _ctx: CallContext) -> RpcResult<String> {
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(DeviceInfoRequest::Model)
            .await
        {
            if let Some(ExtnResponse::String(v)) = response.payload.clone().extract() {
                return Ok(v);
            }
        }
        Err(rpc_err("FB error response TBD"))
    }

    async fn name(&self, _ctx: CallContext) -> RpcResult<String> {
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(DeviceInfoRequest::Name)
            .await
        {
            if let Some(ExtnResponse::String(v)) = response.payload.clone().extract() {
                return Ok(v);
            }
        }
        Err(rpc_err("FB error response TBD"))
    }

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
