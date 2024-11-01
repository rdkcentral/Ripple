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
                id: data.id.to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        extn::extn_id::{ExtnClassId, ExtnType},
        framework::ripple_contract::RippleContract,
    };
    use semver::Version;

    // Mock implementation for ExtnMetadata
    fn mock_extn_metadata() -> ExtnMetadata {
        let launcher_channel_meta = ExtnSymbolMetadata::get(
            ExtnId::new_channel(ExtnClassId::Launcher, "internal".into()),
            ContractFulfiller::new(vec![RippleContract::Launcher]),
            Version::new(1, 1, 0),
        );

        ExtnMetadata {
            name: "launcher".into(),
            symbols: vec![launcher_channel_meta],
        }
    }

    fn init_library() -> CExtnMetadata {
        mock_extn_metadata().into()
    }

    // Test for TryInto<ExtnMetadata> implementation
    #[test]
    fn test_try_into_extn_metadata() {
        // Mock extnMetadata
        let extn_metadata: Box<CExtnMetadata> = Box::new(mock_extn_metadata().into());

        // Perform the conversion
        let result: Result<ExtnMetadata, RippleError> = extn_metadata.try_into();
        assert!(result.is_ok());

        // Extract metadata
        let metadata = result.unwrap();
        // Assertions
        assert_eq!(metadata.name, "launcher");
        assert_eq!(metadata.symbols.len(), 1);

        let symbol = &metadata.symbols[0];
        // Assert ExtnId
        assert_eq!(symbol.id._type, ExtnType::Channel);
        assert_eq!(symbol.id.class, ExtnClassId::Launcher);
        assert_eq!(symbol.id.service, "internal");

        // Assert ContractFulfiller
        assert_eq!(symbol.fulfills.contracts.len(), 1);
        assert!(symbol
            .fulfills
            .contracts
            .contains(&RippleContract::Launcher));

        // Assert required_version
        assert_eq!(symbol.required_version.major, 1);
        assert_eq!(symbol.required_version.minor, 1);
        assert_eq!(symbol.required_version.patch, 0);
    }

    // Test for From<ExtnMetadata> implementation
    #[test]
    fn test_from_extn_metadata() {
        // Mock extnMetadata
        let extn_metadata: ExtnMetadata = mock_extn_metadata();

        // Convert to CExtnMetadata using From
        let c_extn_metadata: CExtnMetadata = extn_metadata.clone().into();
        assert_eq!(extn_metadata.name, "launcher");

        let symbols: Vec<CExtnSymbolMetadata> =
            serde_json::from_str(&c_extn_metadata.metadata).unwrap();
        assert_eq!(symbols.len(), 1);

        let symbol = &symbols[0];
        assert_eq!(symbol.id, "ripple:channel:launcher:internal");
        assert_eq!(symbol.fulfills, format!("[\"\\\"launcher\\\"\"]"));

        // Parse required_version and assert its components
        let required_version = Version::parse(&symbol.required_version).unwrap();
        assert_eq!(required_version.major, 1);
        assert_eq!(required_version.minor, 1);
        assert_eq!(required_version.patch, 0);
    }

    // Test for ExtnSymbolMetadata methods
    #[test]
    fn test_extn_symbol_metadata_methods() {
        let extn_symbol_metadata = ExtnSymbolMetadata::get(
            ExtnId::new_channel(ExtnClassId::Launcher, "internal".into()),
            ContractFulfiller::new(vec![RippleContract::Launcher]),
            Version::new(1, 1, 0),
        );
        // Assert id, fulfills, and required_version using the individual components
        assert_eq!(extn_symbol_metadata.id._type, ExtnType::Channel);
        assert_eq!(extn_symbol_metadata.id.class, ExtnClassId::Launcher);
        assert_eq!(extn_symbol_metadata.id.service, "internal");
        assert!(extn_symbol_metadata
            .fulfills
            .contracts
            .contains(&RippleContract::Launcher));

        // Parse required_version and assert its components
        let version = &extn_symbol_metadata.required_version;
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 1);
        assert_eq!(version.patch, 0);

        // Test get_contract
        assert_eq!(
            extn_symbol_metadata.get_contract(),
            ContractFulfiller::new(vec![RippleContract::Launcher])
        );

        // Test get_version
        assert_eq!(
            extn_symbol_metadata.get_version(),
            Version::parse("1.1.0").unwrap()
        );
    }

    // Test for export_extn_metadata! macro
    #[test]
    fn test_export_extn_metadata_macro() {
        export_extn_metadata!(CExtnMetadata, init_library);
        // TODO - add assertions here based on the expected behavior.
        // For example, loading the metadata from a dynamic library and checking if it matches the expected values.
    }
}
