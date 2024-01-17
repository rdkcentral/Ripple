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
use ripple_sdk::log::debug;
use ripple_sdk::{async_trait::async_trait, framework::RippleResponse};

use crate::processor::main_context_processor::MainContextProcessor;
use crate::service::context_manager::ContextManager;
use crate::state::bootstrap_state::BootstrapState;
use crate::state::metrics_state::MetricsState;

pub struct LoadDistributorValuesStep;

#[async_trait]
impl Bootstep<BootstrapState> for LoadDistributorValuesStep {
    fn get_name(&self) -> String {
        "LoadDistributorSessionStep".into()
    }

    async fn setup(&self, s: BootstrapState) -> RippleResponse {
        MetricsState::initialize(&s.platform_state).await;
        if MainContextProcessor::handle_power_active_cleanup(&s.platform_state) {
            debug!("Power Active grants were cleaned up");
        }
        ContextManager::setup(&s.platform_state).await;
        if !s.platform_state.supports_session() {
            return Ok(());
        }
        MainContextProcessor::initialize_session(&s.platform_state).await;
        // Session available during startup make sure the context is updated with the token for first time
        if s.platform_state
            .session_state
            .get_account_session()
            .is_some()
        {
            ContextManager::update_context_for_session(s.platform_state.clone());
        }

        s.platform_state
            .get_client()
            .add_event_processor(MainContextProcessor::new(s.platform_state.clone()));
        Ok(())
    }
}
