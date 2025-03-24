use crate::state::bootstrap_state::BootstrapState;
use ripple_sdk::utils::logger::set_log_level;
use ripple_sdk::{
    async_trait::async_trait,
    framework::{bootstrap::Bootstep, RippleResponse},
};

pub struct LoggingBootstrapStep;

#[async_trait]
impl Bootstep<BootstrapState> for LoggingBootstrapStep {
    fn get_name(&self) -> String {
        "LoggingBootstrapStep".into()
    }

    async fn setup(&self, state: BootstrapState) -> RippleResponse {
        let device_manifest = &state.platform_state.device_manifest.configuration;

        //set log signal log level
        let log_signal_log_level = device_manifest.log_signal_log_level.clone();
        set_log_level(&log_signal_log_level);

        Ok(())
    }
}
