use ripple_sdk::{
    api::status_update::ExtnStatus,
    log::{info, warn},
    utils::error::RippleError,
};

use crate::{
    client::{plugin_manager::PluginManager, thunder_client_pool::ThunderClientPool},
    thunder_state::{
        ThunderBootstrapStateWithClient, ThunderBootstrapStateWithConfig, ThunderState,
    },
};

pub struct ThunderPoolStep;

impl ThunderPoolStep {
    pub fn get_name() -> String {
        "ThunderPoolStep".into()
    }

    pub async fn setup(
        mut state: ThunderBootstrapStateWithConfig,
    ) -> Result<ThunderBootstrapStateWithClient, RippleError> {
        let pool_size = state.pool_size.clone();
        let url = state.url.clone();
        if pool_size < 2 {
            warn!("Pool size of 1 is not recommended, there will be no dedicated connection for Controller events");
            return Err(RippleError::BootstrapError);
        }
        let controller_pool = ThunderClientPool::start(url.clone(), None, 1);
        let plugin_manager_tx = PluginManager::start(Box::new(controller_pool)).await;
        let client = ThunderClientPool::start(url, Some(plugin_manager_tx), pool_size - 1);
        info!("Thunder client connected successfully");
        let _ = state.extn_client.event(ExtnStatus::Ready).await;
        let extn_client = state.extn_client.clone();
        Ok(ThunderBootstrapStateWithClient {
            prev: state,
            state: ThunderState::new(extn_client, client),
        })
    }
}
