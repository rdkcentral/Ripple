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
    api::status_update::ExtnStatus,
    async_trait::async_trait,
    framework::{bootstrap::Bootstep, RippleResponse},
    log::{error, warn},
    tokio::sync::mpsc,
    utils::error::RippleError,
};

use crate::state::{bootstrap_state::BootstrapState, extn_state::PreLoadedExtnChannel};

fn start_preloaded_channel(
    state: &BootstrapState,
    channel: PreLoadedExtnChannel,
) -> RippleResponse {
    let client = state.platform_state.get_client();

    if let Err(e) = state.extn_state.clone().start_channel(channel, client) {
        error!("Error during Device channel bootstrap");
        return Err(e);
    }

    Ok(())
}

/// Bootstep which starts the All Extns channels intitiating including the device interface connection channel.
/// This step calls the start method on the all the Channels and waits for a successful
/// [ExtnStatus] before proceeding to the next boot step.
pub struct StartExtnChannelsStep;

#[async_trait]
impl Bootstep<BootstrapState> for StartExtnChannelsStep {
    fn get_name(&self) -> String {
        "StartExtnChannelsStep".into()
    }
    async fn setup(&self, state: BootstrapState) -> Result<(), RippleError> {
        let mut extn_ids = Vec::new();
        {
            let mut device_channels = state.extn_state.device_channels.write().unwrap();
            while let Some(device_channel) = device_channels.pop() {
                let id = device_channel.extn_id.clone();
                extn_ids.push(id);
                if let Err(e) = start_preloaded_channel(&state, device_channel) {
                    error!("Error during Device channel bootstrap");
                    return Err(e);
                }
            }
        }

        {
            let mut deferred_channels = state.extn_state.deferred_channels.write().unwrap();
            while let Some(deferred_channel) = deferred_channels.pop() {
                let id = deferred_channel.extn_id.clone();
                extn_ids.push(id);
                if let Err(e) = start_preloaded_channel(&state, deferred_channel) {
                    error!("Error during channel bootstrap");
                    return Err(e);
                }
            }
        }
        for extn_id in extn_ids {
            let (tx, mut tr) = mpsc::channel(1);
            if !state
                .extn_state
                .add_extn_status_listener(extn_id.clone(), tx)
            {
                while let Some(v) = tr.recv().await {
                    match v {
                        ExtnStatus::Ready => break,
                        // When Extension is in this state means it has minimal success criteria for continuing
                        // yet not fully ready due to some errors.
                        // Expectation of the system here progressive to wait for an eventual success
                        // without exiting with the error
                        ExtnStatus::Interrupted => warn!("{} extension is interrupted state. Bootstrap currently paused until extension becomes ready.", extn_id.to_string()),
                        ExtnStatus::Error => {
                            error!("{} extension failed to load. Ripple needs to be restarted.",extn_id.to_string());
                            return Err(RippleError::BootstrapError);
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
