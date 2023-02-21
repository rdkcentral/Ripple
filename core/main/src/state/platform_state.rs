use std::sync::{Arc, RwLock};

use ripple_sdk::{
    api::{
        device::DevicePlatformType, manifest::device_manifest::DeviceManifest,
    },
    log::{debug},
    tokio::sync::mpsc::Sender,
    utils::error::RippleError,
};

use crate::{
    firebolt::firebolt_gateway::FireboltGatewayCommand, service::extn::ripple_client::RippleClient,
};

use super::{session_state::SessionState, cap::cap_state::CapState};


/// Platform state encapsulates the internal state of the Ripple Main application.
/// 
/// # Examples
/// ```
/// let state = PlatformState::default();
/// 
/// let manifest = state.get_device_manifest();
/// println!("{}", manifest.unwrap().configuration.platform);
/// ```
/// 
#[derive(Debug, Clone, Default)]
pub struct PlatformState {
    pub device_manifest: Arc<RwLock<Option<DeviceManifest>>>,
    pub ripple_client: Arc<RwLock<Option<RippleClient>>>,
    pub session_state: SessionState,
    pub cap_state: CapState,
}

impl PlatformState {
    /// Provides the device platform information from the device manifest
    /// as this value is read from a file loaded dynamically during runtime the response
    /// provided will always be a result which can have an error. Handler should panic if
    /// no valid platform type is provided.
    ///
    /// # Examples
    /// ```
    /// let state = PlatformState::default();
    /// match state.get_device_platform() {
    ///     Ok(type) => println!("{:?}", type),
    ///     Err(e) => panic!("Invalid  {:?}", e);
    /// }
    /// ```
    pub fn get_device_platform(&self) -> Result<DevicePlatformType, RippleError> {
        let manifest = self.device_manifest.read().unwrap();
        if manifest.is_none() {
            debug!("Device manifest missing");
            return Err(RippleError::BootstrapError);
        }
        let manifest = manifest.clone();
        Ok(manifest.unwrap().configuration.platform)
    }

    /// Returns the device manifest
    /// # Examples
    /// ```
    /// let state = PlatformState::default();
    /// state.get_device_manifest().configuration.platform
    /// ```
    pub fn get_device_manifest(&self) -> DeviceManifest {
        self.device_manifest.read().unwrap().clone().unwrap()
    }

    /// Returns a cloned instance of the `RippleClient`
    /// # Examples
    /// ```
    /// use crate::firebolt::firebolt_gateway::FireboltGatewayCommand,
    /// 
    /// fn send_command(cmd: FireboltGatewayCommand) {
    ///     let state = PlatformState::default();
    ///     state.client().send_gateway_command(cmd)
    /// }
    /// 
    /// ```
    pub fn client(&self) -> RippleClient {
        let ripple_client_state = self.ripple_client.read().unwrap();
        ripple_client_state.clone().unwrap()
    }

    /// This is a convinient passthru method to Ripple Client's `set_gateway_sender` method
    /// # Examples
    /// ```
    /// use crate::firebolt::firebolt_gateway::FireboltGatewayCommand,
    /// use tokio::sync::mpsc::Sender,
    /// fn send_command(sender: Sender<FireboltGatewayCommand>) {
    ///     let state = PlatformState::default();
    ///     state.client().set_gateway_sender(sender)
    /// }
    /// 
    /// ```
    pub fn set_fb_gateway_sender(&self, sender: Sender<FireboltGatewayCommand>) {
        self.client().set_gateway_sender(sender);
    }

}
