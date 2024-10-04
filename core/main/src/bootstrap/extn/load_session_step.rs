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
use ripple_sdk::tokio;
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
        println!("setup: Calling MetricsState::initialize");
        let ps = s.platform_state.clone();
        tokio::spawn(async move {
            MetricsState::initialize(&ps).await;
        });
        println!("setup: Calling MainContextProcessor::remove_expired_and_inactive_entries");
        MainContextProcessor::remove_expired_and_inactive_entries(&s.platform_state);
        println!("setup: Calling ContextManager::setup");
        ContextManager::setup(&s.platform_state).await; // <pca> Also makes some thunder calls </pca>
        println!("setup: Calling Mark 4");
        if !s.platform_state.supports_session() {
            println!("setup: Session not supported");
            return Ok(());
        }
        println!("setup: Calling MainContextProcessor::initialize_session");
        MainContextProcessor::initialize_session(&s.platform_state).await;
        println!("setup: Calling add_event_processor(MainContextProcessor)");

        s.platform_state
            .get_client()
            .add_event_processor(MainContextProcessor::new(s.platform_state.clone()));
        println!("setup: Exiting");
        Ok(())
    }
}
