use ripple_sdk::{
    api::status_update::ExtnStatus,
    async_trait::async_trait,
    extn::client::wait_for_service_processor::WaitForStatusReadyEventProcessor,
    framework::bootstrap::Bootstep,
    log::{debug, error},
    tokio::sync::mpsc::channel,
    utils::error::RippleError,
};

use crate::state::bootstrap_state::{BootstrapState, ChannelsState};

/// Bootstep which starts the Distributor channel intitiating a cloud interface connection channel.
/// This step calls the start method on the Distributor Channel and waits for a successful Distributor channel
/// connection before proceeding to the next boot step.
pub struct StartDistributorChannel;

#[async_trait]
impl Bootstep<BootstrapState> for StartDistributorChannel {
    fn get_name(&self) -> String {
        "StartDistributorChannel".into()
    }
    async fn setup(&self, mut state: BootstrapState) -> Result<(), RippleError> {
        let extn_capability = state
            .platform_state
            .get_distributor_capability()
            .expect("distributor capability");
        let client = state.platform_state.get_client();
        if let Err(e) = state.extn_state.start(
            extn_capability.clone(),
            ChannelsState::get_crossbeam_channel(),
            client,
        ) {
            error!("Error during distributor channel bootstrap");
            return Err(e);
        }
        let (tx, mut tr) = channel::<ExtnStatus>(1);
        state.platform_state.get_client().add_event_processor(
            WaitForStatusReadyEventProcessor::new(extn_capability.clone(), tx),
        );
        if let Some(_) = tr.recv().await {
            debug!("Distributor connection successful");
            state
                .platform_state
                .get_client()
                .cleanup_event_processor(extn_capability);
            return Ok(());
        }
        Err(RippleError::BootstrapError)
    }
}
