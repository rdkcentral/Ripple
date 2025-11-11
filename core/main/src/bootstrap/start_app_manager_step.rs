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

use ripple_sdk::async_trait::async_trait;
use ripple_sdk::{framework::bootstrap::Bootstep, tokio, utils::error::RippleError};

use crate::{
    service::apps::delegated_launcher_handler::DelegatedLauncherHandler,
    state::bootstrap_state::BootstrapState,
};

/// Starts the App Manager and other supporting services
pub struct StartAppManagerStep;

#[async_trait]
impl Bootstep<BootstrapState> for StartAppManagerStep {
    fn get_name(&self) -> String {
        "StartAppManager".into()
    }

    async fn setup(&self, state: BootstrapState) -> Result<(), RippleError> {
        let mut app_manager =
            DelegatedLauncherHandler::new(state.channels_state, state.platform_state);
        tokio::spawn(async move {
            app_manager.start().await;
        });
        Ok(())
    }
}
