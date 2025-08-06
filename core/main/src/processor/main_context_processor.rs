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

use std::sync::{Arc, RwLock};

use ripple_sdk::{
    api::{
        context::{ActivationStatus, RippleContext, RippleContextUpdateType},
        device::{
            device_request::{PowerState, SystemPowerState},
            device_user_grants_data::GrantLifespan,
        },
        firebolt::fb_capabilities::{CapEvent, CapabilityRole, FireboltCap, FireboltPermission},
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
    tokio::sync::{mpsc::Receiver as MReceiver, mpsc::Sender as MSender},
};

use crate::state::{cap::cap_state::CapState, platform_state::PlatformState};

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

    ///
    /// Method which gets called on bootstrap for a presence of account session
    ///
    async fn check_account_session_token(state: &PlatformState) -> bool {
        let mut token_available = false;
        let mut event = CapEvent::OnUnavailable;

        if let Ok(response) = state
            .get_client()
            .send_extn_request(AccountSessionRequest::Get)
            .await
        {
            if let Some(session) = response.payload.extract() {
                state.clone().session_state.insert_account_session(session);
                event = CapEvent::OnAvailable;
                token_available = true;
            }
        }
        CapState::emit(
            state.clone(),
            &event,
            FireboltCap::Short("token:account".to_owned()),
            None,
        )
        .await;
        CapState::emit(
            state.clone(),
            &event,
            FireboltCap::Short("token:platform".to_owned()),
            None,
        )
        .await;
        token_available
    }

    fn is_update_token(state: &PlatformState) -> bool {
        let available_result = state
            .cap_state
            .generic
            .check_available(&vec![FireboltPermission {
                cap: FireboltCap::Short("token:account".to_owned()),
                role: CapabilityRole::Use,
            }]);
        debug!("token::platform available status: {:?}", available_result);
        available_result.is_ok()
    }
    pub async fn initialize_session(state: &PlatformState) {
        // If the platform:token capability is available then the current call is
        // to update token. If not it is the first time we are receiving token
        // information
        let update_token = Self::is_update_token(state);
        if !update_token && !Self::check_account_session_token(state).await {
            error!("Account session still not available");
        }
    }

    fn handle_power_state(state: PlatformState, power_state: &Option<SystemPowerState>) {
        // fn handle_power_state(state: &PlatformState, power_state: &SystemPowerState) {
        let power_state = match power_state {
            Some(state) => state,
            None => return,
        };

        if matches!(power_state.power_state, PowerState::On)
            && Self::handle_power_active_cleanup(state.clone())
        {
            // if power_state.power_state != PowerState::On && Self::handle_power_active_cleanup(state) {
            info!("Usergrants updated for Powerstate");
        }
    }

    pub fn handle_power_active_cleanup(state: PlatformState) -> bool {
        state
            .clone()
            .cap_state
            .grant_state
            .delete_all_entries_for_lifespan(&GrantLifespan::PowerActive)
    }

    pub fn remove_expired_and_inactive_entries(state: &PlatformState) {
        state.cap_state.grant_state.cleanup_user_grants();
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
        debug!(
            "[REFRESH TOKEN] received context event: {:?}",
            extracted_message
        );
        if let Some(update) = &extracted_message.update_type {
            match update {
                RippleContextUpdateType::TokenChanged => {
                    if let Some(ActivationStatus::AccountToken(t)) =
                        &extracted_message.activation_status
                    {
                        state
                            .state
                            .session_state
                            .insert_session_token(t.token.clone());
                        Self::initialize_session(&state.state).await
                    }
                }
                RippleContextUpdateType::PowerStateChanged => {
                    Self::handle_power_state(state.state, &extracted_message.system_power_state)
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
