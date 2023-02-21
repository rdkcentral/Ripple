use ripple_sdk::{
    async_trait::async_trait, framework::bootstrap::Bootstep, tokio, utils::error::RippleError,
};

use crate::{firebolt::firebolt_ws::FireboltWs, state::platform_state::PlatformState};

pub struct StartWsStep;

#[async_trait]
impl Bootstep<PlatformState> for StartWsStep {
    fn get_name(&self) -> String {
        "StartWsStep".into()
    }

    async fn setup(&self, app_state: PlatformState) -> Result<(), RippleError> {
        let manifest = app_state.get_device_manifest();
        let iai = manifest.get_internal_app_id();
        let ws_enabled = manifest.get_web_socket_enabled();
        let internal_ws_enabled = manifest.get_internal_ws_enabled();
        let iai_c = iai.clone();
        if ws_enabled {
            let ws_addr = manifest.clone().get_ws_gateway_host();
            let state_for_ws = app_state.clone();
            tokio::spawn(async move {
                FireboltWs::start(ws_addr.as_str(), state_for_ws, true, iai.clone()).await;
            });
        }

        if internal_ws_enabled {
            let ws_addr = manifest.clone().get_internal_gateway_host();
            let state_for_ws = app_state.clone();
            tokio::spawn(async move {
                FireboltWs::start(ws_addr.as_str(), state_for_ws, false, iai_c).await;
            });
        }

        Ok(())
    }
}
