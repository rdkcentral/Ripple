use jsonrpsee::core::server::rpc_module::Methods;
use ripple_sdk::{
    crossbeam::channel::Receiver,
    export_extn_metadata, export_jsonrpc_extn_builder,
    extn::{
        client::{extn_client::ExtnClient, extn_sender::ExtnSender},
        extn_capability::ExtnCapability,
        ffi::{
            ffi_library::{CExtnMetadata, ExtnMetaEntry, ExtnMetadata, JsonRpseeExtnBuilder},
            ffi_message::CExtnMessage,
        },
    },
    log::debug,
    semver::Version,
    utils::logger::init_logger,
};

use crate::rpc::{
    hdcp_jsonrpsee_extn::{HdcpImpl, HdcpServer},
    legacy_jsonrpsee_extn::{LegacyImpl, LegacyServer},
};

fn init_library() -> CExtnMetadata {
    let _ = init_logger("rpc_extn".into());

    let json_rpsee_extn_meta = ExtnMetaEntry::get(
        ExtnCapability::new_extn(
            ripple_sdk::extn::extn_capability::ExtnClass::Jsonrpsee,
            "hdcp".into(),
        ),
        Version::new(1, 1, 0),
    );

    debug!("Returning extended thunder library entries");
    let extn_metadata = ExtnMetadata {
        name: "rpc_example".into(),
        metadata: vec![json_rpsee_extn_meta],
    };
    extn_metadata.into()
}

export_extn_metadata!(CExtnMetadata, init_library);

fn get_rpc_extns(sender: ExtnSender, receiver: Receiver<CExtnMessage>) -> Methods {
    let mut methods = Methods::new();
    let client = ExtnClient::new(receiver.clone(), sender);
    let _ = methods.merge(HdcpImpl::new(client.clone()).into_rpc());
    let _ = methods.merge(LegacyImpl::new(client).into_rpc());
    methods
}

fn init_thunder_builder() -> JsonRpseeExtnBuilder {
    JsonRpseeExtnBuilder {
        build: get_rpc_extns,
        service: "hdcp".into(),
    }
}

export_jsonrpc_extn_builder!(JsonRpseeExtnBuilder, init_thunder_builder);
