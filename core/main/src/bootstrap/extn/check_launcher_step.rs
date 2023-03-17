use ripple_sdk::{
    async_trait::async_trait,
    framework::{bootstrap::Bootstep, RippleResponse},
};

use crate::{
    processor::lifecycle_management_processor::LifecycleManagementProcessor,
    state::bootstrap_state::BootstrapState,
};

/// Bootstep which checks if the given run has the launcher channel and starts,
/// This step calls the start method on the Launcher Channel and waits for a successful Status
/// connection before proceeding to the next boot step.
pub struct CheckLauncherStep;

#[async_trait]
impl Bootstep<BootstrapState> for CheckLauncherStep {
    fn get_name(&self) -> String {
        "CheckLauncherStep".into()
    }
    async fn setup(&self, state: BootstrapState) -> RippleResponse {
        if state.platform_state.has_internal_launcher() {
            state.platform_state.get_client().add_request_processor(
                LifecycleManagementProcessor::new(state.platform_state.get_client()),
            );
        }
        Ok(())
    }
}
