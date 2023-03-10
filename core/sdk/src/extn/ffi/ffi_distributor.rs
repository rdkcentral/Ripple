use crate::{
    api::distributor::distributor_session::DistributorSession,
    extn::{client::extn_sender::ExtnSender, extn_capability::ExtnCapability},
    utils::error::RippleError,
};
use crossbeam::channel::Receiver as CReceiver;
use libloading::{Library, Symbol};
use log::{debug, error};
use semver::Version;
use serde_json::Value;

use super::ffi_message::CExtnMessage;

/// Extension channel derived specifically for Distributor Operations.
/// Accepts Distributor Extensions and the Sender Receiver for ExtnClient setup
#[repr(C)]
#[derive(Debug)]
pub struct DistributorChannel {
    pub start:
        fn(client: ExtnSender, receiver: CReceiver<CExtnMessage>, extns: Vec<DistributorExtn>),
    pub version: Version,
    pub capability: ExtnCapability,
}

/// Macro used by Extensions to export a distributor channel.
///
/// # Example
/// ```
/// use ripple_sdk::export_distributor_channel;
/// use ripple_sdk::extn::client::extn_sender::ExtnSender;
/// use ripple_sdk::extn::ffi::ffi_message::CExtnMessage;
/// use ripple_sdk::extn::ffi::ffi_distributor::DistributorExtn;
/// use ripple_sdk::extn::ffi::ffi_distributor::DistributorChannel;
/// use ripple_sdk::extn::extn_capability::{ExtnClass,ExtnCapability};
/// use ripple_sdk::crossbeam::channel::Receiver as CReceiver;
/// use semver::Version;
/// fn start(sender: ExtnSender, receiver: CReceiver<CExtnMessage>, extns: Vec<DistributorExtn>) {
///  // snip
/// }
///  fn init_distributor_channel() -> DistributorChannel {
///    DistributorChannel {
///        start,
///        capability: ExtnCapability::new_channel(ExtnClass::Distributor, "general".into()),
///        version: Version::new(1, 1, 0),
///    }
/// }
///
/// export_distributor_channel!(DistributorChannel, init_distributor_channel);
/// ```
#[macro_export]
macro_rules! export_distributor_channel {
    ($plugin_type:ty, $constructor:path) => {
        #[no_mangle]
        pub extern "C" fn distributor_channel_create() -> *mut DistributorChannel {
            let constructor: fn() -> $plugin_type = $constructor;
            let object = constructor();
            let boxed = Box::new(object);
            Box::into_raw(boxed)
        }
    };
}

/// Method used by Ripple Main to load the device channel
pub unsafe fn load_distributor_channel(
    lib: &Library,
) -> Result<Box<DistributorChannel>, RippleError> {
    type LibraryFfi = unsafe fn() -> *mut DistributorChannel;
    let r = lib.get(b"distributor_channel_create");
    match r {
        Ok(r) => {
            debug!("Symbol extracted from library");
            let constructor: Symbol<LibraryFfi> = r;
            return Ok(Box::from_raw(constructor()));
        }
        Err(e) => error!("Extn library symbol loading failed {:?}", e),
    }
    Err(RippleError::ExtnError)
}

/// DistributorExtn Extn struct provides the Service and other parameters required for Extending distributor operations
#[repr(C)]
#[derive(Debug, Clone)]
pub struct DistributorExtn {
    pub service: String,
    pub process: fn(value: Value, session: DistributorSession) -> Result<Value, RippleError>,
}

#[repr(C)]
#[derive(Debug)]
pub struct DistributorExtnBuilder {
    pub build: fn(cap_str: &'static str) -> Option<DistributorExtn>,
    pub caps: Vec<&'static str>,
}

impl DistributorExtnBuilder {
    pub fn get_all(self: Box<Self>) -> Vec<DistributorExtn> {
        let mut extns = Vec::new();
        for cap in self.caps {
            if let Some(extn) = (self.build)(cap) {
                extns.push(extn)
            }
        }
        extns
    }
}

#[macro_export]
macro_rules! export_distributor_extn_builder {
    ($plugin_type:ty, $constructor:path) => {
        #[no_mangle]
        pub extern "C" fn distributor_extn_builder_create() -> *mut DistributorExtnBuilder {
            let constructor: fn() -> $plugin_type = $constructor;
            let object = constructor();
            let boxed = Box::new(object);
            Box::into_raw(boxed)
        }
    };
}

pub unsafe fn load_distributor_extn_builder(lib: &Library) -> Option<Box<DistributorExtnBuilder>> {
    type LibraryFfi = unsafe fn() -> *mut DistributorExtnBuilder;
    let r = lib.get(b"distributor_extn_builder_create");
    match r {
        Ok(r) => {
            debug!("Device Extn Builder Symbol extracted from library");
            let constructor: Symbol<LibraryFfi> = r;
            return Some(Box::from_raw(constructor()));
        }
        Err(e) => error!("Device Extn Builder symbol loading failed {:?}", e),
    }
    None
}
