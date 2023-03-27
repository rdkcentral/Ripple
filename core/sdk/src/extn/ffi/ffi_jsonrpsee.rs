use crate::extn::client::extn_sender::ExtnSender;
use crossbeam::channel::Receiver as CReceiver;
use jsonrpsee::core::server::rpc_module::Methods;
use libloading::{Library, Symbol};
use log::{debug, error};

use super::ffi_message::CExtnMessage;

#[macro_export]
macro_rules! export_jsonrpc_extn_builder {
    ($plugin_type:ty, $constructor:path) => {
        #[no_mangle]
        pub extern "C" fn jsonrpsee_extn_builder_create() -> *mut JsonRpseeExtnBuilder {
            let constructor: fn() -> $plugin_type = $constructor;
            let object = constructor();
            let boxed = Box::new(object);
            Box::into_raw(boxed)
        }
    };
}

#[repr(C)]
#[derive(Debug)]
pub struct JsonRpseeExtnBuilder {
    pub build: fn(client: ExtnSender, receiver: CReceiver<CExtnMessage>) -> Methods,
    pub service: String,
}

pub unsafe fn load_jsonrpsee_methods(lib: &Library) -> Option<Box<JsonRpseeExtnBuilder>> {
    type LibraryFfi = unsafe fn() -> *mut JsonRpseeExtnBuilder;
    let r = lib.get(b"jsonrpsee_extn_builder_create");
    match r {
        Ok(r) => {
            debug!("Thunder Extn Builder Symbol extracted from library");
            let constructor: Symbol<LibraryFfi> = r;
            return Some(Box::from_raw(constructor()));
        }
        Err(e) => error!("Thunder Extn Builder symbol loading failed {:?}", e),
    }
    None
}
