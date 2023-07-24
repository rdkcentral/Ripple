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

use crate::extn::client::extn_sender::ExtnSender;
use crossbeam::channel::Receiver as CReceiver;
use jsonrpsee_core::server::rpc_module::Methods;
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
    pub get_extended_capabilities: fn() -> Option<String>,
    pub build: fn(client: ExtnSender, receiver: CReceiver<CExtnMessage>) -> Methods,
    pub service: String,
}

pub unsafe fn load_jsonrpsee_methods(lib: &Library) -> Option<Box<JsonRpseeExtnBuilder>> {
    type LibraryFfi = unsafe fn() -> *mut JsonRpseeExtnBuilder;
    let r = lib.get(b"jsonrpsee_extn_builder_create");
    match r {
        Ok(r) => {
            debug!("Jsonrpsee Extn Builder Symbol extracted from library");
            let constructor: Symbol<LibraryFfi> = r;
            return Some(Box::from_raw(constructor()));
        }
        Err(e) => error!("Jsonrpsee Extn Builder symbol loading failed {:?}", e),
    }
    None
}
