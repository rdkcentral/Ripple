use ripple_sdk::{
    api::status_update::ExtnStatus,
    async_trait::async_trait,
    framework::bootstrap::Bootstep,
    log::{info, warn},
    utils::error::RippleError,
};

use crate::{
    client::{plugin_manager::PluginManager, thunder_client_pool::ThunderClientPool},
    thunder_state::ThunderBootstrapState,
};

pub struct ThunderPoolStep;

#[async_trait]
impl Bootstep<ThunderBootstrapState> for ThunderPoolStep {
    fn get_name(&self) -> String {
        "ThunderPoolStep".into()
    }

    async fn setup(&self, state: ThunderBootstrapState) -> Result<(), RippleError> {
        let pool_size = state.get_pool_size();
        let url = state.get_url();
        if pool_size < 2 {
            warn!("Pool size of 1 is not recommended, there will be no dedicated connection for Controller events");
            return Err(RippleError::BootstrapError);
        }
        let controller_pool = ThunderClientPool::start(url.clone(), None, 1);
        let plugin_manager_tx = PluginManager::start(Box::new(controller_pool)).await;
        let client = ThunderClientPool::start(url, Some(plugin_manager_tx), pool_size - 1);
        state.get().set_thunder_client(client);
        info!("Thunder client connected successfully");
        let _ = state.get().get_client().event(ExtnStatus::Ready).await;
        Ok(())
    }
}
