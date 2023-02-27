use ripple_sdk::{framework::bootstrap::Bootstrap, log::info};

use crate::{
    bootstrap::{
        setup_thunder_extns::SetupThunderExtns, setup_thunder_processors::SetupThunderProcessor,
    },
    thunder_state::ThunderBootstrapState,
};

use super::{get_config_step::ThunderGetConfigStep, setup_thunder_pool_step::ThunderPoolStep};

pub async fn boot(state: ThunderBootstrapState) {
    info!("Booting thunder");
    Bootstrap::new(state)
        .step(ThunderGetConfigStep)
        .await
        .unwrap()
        .step(ThunderPoolStep)
        .await
        .unwrap()
        .step(SetupThunderProcessor)
        .await
        .unwrap()
        .step(SetupThunderExtns)
        .await
        .unwrap();
}
