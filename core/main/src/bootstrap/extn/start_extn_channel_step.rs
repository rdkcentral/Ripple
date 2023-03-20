use ripple_sdk::{
    api::status_update::ExtnStatus,
    async_trait::async_trait,
    framework::{bootstrap::Bootstep, RippleResponse},
    log::error,
    tokio::sync::mpsc,
    utils::error::RippleError,
};

use crate::state::{bootstrap_state::BootstrapState, extn_state::PreLoadedExtnChannel};

fn start_preloaded_channel(
    state: &BootstrapState,
    channel: PreLoadedExtnChannel,
) -> RippleResponse {
    let client = state.platform_state.get_client();

    if let Err(e) = state
        .clone()
        .extn_state
        .start_channel(channel, client.clone())
    {
        error!("Error during Device channel bootstrap");
        return Err(e);
    }

    Ok(())
}

/// Bootstep which starts the All Extns channels intitiating including the device interface connection channel.
/// This step calls the start method on the all the Channels and waits for a successful 
/// [ExtnStatus] before proceeding to the next boot step.
pub struct StartExtnChannelsStep;

#[async_trait]
impl Bootstep<BootstrapState> for StartExtnChannelsStep {
    fn get_name(&self) -> String {
        "StartExtnChannelsStep".into()
    }
    async fn setup(&self, state: BootstrapState) -> Result<(), RippleError> {
        let mut extn_ids = Vec::new();
        {
            let mut device_channels = state.extn_state.device_channels.write().unwrap();
            while let Some(device_channel) = device_channels.pop() {
                let id = device_channel.extn_id.clone();
                extn_ids.push(id.clone());
                if let Err(e) = start_preloaded_channel(&state, device_channel) {
                    error!("Error during Device channel bootstrap");
                    return Err(e);
                }
            }
        }

        {
            let mut deferred_channels = state.extn_state.deferred_channels.write().unwrap();
            while let Some(deferred_channel) = deferred_channels.pop() {
                let id = deferred_channel.extn_id.clone();
                extn_ids.push(id.clone());
                if let Err(e) = start_preloaded_channel(&state, deferred_channel) {
                    error!("Error during channel bootstrap");
                    return Err(e);
                }
            }
        }

        for extn_id in extn_ids {
            let (tx, mut tr) = mpsc::channel(1);
            if !state
                .extn_state
                .add_extn_status_listener(extn_id.clone(), tx)
            {
                if let Some(v) = tr.recv().await {
                    state.extn_state.clear_status_listener(extn_id);
                    match v {
                        ExtnStatus::Ready => continue,
                        _ => return Err(RippleError::BootstrapError),
                    }
                }
            }
        }

        Ok(())
    }
}
