use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use ripple_sdk::{
    api::{
        manifest::extn_manifest::{ExtnManifest, ExtnResolutionEntry},
        status_update::ExtnStatus,
    },
    crossbeam::channel::{Receiver as CReceiver, Sender as CSender},
    extn::{
        client::extn_sender::ExtnSender,
        extn_capability::ExtnCapability,
        ffi::{
            ffi_device::{DeviceChannel, DeviceExtn},
            ffi_library::ExtnMetadata,
            ffi_message::CExtnMessage,
        },
    },
    libloading::Library,
    tokio,
    utils::error::RippleError,
};

use crate::service::extn::ripple_client::RippleClient;

use super::bootstrap_state::ChannelsState;

#[derive(Debug)]
pub struct LoadedLibrary {
    pub library: Library,
    metadata: Box<ExtnMetadata>,
    pub resolution: Option<Vec<ExtnResolutionEntry>>,
}

impl LoadedLibrary {
    pub fn new(
        library: Library,
        metadata: Box<ExtnMetadata>,
        resolution: Option<Vec<ExtnResolutionEntry>>,
    ) -> LoadedLibrary {
        LoadedLibrary {
            library,
            metadata,
            resolution,
        }
    }
    pub fn get_symbols(&self) {}

    pub fn get_metadata(&self) -> Box<ExtnMetadata> {
        self.metadata.clone()
    }
}

/// Bootstrap state which is used to store transient extension information used while bootstrapping.
/// Content within state is related to extension symbols and Libraries.
#[derive(Debug, Clone)]
pub struct ExtnState {
    sender: CSender<CExtnMessage>,
    extn_manifest: ExtnManifest,
    pub loaded_libraries: Arc<RwLock<Vec<LoadedLibrary>>>,
    pub device_channel: Arc<RwLock<Option<Box<DeviceChannel>>>>,
    pub extn_sender_map:
        Arc<RwLock<HashMap<String, (CSender<CExtnMessage>, CReceiver<CExtnMessage>)>>>,
    pub extn_state_map: Arc<RwLock<HashMap<String, ExtnStatus>>>,
    pub device_extns: Arc<RwLock<Option<Vec<DeviceExtn>>>>,
}

impl ExtnState {
    pub fn new(channels_state: ChannelsState, extn_manifest: ExtnManifest) -> ExtnState {
        ExtnState {
            sender: channels_state.get_extn_sender(),
            extn_manifest,
            loaded_libraries: Arc::new(RwLock::new(Vec::new())),
            device_channel: Arc::new(RwLock::new(None)),
            extn_sender_map: Arc::new(RwLock::new(HashMap::new())),
            extn_state_map: Arc::new(RwLock::new(HashMap::new())),
            device_extns: Arc::new(RwLock::new(None)),
        }
    }

    pub fn get_manifest(&self) -> ExtnManifest {
        self.extn_manifest.clone()
    }

    pub fn get_sender(self) -> CSender<CExtnMessage> {
        self.sender.clone()
    }

    pub fn add_extn_channels(
        &self,
        capability: ExtnCapability,
        channel: (CSender<CExtnMessage>, CReceiver<CExtnMessage>),
    ) {
        let mut client_state = self.extn_sender_map.write().unwrap();
        client_state.insert(capability.to_string(), channel);
    }

    pub fn start(
        &mut self,
        capability: ExtnCapability,
        channel: (CSender<CExtnMessage>, CReceiver<CExtnMessage>),
        client: RippleClient,
    ) -> Result<(), RippleError> {
        let sender = self.clone().get_sender();
        let extn_sender = ExtnSender::new(sender, capability.clone());
        let (extn_tx, extn_rx) = channel;

        if capability.is_device_channel() {
            let channel = {
                let mut channel = self.device_channel.write().unwrap();
                channel.take().unwrap()
            };
            let extns = {
                let mut extns = self.device_extns.write().unwrap();
                extns.take().unwrap()
            };
            tokio::spawn(async move {
                (channel.start)(extn_sender, extn_rx, extns);
            });
            client.add_extn_sender(capability, extn_tx);
            return Ok(());
        }

        Err(RippleError::BootstrapError)
    }
}
