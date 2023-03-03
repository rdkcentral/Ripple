use ripple_sdk::{
    api::status_update::ExtnStatus,
    async_trait::async_trait,
    extn::client::wait_for_service_processor::WaitForStatusReadyEventProcessor,
    framework::{bootstrap::Bootstep, RippleResponse},
    log::{debug, error},
    tokio::sync::mpsc::channel,
};

use crate::state::bootstrap_state::{BootstrapState, ChannelsState};

/// Bootstep which checks if the given run has the launcher channel and starts,
/// This step calls the start method on the Launcher Channel and waits for a successful Status
/// connection before proceeding to the next boot step.
pub struct CheckLauncherStep;

#[async_trait]
impl Bootstep<BootstrapState> for CheckLauncherStep {
    fn get_name(&self) -> String {
        "StartDeviceChannel".into()
    }
    async fn setup(&self, mut state: BootstrapState) -> RippleResponse {
        if state.platform_state.has_internal_launcher() {
            let extn_capability = state.platform_state.get_launcher_capability().unwrap();
            let client = state.platform_state.get_client();

            if let Err(e) = state.extn_state.start(
                extn_capability.clone(),
                ChannelsState::get_crossbeam_channel(),
                client,
            ) {
                error!("Error during Device channel bootstrap");
                return Err(e);
            }

            let (tx, mut tr) = channel::<ExtnStatus>(1);
            state.platform_state.get_client().add_event_processor(
                WaitForStatusReadyEventProcessor::new(extn_capability.clone(), tx),
            );

            if let Some(_) = tr.recv().await {
                debug!("Launcher ready");
                state
                    .platform_state
                    .get_client()
                    .cleanup_event_processor(extn_capability);
                return Ok(());
            }
        }
        Ok(())
    }
}
