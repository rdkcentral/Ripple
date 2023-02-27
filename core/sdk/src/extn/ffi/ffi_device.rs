use crate::{
    api::device::device_operator::DeviceChannelRequest,
    extn::{client::extn_sender::ExtnSender, extn_capability::ExtnCapability},
    utils::error::RippleError,
};
use crossbeam::channel::Receiver as CReceiver;
use libloading::{Library, Symbol};
use log::{debug, error};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::ffi_message::CExtnMessage;

#[repr(C)]
#[derive(Debug)]
pub struct DeviceChannel {
    pub start: fn(client: ExtnSender, receiver: CReceiver<CExtnMessage>, extns: Vec<DeviceExtn>),
    pub version: Version,
    pub capability: ExtnCapability,
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct DeviceExtn {
    pub service: String,
    pub get_request: fn(params: Value) -> CDeviceRequest,
    pub process: fn(value: Value) -> Result<Value, RippleError>,
}

#[repr(C)]
#[derive(Debug)]
pub struct DeviceExtnBuilder {
    pub build: fn(cap_str: &'static str) -> Option<DeviceExtn>,
    pub caps: Vec<&'static str>,
}

impl DeviceExtnBuilder {
    pub fn get_all(self: Box<Self>) -> Vec<DeviceExtn> {
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
macro_rules! export_device_channel {
    ($plugin_type:ty, $constructor:path) => {
        #[no_mangle]
        pub extern "C" fn device_channel_create() -> *mut DeviceChannel {
            let constructor: fn() -> $plugin_type = $constructor;
            let object = constructor();
            let boxed = Box::new(object);
            Box::into_raw(boxed)
        }
    };
}

pub unsafe fn load_device_channel(lib: &Library) -> Option<Box<DeviceChannel>> {
    type LibraryFfi = unsafe fn() -> *mut DeviceChannel;
    let r = lib.get(b"device_channel_create");
    match r {
        Ok(r) => {
            debug!("Symbol extracted from library");
            let constructor: Symbol<LibraryFfi> = r;
            return Some(Box::from_raw(constructor()));
        }
        Err(e) => error!("Extn library symbol loading failed {:?}", e),
    }
    None
}

#[repr(C)]
#[derive(Debug)]
pub struct CDeviceRequest {
    request: Value,
}

impl From<DeviceChannelRequest> for CDeviceRequest {
    fn from(request: DeviceChannelRequest) -> Self {
        Self {
            request: serde_json::to_value(request).unwrap(),
        }
    }
}

impl Into<DeviceChannelRequest> for CDeviceRequest {
    fn into(self) -> DeviceChannelRequest {
        serde_json::from_value(self.request).unwrap()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceResponse {
    Call(Value),
    Error(RippleError),
}

#[macro_export]
macro_rules! export_device_extn_builder {
    ($plugin_type:ty, $constructor:path) => {
        #[no_mangle]
        pub extern "C" fn device_extn_builder_create() -> *mut DeviceExtnBuilder {
            let constructor: fn() -> $plugin_type = $constructor;
            let object = constructor();
            let boxed = Box::new(object);
            Box::into_raw(boxed)
        }
    };
}

pub unsafe fn load_device_extn_builder(lib: &Library) -> Option<Box<DeviceExtnBuilder>> {
    type LibraryFfi = unsafe fn() -> *mut DeviceExtnBuilder;
    let r = lib.get(b"device_extn_builder_create");
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
