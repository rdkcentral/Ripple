use ripple_sdk::{
    api::manifest::{
        app_library::AppLibraryState,
        device_manifest::{AppLibraryEntry, DeviceManifest},
        extn_manifest::ExtnManifest,
    },
    extn::{extn_capability::ExtnCapability, extn_client_message::ExtnMessage},
    utils::error::RippleError,
};

use crate::service::{
    apps::{
        app_events::AppEventsState, delegated_launcher_handler::AppManagerState,
        provider_broker::ProviderBrokerState,
    },
    extn::ripple_client::RippleClient,
};

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
    extn_manifest: ExtnManifest,
    device_manifest: DeviceManifest,
    ripple_client: RippleClient,
    pub app_library_state: AppLibraryState,
    pub session_state: SessionState,
    pub cap_state: CapState,
    pub app_events_state: AppEventsState,
    pub provider_broker_state: ProviderBrokerState,
    pub app_manager_state: AppManagerState,
}

impl PlatformState {
    pub fn new(
        extn_manifest: ExtnManifest,
        manifest: DeviceManifest,
        client: RippleClient,
        app_library: Vec<AppLibraryEntry>,
    ) -> PlatformState {
        Self {
            extn_manifest,
            cap_state: CapState::default(),
            session_state: SessionState::default(),
            device_manifest: manifest,
            ripple_client: client,
            app_library_state: AppLibraryState::new(app_library),
            app_events_state: AppEventsState::default(),
            provider_broker_state: ProviderBrokerState::default(),
            app_manager_state: AppManagerState::default(),
        }
    }

    pub fn has_internal_launcher(&self) -> bool {
        self.extn_manifest.get_launcher_capability().is_some()
    }

    pub fn get_launcher_capability(&self) -> Option<ExtnCapability> {
        self.extn_manifest.get_launcher_capability()
    }

    pub fn get_manifest(&self) -> ExtnManifest {
        self.extn_manifest.clone()
    }

    pub fn get_device_manifest(&self) -> DeviceManifest {
        self.device_manifest.clone()
    }

    pub fn get_client(&self) -> RippleClient {
        self.ripple_client.clone()
    }

    pub async fn respond(&self, msg: ExtnMessage) -> Result<(), RippleError> {
        self.get_client().respond(msg).await
    }
}
