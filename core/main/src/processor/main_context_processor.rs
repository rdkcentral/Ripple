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

use std::{
    sync::{Arc, Once, RwLock},
    time::Duration,
};

use ripple_sdk::{
    api::{
        context::{ActivationStatus, RippleContext, RippleContextUpdateType},
        device::{
            device_request::{PowerState, SystemPowerState},
            device_user_grants_data::GrantLifespan,
        },
        distributor::distributor_sync::{SyncAndMonitorModule, SyncAndMonitorRequest},
        firebolt::fb_capabilities::{CapEvent, FireboltCap},
        manifest::device_manifest::PrivacySettingsStorageType,
        session::AccountSessionRequest,
    },
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnEventProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::ExtnMessage,
    },
    log::{debug, error, info},
    tokio::{
        self,
        sync::{mpsc::Receiver as MReceiver, mpsc::Sender as MSender},
    },
};
static START_PARTNER_EXCLUSION_SYNC_THREAD: Once = Once::new();

use crate::{
    service::data_governance::DataGovernance,
    state::{cap::cap_state::CapState, metrics_state::MetricsState, platform_state::PlatformState},
};

#[derive(Debug, Clone)]
pub struct ContextState {
    current_context: Arc<RwLock<RippleContext>>,
    state: PlatformState,
}

#[derive(Debug)]
pub struct MainContextProcessor {
    state: ContextState,
    streamer: DefaultExtnStreamer,
}

/// Event processor used for cases where a certain Extension Capability is required to be ready.
/// Bootstrap uses the [WaitForStatusReadyEventProcessor] to await during Device Connnection before starting the gateway.
impl MainContextProcessor {
    pub fn new(state: PlatformState) -> MainContextProcessor {
        MainContextProcessor {
            state: ContextState {
                current_context: Arc::new(RwLock::new(RippleContext::default())),
                state,
            },
            streamer: DefaultExtnStreamer::new(),
        }
    }

    async fn check_account_session_token(state: &PlatformState) -> bool {
        let mut token_available = false;
        let mut event = CapEvent::OnUnavailable;

        if let Ok(response) = state
            .get_client()
            .send_extn_request(AccountSessionRequest::Get)
            .await
        {
            if let Some(session) = response.payload.extract() {
                state.session_state.insert_account_session(session);
                MetricsState::update_account_session(state).await;
                event = CapEvent::OnAvailable;
                token_available = true;
            }
        }
        CapState::emit(
            state,
            event,
            FireboltCap::Short("token:platform".to_owned()),
            None,
        )
        .await;
        token_available
    }
    async fn sync_partner_exclusions(state: &PlatformState) {
        let state_for_exclusion = state.clone();
        START_PARTNER_EXCLUSION_SYNC_THREAD.call_once(|| {
            debug!("Starting partner exclusion sync thread");
            tokio::spawn(async move {
                let duration = state_for_exclusion
                    .get_device_manifest()
                    .configuration
                    .partner_exclusion_refresh_timeout
                    .into();
                let mut interval = tokio::time::interval(Duration::from_secs(duration));
                loop {
                    let resp: bool =
                        DataGovernance::refresh_partner_exclusions(&state_for_exclusion).await;
                    debug!(
                        "refresh_partner_exclusions: {:?} interval : {:?}",
                        resp, interval
                    );
                    interval.tick().await;
                }
            });
        });
    }
    pub async fn initialize_token(state: &PlatformState) {
        if !Self::check_account_session_token(state).await {
            error!("Account session still not available");
        } else if state.supports_cloud_sync() {
            debug!("Cloud Sync  configured as a required contract so starting.");
            if state
                .get_device_manifest()
                .configuration
                .features
                .privacy_settings_storage_type
                == PrivacySettingsStorageType::Sync
            {
                debug!(
                "Privacy settings storage type is not set as sync so not starting cloud monitor"
            );
                if let Some(account_session) = state.session_state.get_account_session() {
                    debug!("Successfully got account session");
                    //sync up partner exclusion data and setup polling thread for refreshing it.
                    Self::sync_partner_exclusions(state).await;

                    let sync_response = state
                        .get_client()
                        .send_extn_request(SyncAndMonitorRequest::SyncAndMonitor(
                            SyncAndMonitorModule::Privacy,
                            account_session.clone(),
                        ))
                        .await;
                    debug!("Received Sync response for privacy: {:?}", sync_response);
                    let sync_response = state
                        .get_client()
                        .send_extn_request(SyncAndMonitorRequest::SyncAndMonitor(
                            SyncAndMonitorModule::UserGrants,
                            account_session.clone(),
                        ))
                        .await;
                    debug!(
                        "Received Sync response for user grants: {:?}",
                        sync_response
                    );
                }
            }
        }
    }

    fn handle_power_state(state: &PlatformState, power_state: &SystemPowerState) {
        if power_state.power_state != PowerState::On && Self::handle_power_active_cleanup(state) {
            info!("Usergrants updated for Powerstate");
        }
    }

    pub fn handle_power_active_cleanup(state: &PlatformState) -> bool {
        state
            .cap_state
            .grant_state
            .delete_all_entries_for_lifespan(&GrantLifespan::PowerActive)
    }
}

impl ExtnStreamProcessor for MainContextProcessor {
    type VALUE = RippleContext;
    type STATE = ContextState;

    fn get_state(&self) -> Self::STATE {
        self.state.clone()
    }

    fn sender(&self) -> MSender<ExtnMessage> {
        self.streamer.sender()
    }

    fn receiver(&mut self) -> MReceiver<ExtnMessage> {
        self.streamer.receiver()
    }
}

#[async_trait]
impl ExtnEventProcessor for MainContextProcessor {
    async fn process_event(
        state: Self::STATE,
        _msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> Option<bool> {
        if let Some(update) = &extracted_message.update_type {
            match update {
                RippleContextUpdateType::TokenChanged => {
                    if let ActivationStatus::AccountToken(_t) = &extracted_message.activation_status
                    {
                        Self::initialize_token(&state.state).await
                    }
                }
                RippleContextUpdateType::PowerStateChanged => {
                    Self::handle_power_state(&state.state, &extracted_message.system_power_state)
                }
                _ => {}
            }
            {
                let mut context = state.current_context.write().unwrap();
                context.deep_copy(extracted_message);
            }
        }
        None
    }
}
