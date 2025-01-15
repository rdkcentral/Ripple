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

use async_channel::Receiver as CReceiver;
use libloading::{Library, Symbol};
use log::{debug, error};

use crate::{extn::client::extn_sender::ExtnSender, utils::error::RippleError};

use super::ffi_message::CExtnMessage;

/// Generic Extension channel
#[repr(C)]
#[derive(Debug)]
pub struct ExtnChannel {
    pub start: fn(client: ExtnSender, receiver: CReceiver<CExtnMessage>),
}

#[repr(C)]
#[derive(Debug)]
pub struct ExtnChannelBuilder {
    pub get_extended_capabilities: fn() -> Option<String>,
    pub build: fn(extn_id: String) -> Result<Box<ExtnChannel>, RippleError>,
    pub service: String,
}

/// # Safety
/// TODO: Why is this unsafe allowed here? https://rust-lang.github.io/rust-clippy/master/index.html#missing_safety_doc
pub unsafe fn load_channel_builder(lib: &Library) -> Result<Box<ExtnChannelBuilder>, RippleError> {
    type LibraryFfi = unsafe fn() -> *mut ExtnChannelBuilder;
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
/// use ripple_sdk::extn::client::extn_sender::ExtnSender;
/// use ripple_sdk::extn::ffi::ffi_message::CExtnMessage;
/// use ripple_sdk::extn::ffi::ffi_channel::{ExtnChannel,ExtnChannelBuilder};
/// use ripple_sdk::extn::extn_id::{ExtnClassId};
/// use ripple_sdk::async_channel::Receiver as CReceiver;
/// use semver::Version;
/// use ripple_sdk::utils::error::RippleError;
/// use ripple_sdk::export_channel_builder;
/// fn start(sender: ExtnSender, receiver: CReceiver<CExtnMessage>) {
///  // snip
/// }
///  fn init_builder_channel() -> ExtnChannelBuilder {
///    ExtnChannelBuilder {
///        build,
///        service: "info".into()
///    }
/// }
///
/// fn build(extn_id: String) -> Result<Box<ExtnChannel>, RippleError> {
///     Ok(Box::new(ExtnChannel {
///     start
/// }))
/// }
///
/// export_channel_builder!(ExtnChannelBuilder, init_builder_channel);
/// ```
#[macro_export]
macro_rules! export_channel_builder {
    ($plugin_type:ty, $constructor:path) => {
        #[no_mangle]
        pub extern "C" fn channel_builder_create() -> *mut ExtnChannelBuilder {
            let constructor: fn() -> $plugin_type = $constructor;
            let object = constructor();
            let boxed = Box::new(object);
            Box::into_raw(boxed)
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extn::client::extn_sender::tests::Mockable;

    #[test]
    fn test_extn_channel_builder() {
        // Mock implementation for the build function
        fn build_fn(_extn_id: String) -> Result<Box<ExtnChannel>, RippleError> {
            // Mock implementation for creating an ExtnChannel
            let extn_channel = ExtnChannel {
                start: |_client, _receiver| {
                    // Mock implementation for ExtnChannel's start function
                },
            };
            Ok(Box::new(extn_channel))
        }

        // Mock service name
        let service = "mock_service".to_string();

        // Create an instance of ExtnChannelBuilder with the mock build function
        let extn_channel_builder = ExtnChannelBuilder {
            build: build_fn,
            service,
        };

        // Use the ExtnChannelBuilder to create an ExtnChannel
        let extn_id = "some_extn_id".to_string();
        let result = (extn_channel_builder.build)(extn_id);

        // Perform assertions or actions based on the expected behavior
        match result {
            Ok(extn_channel) => {
                let (mock_sender, rx) = ExtnSender::mock();
                // If the build was successful, you can now use the created ExtnChannel
                (extn_channel.start)(mock_sender, rx);
            }
            Err(err) => {
                // Handle the error case if the build fails
                panic!("Error building ExtnChannel: {:?}", err);
            }
        }
    }
}
