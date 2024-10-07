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

use std::time::{SystemTime, UNIX_EPOCH};

use ripple_sdk::framework::bootstrap::Bootstep;
use ripple_sdk::tokio;
use ripple_sdk::{async_trait::async_trait, framework::RippleResponse};

use crate::processor::main_context_processor::MainContextProcessor;
use crate::service::context_manager::ContextManager;
use crate::state::bootstrap_state::BootstrapState;
use crate::state::metrics_state::MetricsState;

pub struct LoadDistributorValuesStep;

// <pca> debug
fn get_current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .try_into()
        .unwrap()
}
// </pca>

#[async_trait]
impl Bootstep<BootstrapState> for LoadDistributorValuesStep {
    fn get_name(&self) -> String {
        "LoadDistributorSessionStep".into()
    }

    async fn setup(&self, s: BootstrapState) -> RippleResponse {
        println!(
            "setup: Calling MetricsState::initialize: {}",
            get_current_time_ms()
        );
        let ps = s.platform_state.clone();
        tokio::spawn(async move {
            MetricsState::initialize(&ps).await;
        });
        println!(
            "setup: Calling MainContextProcessor::remove_expired_and_inactive_entries: {}",
            get_current_time_ms()
        );
        MainContextProcessor::remove_expired_and_inactive_entries(&s.platform_state);
        println!(
            "setup: Calling ContextManager::setup: {}",
            get_current_time_ms()
        );
        ContextManager::setup(&s.platform_state).await; // <pca> Also makes some thunder calls </pca>
        println!("setup: Calling Mark 4: {}", get_current_time_ms());
        if !s.platform_state.supports_session() {
            println!("setup: Session not supported: {}", get_current_time_ms());
            return Ok(());
        }
        println!(
            "setup: Calling MainContextProcessor::initialize_session: {}",
            get_current_time_ms()
        );
        MainContextProcessor::initialize_session(&s.platform_state).await;
        println!(
            "setup: Calling add_event_processor(MainContextProcessor): {}",
            get_current_time_ms()
        );

        s.platform_state
            .get_client()
            .add_event_processor(MainContextProcessor::new(s.platform_state.clone()));
        println!("setup: Exiting: {}", get_current_time_ms());
        Ok(())
    }
}
