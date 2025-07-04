pub mod errors;
pub mod mock_config;
pub mod mock_data;
pub mod mock_device_controller;
pub mod mock_device_ffi;
pub mod mock_server;
pub mod mock_web_socket_server;

pub mod utils;
extern crate tokio;

#[tokio::main(worker_threads = 2)]
async fn main() {
    mock_device_ffi::start_service().await;
}
