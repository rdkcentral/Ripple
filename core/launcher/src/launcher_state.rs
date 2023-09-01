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

use ripple_sdk::{
    api::config::{Config, LauncherConfig},
    extn::{
        client::extn_client::ExtnClient,
        extn_client_message::{ExtnMessage, ExtnPayloadProvider},
    },
    framework::bootstrap::TransientChannel,
    utils::error::RippleError,
};

use crate::manager::{
    app_launcher::AppLauncherState,
    container_manager::ContainerState,
    view_manager::{ViewRequest, ViewState},
};

#[derive(Debug, Clone)]
pub struct LauncherBootstrapState {
    pub channels: LauncherChannels,
    pub state: LauncherState,
}

#[derive(Debug, Clone)]
pub struct LauncherChannels {
    pub view_manager_channel: TransientChannel<ViewRequest>,
}

#[derive(Debug, Clone)]
pub struct LauncherState {
    pub config: LauncherConfig,
    pub view_state: ViewState,
    pub container_state: ContainerState,
    pub app_launcher_state: AppLauncherState,
    pub extn_client: ExtnClient,
}

impl LauncherState {
    pub async fn send_extn_request(
        &self,
        payload: impl ExtnPayloadProvider,
    ) -> Result<ExtnMessage, RippleError> {
        self.extn_client.clone().request(payload).await
    }

    pub async fn new(extn_client: ExtnClient) -> Result<LauncherState, RippleError> {
        let extn_message_response: Result<ExtnMessage, RippleError> =
            extn_client.clone().request(Config::LauncherConfig).await;
        if let Ok(message) = extn_message_response {
            if let Some(config) = message.payload.extract() {
                return Ok(LauncherState {
                    config,
                    view_state: ViewState::default(),
                    container_state: ContainerState::default(),
                    app_launcher_state: AppLauncherState::default(),
                    extn_client,
                });
            }
        }

        Err(RippleError::BootstrapError)
    }
}
