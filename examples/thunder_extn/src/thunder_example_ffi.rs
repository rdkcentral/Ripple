use ripple_sdk::{
    export_extn_metadata,
    extn::{
        extn_capability::ExtnCapability,
        ffi::ffi_library::{CExtnMetadata, ExtnMetaEntry, ExtnMetadata},
    },
    log::debug,
    semver::Version,
    utils::logger::init_logger,
};

fn init_library() -> CExtnMetadata {
    let _ = init_logger("device_extn".into());
    let thunder_extn_meta = ExtnMetaEntry::get(
        ExtnCapability::new_extn(
            ripple_sdk::extn::extn_capability::ExtnClass::Device,
            "other".into(),
        ),
        Version::new(1, 1, 0),
    );

    debug!("Returning extended thunder library entries");
    let extn_metadata = ExtnMetadata {
        name: "thunder_example".into(),
        metadata: vec![thunder_extn_meta],
    };
    extn_metadata.into()
}

export_extn_metadata!(CExtnMetadata, init_library);
