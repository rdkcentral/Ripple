use jsonrpsee::core::{Error, RpcResult};
use ripple_sdk::tokio::sync::oneshot;

pub fn rpc_err(msg: impl Into<String>) -> Error {
    Error::Custom(msg.into())
}

/// Awaits a oneshot to respond. If the oneshot fails to repond, creates a generic
/// RPC internal error
pub async fn rpc_await_oneshot<T>(rx: oneshot::Receiver<T>) -> RpcResult<T> {
    match rx.await {
        Ok(v) => Ok(v),
        Err(e) => Err(jsonrpsee::core::error::Error::Custom(format!(
            "Internal failure: {:?}",
            e
        ))),
    }
}
