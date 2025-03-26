use crate::{state::bootstrap_state::BootstrapState, SEMVER_LIGHTWEIGHT};
use ripple_sdk::{
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
        //let device_manifest = &state.platform_state.device_manifest.configuration;
        let manifest = state.platform_state.get_device_manifest();
        let log_signal_level = match manifest
            .configuration
            .log_signal_log_level
            .to_lowercase()
            .as_str()
        {
            "info" => log::LevelFilter::Info,
            "debug" => log::LevelFilter::Debug,
            "trace" => log::LevelFilter::Trace,
            _ => log::LevelFilter::Info,
        };

        let _ = init_and_configure_logger(
            SEMVER_LIGHTWEIGHT,
            "gateway".into(),
            Some(vec![(
                "ripple_sdk::api::observability::log_signal".to_string(),
                log_signal_level,
            )]),
        );

        Ok(())
    }
}
