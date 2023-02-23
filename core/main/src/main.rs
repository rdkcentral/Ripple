use crate::bootstrap::boot::boot;
use ripple_sdk::{tokio, utils::logger::init_logger};
use state::bootstrap_state::BootstrapState;
pub mod bootstrap;
pub mod firebolt;
pub mod processor;
pub mod service;
pub mod state;
pub mod utils;

#[tokio::main]
async fn main() {
    // Init logger
    if let Err(e) = init_logger("gateway".into()) {
        println!("{:?} logger init error", e);
        return;
    }
    let bootstate = BootstrapState::build().expect("Failure to init state for bootstrap");
    // bootstrap
    boot(bootstate).await
}
