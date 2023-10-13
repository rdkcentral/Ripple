// Copyright 2023 Comcast Cable Communications Management, LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::{
    extn::extn_id::ExtnId, framework::ripple_contract::ContractFulfiller, utils::error::RippleError,
};
use libloading::{Library, Symbol};
use log::{debug, error, info};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[repr(C)]
#[derive(Debug, Clone)]
pub struct ExtnMetadata {
    pub name: String,
    pub symbols: Vec<ExtnSymbolMetadata>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CExtnSymbolMetadata {
    fulfills: String,
    id: String,
    required_version: String,
}

#[derive(Debug, Clone)]
pub struct ExtnSymbolMetadata {
    pub id: ExtnId,
    pub fulfills: ContractFulfiller,
    pub required_version: Version,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq)]
pub struct CExtnMetadata {
    pub name: String,
    pub metadata: String,
}

impl TryInto<ExtnMetadata> for Box<CExtnMetadata> {
    type Error = RippleError;
    fn try_into(self) -> Result<ExtnMetadata, Self::Error> {
        if let Ok(r) = serde_json::from_str(&self.metadata.clone()) {
            let cap_entries: Vec<CExtnSymbolMetadata> = r;
            let mut metadata: Vec<ExtnSymbolMetadata> = Vec::new();
            for c_entry in cap_entries {
                if let Ok(id) = ExtnId::try_from(c_entry.id) {
                    if let Ok(fulfills) = ContractFulfiller::try_from(c_entry.fulfills) {
                        if let Ok(required_version) = Version::from_str(&c_entry.required_version) {
                            metadata.push(ExtnSymbolMetadata {
                                id,
                                fulfills,
                                required_version,
                            })
                        }
                    }
                }
            }
            return Ok(ExtnMetadata {
                name: self.name,
                symbols: metadata,
            });
        }

        Err(RippleError::ExtnError)
    }
}

impl From<ExtnMetadata> for CExtnMetadata {
    fn from(value: ExtnMetadata) -> Self {
        let mut metadata: Vec<CExtnSymbolMetadata> = Vec::new();
        for data in value.symbols {
            metadata.push(CExtnSymbolMetadata {
                id: data.clone().id.to_string(),
                fulfills: data.clone().fulfills.into(),
                required_version: data.get_version().to_string(),
            });
        }
        let symbols = serde_json::to_string(&metadata).unwrap();
        info!("exported symbols in library {}", symbols);
        CExtnMetadata {
            name: value.name,
            metadata: symbols,
        }
    }
}

impl ExtnSymbolMetadata {
    pub fn get(
        id: ExtnId,
        fulfills: ContractFulfiller,
        required_version: Version,
    ) -> ExtnSymbolMetadata {
        ExtnSymbolMetadata {
            id,
            fulfills,
            required_version,
        }
    }

    pub fn get_contract(&self) -> ContractFulfiller {
        self.fulfills.clone()
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
/// use ripple_sdk::extn::ffi::ffi_library::ExtnSymbolMetadata;
/// use ripple_sdk::extn::extn_id::{ExtnClassId,ExtnId};
/// use ripple_sdk::framework::ripple_contract::{RippleContract, ContractFulfiller};
/// use semver::Version;
/// use ripple_sdk::extn::ffi::ffi_library::ExtnMetadata;
/// fn init_library() -> CExtnMetadata {
/// let _ = init_logger("device_channel".into());
/// let thunder_channel_meta = ExtnSymbolMetadata::get(
///     ExtnId::new_channel(ExtnClassId::Device, "device_interface".into()),
///     ContractFulfiller::new(vec![RippleContract::DeviceInfo]),
///     Version::new(1, 1, 0),
/// );

/// let extn_metadata = ExtnMetadata {
///     name: "device_interface".into(),
///     symbols: vec![thunder_channel_meta],
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
/// # Safety
/// TODO: Why is this unsafe allowed here? https://rust-lang.github.io/rust-clippy/master/index.html#missing_safety_doc
pub unsafe fn load_extn_library_metadata(lib: &Library) -> Option<Box<ExtnMetadata>> {
    type LibraryFfi = unsafe fn() -> *mut CExtnMetadata;
    let r = lib.get(b"extn_library_create_metadata");
    match r {
        Ok(r) => {
            debug!("Symbol extracted from library");
            let constructor: Symbol<LibraryFfi> = r;
            let r = Box::from_raw(constructor());
            let metadata: Result<ExtnMetadata, RippleError> = r.try_into();
            if let Ok(metadata) = metadata {
                return Some(Box::new(metadata));
            }
            //return Some();
        }
        Err(e) => error!("Extn library symbol loading failed {:?}", e),
    }
    None
}
