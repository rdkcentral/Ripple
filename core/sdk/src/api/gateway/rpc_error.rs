pub trait RpcError {
    type E;
    fn get_rpc_error_code(&self) -> i32;
    fn get_rpc_error_message(&self, error_type: Self::E) -> String;
}
