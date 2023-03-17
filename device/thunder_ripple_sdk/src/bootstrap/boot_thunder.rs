use crate::thunder_state::ThunderBootstrapStateWithClient;
use ripple_sdk::{extn::client::extn_client::ExtnClient, log::info};

use super::{get_config_step::ThunderGetConfigStep, setup_thunder_pool_step::ThunderPoolStep};

pub async fn boot_thunder(state: ExtnClient) -> ThunderBootstrapStateWithClient {
    info!("Booting thunder");
    let state = ThunderGetConfigStep::setup(state)
        .await
        .expect(&ThunderGetConfigStep::get_name());
    ThunderPoolStep::setup(state)
        .await
        .expect(&ThunderPoolStep::get_name())
}
