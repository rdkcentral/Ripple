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

use libloading::{Library, Symbol};
use log::{debug, error};

use crate::utils::error::RippleError;

/// Generic Extension channel
#[repr(C)]
#[derive(Debug)]
pub struct ExtnChannel {
    pub start: fn(),
}

/// # Safety
/// TODO: Why is this unsafe allowed here? https://rust-lang.github.io/rust-clippy/master/index.html#missing_safety_doc
pub unsafe fn load_channel_builder(lib: &Library) -> Result<Box<ExtnChannel>, RippleError> {
    type LibraryFfi = unsafe fn() -> *mut ExtnChannel;
    let r = lib.get(b"channel_builder_create");
    match r {
        Ok(r) => {
            debug!("Device Extn Builder Symbol extracted from library");
            let constructor: Symbol<LibraryFfi> = r;
            return Ok(Box::from_raw(constructor()));
        }
        Err(e) => error!("Device Extn Builder symbol loading failed {:?}", e),
    }
    Err(RippleError::ExtnError)
}

/// Macro used by Extensions to export a channel.
///
/// # Example
/// ```
/// use ripple_sdk::extn::ffi::ffi_channel::ExtnChannel;
/// use ripple_sdk::export_extn_channel;
/// fn start() {
///  // snip
/// }
///
/// fn init_channel() -> ExtnChannel {
///     ExtnChannel {///        
///         start,
///     }
/// }
///
/// export_extn_channel!(ExtnChannel, init_channel);
/// ```
#[macro_export]
macro_rules! export_extn_channel {
    ($plugin_type:ty, $constructor:path) => {
        #[no_mangle]
        pub extern "C" fn channel_builder_create() -> *mut ExtnChannel {
            let constructor: fn() -> $plugin_type = $constructor;
            let object = constructor();
            let boxed = Box::new(object);
            Box::into_raw(boxed)
        }
    };
}
