use jsonrpsee::RpcModule;

use crate::state::platform_state::PlatformState;

pub trait RippleRPCProvider<I> {
    fn provide(state: PlatformState) -> RpcModule<I>;
}
