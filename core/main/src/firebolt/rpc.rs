use jsonrpsee::core::Error;
use jsonrpsee::RpcModule;

use crate::state::platform_state::PlatformState;

pub trait RippleRPCProvider<I> {
    fn provide(state: PlatformState) -> RpcModule<I>;
}

pub fn rpc_err(msg: impl Into<String>) -> Error {
    Error::Custom(msg.into())
}
