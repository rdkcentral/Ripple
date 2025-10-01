use crate::{state::bootstrap_state::BootstrapState, SEMVER_LIGHTWEIGHT};
use ripple_sdk::{
    api::observability::log_signal::{determine_log_level, LogSignal},
    async_trait::async_trait,
    framework::{bootstrap::Bootstep, RippleResponse},
    log,
    utils::logger::init_and_configure_logger,
};

pub struct LoggingBootstrapStep;

#[async_trait]
impl Bootstep<BootstrapState> for LoggingBootstrapStep {
    fn get_name(&self) -> String {
        "LoggingBootstrapStep".into()
    }

    async fn setup(&self, state: BootstrapState) -> RippleResponse {
        let manifest = state.platform_state.get_device_manifest();
        let log_signal_level: log::LevelFilter = manifest
            .configuration
            .log_signal_log_level
            .parse()
            .unwrap_or(log::LevelFilter::Info);
        println!(
            "LoggingBootstrapStep, log_signal_level={}",
            log_signal_level
        );
        let _ = init_and_configure_logger(
            SEMVER_LIGHTWEIGHT,
            "gateway".into(),
            Some(vec![(
                "ripple_sdk::api::observability::log_signal".to_string(),
                determine_log_level(log_signal_level),
            )]),
        );

        Ok(())
    }
}
