use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use ripple_sdk::{
    api::{
        gateway::rpc_gateway_api::CallContext,
    },
    extn::client::extn_client::ExtnClient,
};

pub struct CustomImpl {
    _client: ExtnClient,
}

impl CustomImpl {
    pub fn new(_client: ExtnClient) -> CustomImpl {
        CustomImpl { _client }
    }
}

#[rpc(server)]
pub trait Custom {
    #[method(name = "custom.info")]
    fn info(&self, ctx: CallContext) -> RpcResult<String>;
}

impl CustomServer for CustomImpl {
    fn info(&self, _ctx: CallContext) -> RpcResult<String> {
        Ok("Custom".into())
    }
}
