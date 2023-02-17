use jsonrpsee::RpcModule;

use crate::service::extn::ripple_client::RippleClient;

pub trait RippleRPCProvider<I> {
    fn provide(client: RippleClient) -> RpcModule<I>;
}
