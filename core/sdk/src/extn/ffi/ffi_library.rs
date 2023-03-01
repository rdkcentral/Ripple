use std::str::FromStr;

use crate::{extn::extn_capability::ExtnCapability, utils::error::RippleError};
use libloading::{Library, Symbol};
use log::{debug, error, info};
use semver::Version;
use serde::{Deserialize, Serialize};

#[repr(C)]
#[derive(Debug, Clone)]
pub struct ExtnMetadata {
    pub name: String,
    pub metadata: Vec<ExtnMetaEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CExtnMetaEntry {
    cap: String,
    required_version: String,
}

#[derive(Debug, Clone)]
pub struct ExtnMetaEntry {
    cap: ExtnCapability,
    required_version: Version,
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct CExtnMetadata {
    pub name: String,
    pub metadata: String,
}

impl TryInto<ExtnMetadata> for Box<CExtnMetadata> {
    type Error = RippleError;
    fn try_into(self) -> Result<ExtnMetadata, Self::Error> {
        if let Ok(r) = serde_json::from_str(&self.metadata.clone()) {
            let cap_entries: Vec<CExtnMetaEntry> = r;
            let mut metadata: Vec<ExtnMetaEntry> = Vec::new();
            for c_entry in cap_entries {
                if let Ok(cap) = ExtnCapability::try_from(c_entry.cap) {
                    if let Ok(required_version) = Version::from_str(&c_entry.required_version) {
                        metadata.push(ExtnMetaEntry {
                            cap,
                            required_version,
                        })
                    }
                }
            }
            return Ok(ExtnMetadata {
                name: self.name,
                metadata,
            });
        }
        Err(RippleError::ExtnError)
    }
}

impl From<ExtnMetadata> for CExtnMetadata {
    fn from(value: ExtnMetadata) -> Self {
        let mut metadata: Vec<CExtnMetaEntry> = Vec::new();
        for data in value.metadata {
            metadata.push(CExtnMetaEntry {
                cap: data.cap.to_string(),
                required_version: data.get_version().to_string(),
            });
        }
        let symbols = serde_json::to_string(&metadata).unwrap();
        info!("exported symbols in library {}", symbols.clone());
        CExtnMetadata {
            name: value.name.clone(),
            metadata: symbols,
        }
    }
}

impl ExtnMetaEntry {
    pub fn get(cap: ExtnCapability, required_version: Version) -> ExtnMetaEntry {
        ExtnMetaEntry {
            cap,
            required_version,
        }
    }

    pub fn get_cap(&self) -> ExtnCapability {
        self.cap.clone()
    }

    pub fn get_version(&self) -> Version {
        self.required_version.clone()
    }
}

/// Macro to assist extensions define their metadata. Each Extension library will contain one metadata symbol
/// which catalogues the underlying extensions within the libarary.
///
/// # Example
/// ```
/// use ripple_sdk::export_extn_metadata;
/// use ripple_sdk::extn::ffi::ffi_library::CExtnMetadata;
/// use ripple_sdk::utils::logger::init_logger;
/// use ripple_sdk::extn::ffi::ffi_library::ExtnMetaEntry;
/// use ripple_sdk::extn::extn_capability::{ExtnClass,ExtnCapability};
/// use semver::Version;
/// use ripple_sdk::extn::ffi::ffi_library::ExtnMetadata;
/// fn init_library() -> CExtnMetadata {
/// let _ = init_logger("device_channel".into());
/// let thunder_channel_meta = ExtnMetaEntry::get(
///     ExtnCapability::new_channel(ExtnClass::Device, "device_interface".into()),
///     Version::new(1, 1, 0),
/// );

/// let extn_metadata = ExtnMetadata {
///     name: "device_interface".into(),
///     metadata: vec![thunder_channel_meta],
/// };
/// extn_metadata.into()
/// }
/// export_extn_metadata!(CExtnMetadata, init_library);
/// ```

#[macro_export]
macro_rules! export_extn_metadata {
    ($plugin_type:ty, $constructor:path) => {
        #[no_mangle]
        pub extern "C" fn extn_library_create_metadata() -> *mut CExtnMetadata {
            let constructor: fn() -> $plugin_type = $constructor;
            let object = constructor();
            let boxed = Box::new(object);
            Box::into_raw(boxed)
        }
    };
}

/// Used by Ripple main to load the metadata for a given dynamic library.
pub unsafe fn load_extn_library_metadata(lib: &Library) -> Option<Box<ExtnMetadata>> {
    type LibraryFfi = unsafe fn() -> *mut CExtnMetadata;
    let r = lib.get(b"extn_library_create_metadata");
    match r {
        Ok(r) => {
            debug!("Symbol extracted from library");
            let constructor: Symbol<LibraryFfi> = r;
            let r = Box::from_raw(constructor());
            let metadata: Result<ExtnMetadata, RippleError> = r.try_into();
            if metadata.is_ok() {
                return Some(Box::new(metadata.unwrap()));
            }
            //return Some();
        }
        Err(e) => error!("Extn library symbol loading failed {:?}", e),
    }
    None
}
