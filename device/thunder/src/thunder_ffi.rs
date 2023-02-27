use ripple_sdk::{
    crossbeam::channel::Receiver as CReceiver,
    export_device_channel, export_extn_metadata,
    extn::{
        client::{extn_client::ExtnClient, extn_sender::ExtnSender},
        extn_capability::{ExtnCapability, ExtnClass},
        ffi::{
            ffi_device::{DeviceChannel, DeviceExtn},
            ffi_library::{CExtnMetadata, ExtnMetaEntry, ExtnMetadata},
            ffi_message::CExtnMessage,
        },
    },
    log::{debug, info},
    semver::Version,
    tokio::{self, runtime::Runtime},
    utils::logger::init_logger,
};

use crate::{bootstrap::boot_thunder::boot, thunder_state::ThunderBootstrapState};

fn init_library() -> CExtnMetadata {
    let _ = init_logger("device_channel".into());
    let thunder_channel_meta = ExtnMetaEntry::get(
        ExtnCapability::new_channel(ExtnClass::Device, "thunder".into()),
        Version::new(1, 1, 0),
    );

    debug!("Returning thunder library entries");
    let extn_metadata = ExtnMetadata {
        name: "thunder".into(),
        metadata: vec![thunder_channel_meta],
    };
    extn_metadata.into()
}
export_extn_metadata!(CExtnMetadata, init_library);

pub fn start(sender: ExtnSender, receiver: CReceiver<CExtnMessage>, extns: Vec<DeviceExtn>) {
    let _ = init_logger("device_channel".into());
    info!("Starting device channel");
    let runtime = Runtime::new().unwrap();
    let client = ExtnClient::new(receiver.clone(), sender);
    runtime.block_on(async move {
        let client_for_receiver = client.clone();
        let client_for_thunder = client.clone();
        let state = ThunderBootstrapState::new(client_for_thunder, extns);
        tokio::spawn(async move { boot(state).await });
        client_for_receiver.initialize().await;
    });
}

fn init_device_channel() -> DeviceChannel {
    DeviceChannel {
        start,
        capability: ExtnCapability::new_channel(ExtnClass::Device, "thunder".into()),
        version: Version::new(1, 1, 0),
    }
}

export_device_channel!(DeviceChannel, init_device_channel);
