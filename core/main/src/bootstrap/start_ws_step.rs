// If not stated otherwise in this file or this component's license file the
// following copyright and licenses apply:
//
// Copyright 2023 RDK Management
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

use ripple_sdk::{
    async_trait::async_trait, framework::bootstrap::Bootstep, tokio, utils::error::RippleError,
};

use crate::state::bootstrap_state::BootstrapState;

use crate::firebolt::firebolt_ws::FireboltWs;

pub struct StartWsStep;

#[async_trait]
impl Bootstep<BootstrapState> for StartWsStep {
    fn get_name(&self) -> String {
        "StartWsStep".into()
    }

    async fn setup(&self, state: BootstrapState) -> Result<(), RippleError> {
        let manifest = state.platform_state.get_device_manifest();
        let iai = manifest.get_internal_app_id();
        let ws_enabled = manifest.get_web_socket_enabled();
        let internal_ws_enabled = manifest.get_internal_ws_enabled();
        let iai_c = iai.clone();
        if ws_enabled {
            let ws_addr = manifest.clone().get_ws_gateway_host();
            let state_for_ws = state.platform_state.clone();
            tokio::spawn(async move {
                FireboltWs::start(ws_addr.as_str(), state_for_ws, true, iai.clone()).await;
            });
        }

        if internal_ws_enabled {
            let ws_addr = manifest.clone().get_internal_gateway_host();
            let state_for_ws = state.platform_state.clone();
            tokio::spawn(async move {
                FireboltWs::start(ws_addr.as_str(), state_for_ws, false, iai_c).await;
            });
        }

        Ok(())
    }
}
