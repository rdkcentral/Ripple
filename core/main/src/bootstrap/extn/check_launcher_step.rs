use ripple_sdk::{
    api::{
        apps::{AppMethod, AppRequest, AppResponse},
        firebolt::fb_discovery::{DiscoveryContext, LaunchRequest, NavigationIntent},
    },
    async_trait::async_trait,
    framework::{bootstrap::Bootstep, RippleResponse},
    tokio::sync::oneshot,
    utils::error::RippleError,
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
            let app = state
                .platform_state
                .app_library_state
                .get_default_app()
                .expect("Default app to be available in app library");
            let (app_resp_tx, app_resp_rx) = oneshot::channel::<AppResponse>();

            let app_request = AppRequest::new(
                AppMethod::Launch(LaunchRequest {
                    app_id: app.app_id,
                    intent: Some(NavigationIntent {
                        action: "boot".into(),
                        data: None,
                        context: DiscoveryContext::new("device"),
                    }),
                }),
                app_resp_tx,
            );
            state
                .platform_state
                .get_client()
                .send_app_request(app_request)
                .expect("App Request to be sent successfully");

            if let Err(_) = app_resp_rx.await {
                return Err(RippleError::BootstrapError);
            }
        }
        Ok(())
    }
}
