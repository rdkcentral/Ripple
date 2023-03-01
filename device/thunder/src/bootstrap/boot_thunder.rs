use ripple_sdk::log::info;

use crate::{
    bootstrap::{
        setup_thunder_extns::SetupThunderExtns, setup_thunder_processors::SetupThunderProcessor,
    },
    thunder_state::ThunderBootstrapStateInitial,
};

use super::{get_config_step::ThunderGetConfigStep, setup_thunder_pool_step::ThunderPoolStep};

pub async fn boot(state: ThunderBootstrapStateInitial) {
    info!("Booting thunder");
    let state = ThunderGetConfigStep::setup(state)
        .await
        .expect(&ThunderGetConfigStep::get_name());
    let state = ThunderPoolStep::setup(state)
        .await
        .expect(&ThunderPoolStep::get_name());
    let state = SetupThunderProcessor::setup(state)
        .await
        .expect(&SetupThunderProcessor::get_name());
    SetupThunderExtns::setup(state)
        .await
        .expect(&SetupThunderExtns::get_name())
}
