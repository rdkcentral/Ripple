use ripple_sdk::{
    async_trait::async_trait,
    extn::{
        extn_capability::{ExtnClass, ExtnType},
        ffi::{
            ffi_channel::{load_extn_channel, ExtnChannel},
            ffi_device::{
                load_device_channel, load_device_extn_builder, DeviceChannel, DeviceExtn,
            },
            ffi_distributor::{load_distributor_channel, DistributorChannel},
        },
    },
    framework::bootstrap::Bootstep,
    log::{debug, info},
    utils::error::RippleError,
};

use crate::state::bootstrap_state::BootstrapState;

/// Actual bootstep which loads the extensions into the ExtnState.
/// Currently this step loads
/// 1. Device Channel
/// 2. Device Extensions
pub struct LoadExtensionsStep;

#[async_trait]
impl Bootstep<BootstrapState> for LoadExtensionsStep {
    fn get_name(&self) -> String {
        "LoadExtensionsStep".into()
    }
    async fn setup(&self, state: BootstrapState) -> Result<(), RippleError> {
        let loaded_extensions = state.extn_state.loaded_libraries.read().unwrap();
        let mut device_extns: Vec<DeviceExtn> = Vec::new();
        let mut launcher_channel: Option<Box<ExtnChannel>> = None;
        let mut device_channel: Option<Box<DeviceChannel>> = None;
        let mut dist_channel: Option<Box<DistributorChannel>> = None;
        for extn in loaded_extensions.iter() {
            unsafe {
                let library = &extn.library;
                debug!("loading symbols from {}", extn.get_metadata().name);
                let extn_metadata = extn.get_metadata().metadata;
                for metadata in extn_metadata.iter() {
                    let cap_string = metadata.get_cap().to_string();
                    debug!("loading extension {}", cap_string);
                    match metadata.get_cap().get_type() {
                        ExtnType::Channel => match metadata.get_cap().class() {
                            ExtnClass::Device => match load_device_channel(library) {
                                Ok(channel) => {
                                    info!("Adding to channel map {}", cap_string.clone());
                                    let _ = device_channel.insert(channel);
                                }
                                Err(e) => return Err(e),
                            },
                            ExtnClass::Launcher => match load_extn_channel(library) {
                                Ok(channel) => {
                                    info!("Adding launcher to channel map {}", cap_string.clone());
                                    let _ = launcher_channel.insert(channel);
                                }
                                Err(e) => return Err(e),
                            },
                            ExtnClass::Distributor => match load_distributor_channel(library) {
                                Ok(channel) => {
                                    info!("Adding Distributor channel {}", cap_string.clone());
                                    let _ = dist_channel.insert(channel);
                                }
                                Err(e) => return Err(e),
                            },
                            _ => {}
                        },
                        ExtnType::Extn => match metadata.get_cap().class() {
                            ExtnClass::Device => match load_device_extn_builder(library) {
                                Some(builder) => {
                                    device_extns.extend(builder.get_all());
                                }
                                None => info!("no device extns loaded"),
                            },
                            _ => {}
                        },
                        _ => {}
                    }
                }
            }
        }

        {
            let mut device_channel_state = state.extn_state.device_channel.write().unwrap();
            let _ = device_channel_state.insert(device_channel.unwrap());
            info!("Device channel extension loaded");
        }

        if launcher_channel.is_some() {
            let mut launcher_channel_state = state.extn_state.launcher_channel.write().unwrap();
            let _ = launcher_channel_state.insert(launcher_channel.unwrap());
            info!("Launcher channel extension loaded");
        }

        if dist_channel.is_some() {
            let mut distributor_channel_state =
                state.extn_state.distributor_channel.write().unwrap();
            let _ = distributor_channel_state.insert(dist_channel.unwrap());
            info!("Distributor channel extension loaded");
        }

        {
            let mut device_extn = state.extn_state.device_extns.write().unwrap();
            info!("Total device extns loaded {}", device_extns.len());
            let _ = device_extn.insert(device_extns);
        }

        Ok(())
    }
}
