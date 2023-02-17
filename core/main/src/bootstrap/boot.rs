use ripple_sdk::framework::bootstrap::Bootstrap;

use crate::state::platform_state::PlatformState;

use super::{
    manifest::{device::LoadDeviceManifestStep},
    start_fbgateway_step::FireboltGatewayStep,
    start_ws_step::StartWsStep,
};
/// Starts up Ripple uses `PlatformState` to manage State
/// # Arguments
/// * `platform_state` - PlatformState
/// 
/// # Panics
///
/// Bootstrap panics are fatal in nature and it could happen due to bad configuration or device state. Logs should provide more information on which step the failure had occurred.
/// 
/// # Steps
///
/// 1. [LoadDeviceManifestStep] - Loads up Device manifest and updates the app state.
/// 2. [StartWsStep] - Starts the Websocket to accept external and internal connections
/// 3. [FireboltGatewayStep] - Starts the firebolt gateway and blocks the thread to keep it alive till interruption.
/// 
pub async fn boot(platform_state: PlatformState) {
    let bootstrap = &Bootstrap::new(platform_state);
    bootstrap
        .step(LoadDeviceManifestStep)
        .await
        .unwrap()
        .step(StartWsStep)
        .await
        .unwrap()
        .step(FireboltGatewayStep)
        .await
        .unwrap();
    // -- capability Manager
    // -- App event manager
    // -- User grant manager
    // -- permissions manager
    // -- app manager
    // Start Launcher
    // Start Dpab
    // Start Launcher
}
