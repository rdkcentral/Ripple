use ripple_sdk::{
    crossbeam::channel::Receiver,
    export_extn_channel, export_extn_metadata,
    extn::{
        client::{extn_client::ExtnClient, extn_sender::ExtnSender},
        extn_capability::{ExtnCapability, ExtnClass},
        ffi::{
            ffi_channel::ExtnChannel,
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
    launcher_lifecycle_processor::LauncherLifecycleEventProcessor, launcher_state::LauncherState,
};

fn init_library() -> CExtnMetadata {
    let _ = init_logger("launcher".into());

    let launcher_meta = ExtnMetaEntry::get(
        ExtnCapability::new_channel(ExtnClass::Launcher, "thunder".into()),
        Version::new(1, 1, 0),
    );

    debug!("Returning launcher builder");
    let extn_metadata = ExtnMetadata {
        name: "launcher".into(),
        metadata: vec![launcher_meta],
    };
    extn_metadata.into()
}

export_extn_metadata!(CExtnMetadata, init_library);

fn start_launcher(sender: ExtnSender, receiver: Receiver<CExtnMessage>) {
    let _ = init_logger("launcher_channel".into());
    info!("Starting launcher channel");
    let runtime = Runtime::new().unwrap();
    let client = ExtnClient::new(receiver.clone(), sender);
    runtime.block_on(async move {
        let client_for_receiver = client.clone();
        let state = LauncherState::new(client.clone())
            .await
            .expect("state initialization to succeed");
        let mut client_for_processor = client.clone();
        client_for_processor.add_event_processor(LauncherLifecycleEventProcessor::new(state));
        client_for_receiver.initialize().await;
    });
}

fn init_channel() -> ExtnChannel {
    ExtnChannel {
        start: start_launcher,
    }
}

export_extn_channel!(ExtnChannel, init_channel);
