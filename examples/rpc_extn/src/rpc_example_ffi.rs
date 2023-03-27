use jsonrpsee::core::server::rpc_module::Methods;
use ripple_sdk::{
    crossbeam::channel::Receiver,
    export_extn_metadata, export_jsonrpc_extn_builder,
    extn::{
        client::{extn_client::ExtnClient, extn_sender::ExtnSender},
        extn_id::{ExtnClassId, ExtnId},
        ffi::{
            ffi_jsonrpsee::JsonRpseeExtnBuilder,
            ffi_library::{CExtnMetadata, ExtnMetadata, ExtnSymbolMetadata},
            ffi_message::CExtnMessage,
        },
    },
    framework::ripple_contract::RippleContract,
    log::debug,
    semver::Version,
    utils::logger::init_logger,
};

use crate::rpc::{
    custom_jsonrpsee_extn::{CustomImpl, CustomServer},
    legacy_jsonrpsee_extn::{LegacyImpl, LegacyServer},
};

fn init_library() -> CExtnMetadata {
    let _ = init_logger("rpc_extn".into());

    let json_rpsee_extn_meta = ExtnSymbolMetadata::get(
        ExtnId::new_extn(ExtnClassId::Jsonrpsee, "custom".into()),
        RippleContract::JsonRpsee,
        Version::new(1, 1, 0),
    );

    debug!("Returning extended custom library entries");
    let extn_metadata = ExtnMetadata {
        name: "custom".into(),
        symbols: vec![json_rpsee_extn_meta],
    };
    extn_metadata.into()
}

export_extn_metadata!(CExtnMetadata, init_library);

fn get_rpc_extns(sender: ExtnSender, receiver: Receiver<CExtnMessage>) -> Methods {
    let mut methods = Methods::new();
    let client = ExtnClient::new(receiver.clone(), sender);
    let _ = methods.merge(CustomImpl::new(client.clone()).into_rpc());
    let _ = methods.merge(LegacyImpl::new(client).into_rpc());
    methods
}

fn init_jsonrpsee_builder() -> JsonRpseeExtnBuilder {
    JsonRpseeExtnBuilder {
        build: get_rpc_extns,
        service: "custom".into(),
    }
}

export_jsonrpc_extn_builder!(JsonRpseeExtnBuilder, init_jsonrpsee_builder);
