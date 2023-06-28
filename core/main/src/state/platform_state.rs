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

use std::collections::HashMap;

use ripple_sdk::{
    api::{
        gateway::rpc_gateway_api::ApiMessage,
        manifest::{
            app_library::AppLibraryState,
            device_manifest::{AppLibraryEntry, DeviceManifest},
            extn_manifest::ExtnManifest,
        },
        protocol::BridgeProtocolRequest,
    },
    extn::{extn_client_message::ExtnMessage, extn_id::ExtnId},
    framework::{ripple_contract::RippleContract, RippleResponse},
    utils::error::RippleError,
};

use crate::{
    firebolt::rpc_router::RouterState,
    service::{
        apps::{
            app_events::AppEventsState, delegated_launcher_handler::AppManagerState,
            provider_broker::ProviderBrokerState,
        },
        data_governance::DataGovernanceState,
        extn::ripple_client::RippleClient,
    },
};

use super::{cap::cap_state::CapState, openrpc_state::OpenRpcState, session_state::SessionState};

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
    pub open_rpc_state: OpenRpcState,
    pub router_state: RouterState,
    pub data_governance: DataGovernanceState,
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
            cap_state: CapState::new(manifest.clone()),
            session_state: SessionState::default(),
            device_manifest: manifest.clone(),
            ripple_client: client,
            app_library_state: AppLibraryState::new(app_library),
            app_events_state: AppEventsState::default(),
            provider_broker_state: ProviderBrokerState::default(),
            app_manager_state: AppManagerState::default(),
            open_rpc_state: OpenRpcState::new(manifest.clone().configuration.exclusory),
            router_state: RouterState::new(),
            data_governance: DataGovernanceState::default(),
        }
    }

    pub fn has_internal_launcher(&self) -> bool {
        self.extn_manifest.get_launcher_capability().is_some()
    }

    pub fn get_launcher_capability(&self) -> Option<ExtnId> {
        self.extn_manifest.get_launcher_capability()
    }

    pub fn get_distributor_capability(&self) -> Option<ExtnId> {
        self.extn_manifest.get_distributor_capability()
    }

    pub fn get_manifest(&self) -> ExtnManifest {
        self.extn_manifest.clone()
    }

    pub fn get_rpc_aliases(&self) -> HashMap<String, Vec<String>> {
        self.extn_manifest.clone().rpc_aliases
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

    pub fn supports_bridge(&self) -> bool {
        let contract = RippleContract::BridgeProtocol.as_clear_string();
        self.extn_manifest.required_contracts.contains(&contract)
    }

    pub fn supports_cloud_sync(&self) -> bool {
        let contract = RippleContract::CloudSync.as_clear_string();
        self.extn_manifest.required_contracts.contains(&contract)
    }

    pub fn supports_encoding(&self) -> bool {
        let contract = RippleContract::Encoder.as_clear_string();
        self.extn_manifest.required_contracts.contains(&contract)
    }

    pub async fn send_to_bridge(&self, id: String, msg: ApiMessage) -> RippleResponse {
        let request = BridgeProtocolRequest::Send(id, msg);
        if let Err(e) = self.get_client().send_extn_request(request).await {
            return Err(e);
        }
        Ok(())
    }
}
