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

use ripple_sdk::framework::bootstrap::Bootstep;
use ripple_sdk::{async_trait::async_trait, framework::RippleResponse};

use crate::processor::main_context_processor::MainContextProcessor;
use crate::state::bootstrap_state::BootstrapState;
use crate::state::cap::cap_state::CapState;
use crate::state::platform_state::PlatformState;
use crate::tokio;
use crate::tokio::sync::mpsc;
use jsonrpsee::core::RpcResult;
use ripple_sdk::api::context::RippleContext;
use ripple_sdk::api::context::{ActivationStatus, RippleContextUpdateType};
use ripple_sdk::api::device::device_request::PowerState;
use ripple_sdk::api::device::device_request::SystemPowerState;
use ripple_sdk::api::device::device_user_grants_data::GrantLifespan;
use ripple_sdk::api::firebolt::fb_capabilities::CapEvent;
use ripple_sdk::api::firebolt::fb_capabilities::CapabilityRole;
use ripple_sdk::api::firebolt::fb_capabilities::FireboltCap;
use ripple_sdk::api::firebolt::fb_capabilities::FireboltPermission;
use ripple_sdk::api::session::AccountSessionRequest;
use ripple_sdk::log::{debug, error, info};
use ripple_sdk::service::service_message::{JsonRpcMessage, ServiceMessage};

pub struct LoadDistributorValuesStep;

#[async_trait]
impl Bootstep<BootstrapState> for LoadDistributorValuesStep {
    fn get_name(&self) -> String {
        "LoadDistributorSessionStep".into()
    }

    async fn setup(&self, s: BootstrapState) -> RippleResponse {
        MainContextProcessor::remove_expired_and_inactive_entries(&s.platform_state);
        s.platform_state.cap_state.grant_state.cleanup_user_grants();

        if !s.platform_state.supports_session() {
            return Ok(());
        }

        s.platform_state
            .get_client()
            .add_event_processor(MainContextProcessor::new(s.platform_state.clone()));

        setup_token_event_handler(s.platform_state);

        Ok(())
    }
}

fn setup_token_event_handler(state: PlatformState) {
    let (tx, mut rx) = mpsc::channel::<ServiceMessage>(1);
    tokio::spawn(async move {
        state
            .service_controller_state
            .service_event_state
            .add_event_main_processor(RippleContextUpdateType::TokenChanged, tx);

        while let Some(sm) = rx.recv().await {
            debug!("[REFRESH TOKEN] received context event {:?}", sm);
            match sm.message {
                JsonRpcMessage::Notification(_) => {
                    let result: RpcResult<RippleContext> = sm.parse_rpc_notification();
                    debug!("[REFRESH TOKEN] received parsed rpc result: {:?}", result);
                    match result {
                        Ok(ripple_context) => {
                            if let Some(update_type) = ripple_context.update_type.clone() {
                                match update_type {
                                    RippleContextUpdateType::TokenChanged => {
                                        if let Some(ActivationStatus::AccountToken(t)) =
                                            &ripple_context.activation_status
                                        {
                                            state
                                                .session_state
                                                .insert_session_token(t.token.clone());
                                        }
                                        initialize_session(&state).await;
                                    }
                                    RippleContextUpdateType::PowerStateChanged => {
                                        handle_power_state(
                                            &state,
                                            &ripple_context.system_power_state,
                                        );
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to parse RippleContext from service message: {}", e);
                        }
                    }
                }
                _ => {
                    error!("Received unexpected JsonRpc message");
                }
            }
        }
    });
}

pub async fn initialize_session(state: &PlatformState) {
    // If the platform:token capability is available then the current call is
    // to update token. If not it is the first time we are receiving token
    // information
    let update_token = is_update_token(state);
    if !update_token && !check_account_session_token(state).await {
        error!("Account session still not available");
    }
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
            event = CapEvent::OnAvailable;
            token_available = true;
        }
    }
    CapState::emit(
        state,
        &event,
        FireboltCap::Short("token:account".to_owned()),
        None,
    )
    .await;
    CapState::emit(
        state,
        &event,
        FireboltCap::Short("token:platform".to_owned()),
        None,
    )
    .await;
    token_available
}

fn handle_power_state(state: &PlatformState, power_state: &Option<SystemPowerState>) {
    let power_state = match power_state {
        Some(state) => state,
        None => return,
    };

    if matches!(power_state.power_state, PowerState::On) && handle_power_active_cleanup(state) {
        info!("Usergrants updated for Powerstate");
    }
}

pub fn handle_power_active_cleanup(state: &PlatformState) -> bool {
    state
        .cap_state
        .grant_state
        .delete_all_entries_for_lifespan(&GrantLifespan::PowerActive)
}
