use ripple_sdk::{
    crossbeam::channel::{unbounded, Receiver as CReceiver, Sender as CSender},
    extn::ffi::ffi_message::CExtnMessage,
    framework::bootstrap::TransientChannel,
    tokio::sync::mpsc::{self, Receiver, Sender},
    utils::error::RippleError,
};

use crate::{
    bootstrap::manifest::{device::LoadDeviceManifestStep, extn::LoadExtnManifestStep},
    firebolt::firebolt_gateway::FireboltGatewayCommand,
    service::extn::ripple_client::RippleClient,
};

use super::{extn_state::ExtnState, platform_state::PlatformState};

#[derive(Debug, Clone)]
pub struct ChannelsState {
    gateway_channel: TransientChannel<FireboltGatewayCommand>,
    extn_sender: CSender<CExtnMessage>,
    extn_receiver: CReceiver<CExtnMessage>,
}

impl ChannelsState {
    pub fn new() -> ChannelsState {
        let (gateway_tx, gateway_tr) = mpsc::channel(32);
        let (ctx, ctr) = unbounded();
        ChannelsState {
            gateway_channel: TransientChannel::new(gateway_tx, gateway_tr),
            extn_sender: ctx,
            extn_receiver: ctr,
        }
    }
    pub fn get_gateway_sender(&self) -> Sender<FireboltGatewayCommand> {
        self.gateway_channel.get_sender()
    }

    pub fn get_gateway_receiver(&self) -> Result<Receiver<FireboltGatewayCommand>, RippleError> {
        self.gateway_channel.get_receiver()
    }

    pub fn get_extn_sender(&self) -> CSender<CExtnMessage> {
        self.extn_sender.clone()
    }

    pub fn get_extn_receiver(&self) -> CReceiver<CExtnMessage> {
        self.extn_receiver.clone()
    }

    pub fn get_crossbeam_channel() -> (CSender<CExtnMessage>, CReceiver<CExtnMessage>) {
        unbounded()
    }
}

#[derive(Debug, Clone)]
pub struct BootstrapState {
    pub platform_state: PlatformState,
    pub channels_state: ChannelsState,
    pub extn_state: ExtnState,
}

impl BootstrapState {
    pub fn build() -> Result<BootstrapState, RippleError> {
        let channels_state = ChannelsState::new();
        let client = RippleClient::new(channels_state.clone());
        let platform_state = PlatformState::new(LoadDeviceManifestStep::get_manifest(), client);
        let extn_state =
            ExtnState::new(channels_state.clone(), LoadExtnManifestStep::get_manifest());
        Ok(BootstrapState {
            platform_state,
            channels_state,
            extn_state,
        })
    }
}
