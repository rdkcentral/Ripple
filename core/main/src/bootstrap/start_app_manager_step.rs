use ripple_sdk::async_trait::async_trait;
use ripple_sdk::{framework::bootstrap::Bootstep, tokio, utils::error::RippleError};

use crate::{
    service::apps::delegated_launcher_handler::DelegatedLauncherHandler,
    state::bootstrap_state::BootstrapState,
};

/// Starts the App Manager and other supporting services
pub struct StartAppManagerStep;

#[async_trait]
impl Bootstep<BootstrapState> for StartAppManagerStep {
    fn get_name(&self) -> String {
        "StartAppManager".into()
    }

    async fn setup(&self, state: BootstrapState) -> Result<(), RippleError> {
        let mut app_manager =
            DelegatedLauncherHandler::new(state.channels_state, state.platform_state);
        tokio::spawn(async move {
            app_manager.start().await;
        });
        Ok(())
    }
}
