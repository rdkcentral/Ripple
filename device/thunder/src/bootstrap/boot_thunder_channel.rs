use thunder_ripple_sdk::{
    bootstrap::boot_thunder::boot_thunder,
    ripple_sdk::{extn::client::extn_client::ExtnClient, log::info},
};

use crate::bootstrap::setup_thunder_processors::SetupThunderProcessor;

pub async fn boot_thunder_channel(state: ExtnClient) {
    info!("Booting thunder");
    let state = boot_thunder(state).await;
    SetupThunderProcessor::setup(state)
        .await
        .expect(&SetupThunderProcessor::get_name());
}
