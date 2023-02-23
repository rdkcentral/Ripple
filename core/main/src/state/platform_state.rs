use ripple_sdk::api::manifest::device_manifest::DeviceManifest;

use crate::service::extn::ripple_client::RippleClient;

use super::{cap::cap_state::CapState, session_state::SessionState};

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
#[derive(Debug, Clone)]
pub struct PlatformState {
    device_manifest: DeviceManifest,
    ripple_client: RippleClient,
    pub session_state: SessionState,
    pub cap_state: CapState,
}

impl PlatformState {
    pub fn new(manifest: DeviceManifest, client: RippleClient) -> PlatformState {
        Self {
            cap_state: CapState::default(),
            session_state: SessionState::default(),
            device_manifest: manifest,
            ripple_client: client,
        }
    }

    pub fn get_device_manifest(&self) -> DeviceManifest {
        self.device_manifest.clone()
    }

    pub fn get_client(&self) -> RippleClient {
        self.ripple_client.clone()
    }
}
