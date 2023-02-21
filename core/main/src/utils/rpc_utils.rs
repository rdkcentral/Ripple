use jsonrpsee::core::Error;

pub fn rpc_err(msg: impl Into<String>) -> Error {
    Error::Custom(msg.into())
}
