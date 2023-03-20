use thunder_ripple_sdk::ripple_sdk::{
    crossbeam::channel::Receiver as CReceiver,
    export_channel_builder, export_extn_metadata,
    extn::{
        client::{extn_client::ExtnClient, extn_sender::ExtnSender},
        extn_id::{ExtnClassId, ExtnId},
        ffi::{
            ffi_channel::{ExtnChannel, ExtnChannelBuilder},
            ffi_library::{CExtnMetadata, ExtnMetadata, ExtnSymbolMetadata},
            ffi_message::CExtnMessage,
        },
    },
    framework::ripple_contract::{DeviceContract, RippleContract},
    log::{debug, info},
    semver::Version,
    tokio::{self, runtime::Runtime},
    utils::{error::RippleError, logger::init_logger},
};

use crate::bootstrap::boot_thunder_channel::boot_thunder_channel;

fn init_library() -> CExtnMetadata {
    let _ = init_logger("device_channel".into());
    let thunder_channel_meta = ExtnSymbolMetadata::get(
        ExtnId::new_channel(ExtnClassId::Device, "thunder".into()),
        RippleContract::Device(DeviceContract::Info),
        Version::new(1, 1, 0),
    );

    debug!("Returning thunder library entries");
    let extn_metadata = ExtnMetadata {
        name: "thunder".into(),
        symbols: vec![thunder_channel_meta],
    };
    extn_metadata.into()
}
export_extn_metadata!(CExtnMetadata, init_library);

pub fn start(sender: ExtnSender, receiver: CReceiver<CExtnMessage>) {
    let _ = init_logger("device_channel".into());
    info!("Starting device channel");
    let runtime = Runtime::new().unwrap();
    let client = ExtnClient::new(receiver.clone(), sender);
    runtime.block_on(async move {
        let client_for_receiver = client.clone();
        let client_for_thunder = client.clone();
        tokio::spawn(async move { boot_thunder_channel(client_for_thunder).await });
        client_for_receiver.initialize().await;
    });
}

fn build(extn_id: String) -> Result<Box<ExtnChannel>, RippleError> {
    if let Ok(id) = ExtnId::try_from(extn_id.clone()) {
        let current_id = ExtnId::new_channel(ExtnClassId::Device, "thunder".into());

        if id.eq(&current_id) {
            return Ok(Box::new(ExtnChannel { start }));
        } else {
            Err(RippleError::ExtnError)
        }
    } else {
        Err(RippleError::InvalidInput)
    }
}

fn init_extn_builder() -> ExtnChannelBuilder {
    ExtnChannelBuilder {
        build,
        service: "thunder".into(),
    }
}

export_channel_builder!(ExtnChannelBuilder, init_extn_builder);
