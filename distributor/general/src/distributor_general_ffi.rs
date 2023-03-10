use ripple_sdk::{
    crossbeam::channel::Receiver as CReceiver,
    export_distributor_channel, export_extn_metadata,
    extn::{
        client::{extn_client::ExtnClient, extn_sender::ExtnSender},
        extn_capability::{ExtnCapability, ExtnClass},
        ffi::{
            ffi_distributor::{DistributorChannel, DistributorExtn},
            ffi_library::{CExtnMetadata, ExtnMetaEntry, ExtnMetadata},
            ffi_message::CExtnMessage,
        },
    },
    log::{debug, info},
    semver::Version,
    tokio::runtime::Runtime,
    utils::logger::init_logger,
};

use crate::{
    general_permission_processor::DistributorPermissionProcessor,
    general_session_processor::DistributorSessionProcessor,
};

fn init_library() -> CExtnMetadata {
    let _ = init_logger("dist_channel".into());
    let dist_channel_meta = ExtnMetaEntry::get(
        ExtnCapability::new_channel(ExtnClass::Distributor, "general".into()),
        Version::new(1, 1, 0),
    );

    debug!("Returning thunder library entries");
    let extn_metadata = ExtnMetadata {
        name: "general".into(),
        metadata: vec![dist_channel_meta],
    };
    extn_metadata.into()
}
export_extn_metadata!(CExtnMetadata, init_library);

pub fn start(sender: ExtnSender, receiver: CReceiver<CExtnMessage>, _: Vec<DistributorExtn>) {
    let _ = init_logger("dist_channel".into());
    info!("Starting distributor channel");
    let runtime = Runtime::new().unwrap();
    let mut client = ExtnClient::new(receiver.clone(), sender);
    runtime.block_on(async move {
        client.add_request_processor(DistributorSessionProcessor::new(client.clone()));
        client.add_request_processor(DistributorPermissionProcessor::new(client.clone()));
        client.initialize().await;
    });
}

fn init_dist_channel() -> DistributorChannel {
    DistributorChannel {
        start,
        capability: ExtnCapability::new_channel(ExtnClass::Distributor, "general".into()),
        version: Version::new(1, 1, 0),
    }
}

export_distributor_channel!(DistributorChannel, init_dist_channel);
