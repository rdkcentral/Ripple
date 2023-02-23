use crate::{bootstrap::boot::boot, state::platform_state::PlatformState};
use ripple_sdk::{tokio, utils::logger::init_logger};
pub mod bootstrap;
pub mod firebolt;
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
    // bootstrap
    boot(PlatformState::default()).await
}
