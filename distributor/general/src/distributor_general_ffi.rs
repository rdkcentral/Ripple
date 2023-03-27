use ripple_sdk::{
    api::status_update::ExtnStatus,
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
    framework::ripple_contract::{DistributorContract, RippleContract},
    log::{debug, info},
    semver::Version,
    tokio::runtime::Runtime,
    utils::{error::RippleError, logger::init_logger},
};

use crate::{
    general_permission_processor::DistributorPermissionProcessor,
    general_session_processor::DistributorSessionProcessor,
};

fn init_library() -> CExtnMetadata {
    let _ = init_logger("launcher".into());

    let dist_meta = ExtnSymbolMetadata::get(
        ExtnId::new_channel(ExtnClassId::Distributor, "general".into()),
        RippleContract::Distributor(DistributorContract::Permissions),
        Version::new(1, 1, 0),
    );

    debug!("Returning launcher builder");
    let extn_metadata = ExtnMetadata {
        name: "distributor_general".into(),
        symbols: vec![dist_meta],
    };
    extn_metadata.into()
}

export_extn_metadata!(CExtnMetadata, init_library);

fn start_launcher(sender: ExtnSender, receiver: CReceiver<CExtnMessage>) {
    let _ = init_logger("launcher_channel".into());
    info!("Starting launcher channel");
    let runtime = Runtime::new().unwrap();
    let mut client = ExtnClient::new(receiver.clone(), sender);
    runtime.block_on(async move {
        client.add_request_processor(DistributorSessionProcessor::new(client.clone()));
        client.add_request_processor(DistributorPermissionProcessor::new(client.clone()));
        // Lets Main know that the distributor channel is ready
        let _ = client.event(ExtnStatus::Ready).await;
        client.initialize().await;
    });
}

fn build(extn_id: String) -> Result<Box<ExtnChannel>, RippleError> {
    if let Ok(id) = ExtnId::try_from(extn_id.clone()) {
        let current_id = ExtnId::new_channel(ExtnClassId::Launcher, "internal".into());

        if id.eq(&current_id) {
            return Ok(Box::new(ExtnChannel {
                start: start_launcher,
            }));
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
        service: "launcher".into(),
    }
}

export_channel_builder!(ExtnChannelBuilder, init_extn_builder);
