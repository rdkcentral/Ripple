use crossbeam::channel::Receiver as CReceiver;
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
    pub build: fn(extn_id: String) -> Result<Box<ExtnChannel>, RippleError>,
    pub service: String,
}

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
/// use ripple_sdk::export_extn_channel;
/// use ripple_sdk::extn::client::extn_sender::ExtnSender;
/// use ripple_sdk::extn::ffi::ffi_message::CExtnMessage;
/// use ripple_sdk::extn::ffi::ffi_channel::ExtnChannel;
/// use ripple_sdk::extn::extn_capability::{ExtnClass,ExtnCapability};
/// use ripple_sdk::crossbeam::channel::Receiver as CReceiver;
/// use semver::Version;
/// fn start(sender: ExtnSender, receiver: CReceiver<CExtnMessage>) {
///  // snip
/// }
///  fn init_channel() -> ExtnChannel {
///    ExtnChannel {
///        start
///    }
/// }
///
/// export_extn_channel!(ExtnChannel, init_channel);
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
