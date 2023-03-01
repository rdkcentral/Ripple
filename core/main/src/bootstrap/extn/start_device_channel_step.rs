use ripple_sdk::{
    api::status_update::ExtnStatus,
    async_trait::async_trait,
    extn::extn_capability::{ExtnCapability, ExtnClass},
    framework::bootstrap::Bootstep,
    log::{debug, error},
    tokio::sync::mpsc::channel,
    utils::error::RippleError,
};

use crate::{
    processor::wait_for_service_processor::WaitForStatusReadyEventProcessor,
    state::bootstrap_state::{BootstrapState, ChannelsState},
};

/// Bootstep which starts the Device channel intitiating a device interface connection channel.
/// This step calls the start method on the Device Channel and waits for a successful Device
/// connection before proceeding to the next boot step.
pub struct StartDeviceChannel;

#[async_trait]
impl Bootstep<BootstrapState> for StartDeviceChannel {
    fn get_name(&self) -> String {
        "StartDeviceChannel".into()
    }
    async fn setup(&self, mut state: BootstrapState) -> Result<(), RippleError> {
        let platform = state
            .platform_state
            .get_device_manifest()
            .get_device_platform();
        let extn_capability = ExtnCapability::new_channel(ExtnClass::Device, platform.to_string());
        let client = state.platform_state.get_client();
        if let Err(e) = state.extn_state.start(
            extn_capability.clone(),
            ChannelsState::get_crossbeam_channel(),
            client,
        ) {
            error!("Error during Device channel bootstrap");
            return Err(e);
        }
        let (tx, mut tr) = channel::<ExtnStatus>(1);
        state.platform_state.get_client().add_event_processor(
            WaitForStatusReadyEventProcessor::new(extn_capability.clone(), tx),
        );
        if let Some(_) = tr.recv().await {
            debug!("Device connection successful");
            state
                .platform_state
                .get_client()
                .cleanup_event_processor(extn_capability);
            return Ok(());
        }
        Err(RippleError::BootstrapError)
    }
}
