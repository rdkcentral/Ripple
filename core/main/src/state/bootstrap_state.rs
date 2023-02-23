use ripple_sdk::{
    framework::bootstrap::TransientChannel,
    tokio::sync::mpsc::{self, Receiver, Sender},
    utils::error::RippleError,
};

use crate::{
    bootstrap::manifest::device::LoadDeviceManifestStep,
    firebolt::firebolt_gateway::FireboltGatewayCommand, service::extn::ripple_client::RippleClient,
};

use super::platform_state::PlatformState;

#[derive(Debug, Clone)]
pub struct ChannelsState {
    gateway_channel: TransientChannel<FireboltGatewayCommand>,
}

impl ChannelsState {
    pub fn new() -> ChannelsState {
        let (gateway_tx, gateway_tr) = mpsc::channel(32);
        ChannelsState {
            gateway_channel: TransientChannel::new(gateway_tx, gateway_tr),
        }
    }
    pub fn get_gateway_sender(&self) -> Sender<FireboltGatewayCommand> {
        self.gateway_channel.get_sender()
    }

    pub fn get_gateway_receiver(&self) -> Result<Receiver<FireboltGatewayCommand>, RippleError> {
        self.gateway_channel.get_receiver()
    }
}

#[derive(Debug, Clone)]
pub struct BootstrapState {
    pub platform_state: PlatformState,
    pub channels_state: ChannelsState,
}

impl BootstrapState {
    pub fn build() -> Result<BootstrapState, RippleError> {
        let channels_state = ChannelsState::new();
        let client = RippleClient::new(channels_state.clone());
        let platform_state = PlatformState::new(LoadDeviceManifestStep::get_manifest(), client);
        Ok(BootstrapState {
            platform_state,
            channels_state,
        })
    }
}
