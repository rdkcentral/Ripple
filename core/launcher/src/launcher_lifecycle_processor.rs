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
    api::firebolt::{
        fb_discovery::LaunchRequest, fb_lifecycle_management::LifecycleManagementEventRequest,
    },
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnEventProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::ExtnMessage,
    },
    log::error,
    tokio::sync::{mpsc::Receiver as MReceiver, mpsc::Sender as MSender},
};

use crate::{launcher_state::LauncherState, manager::app_launcher::AppLauncher};

/// This processor will be the processor which handles in the incoming LifecycleManagementEvent(s) from the
/// delegated launcher existing in main module.
#[derive(Debug)]
pub struct LauncherLifecycleEventProcessor {
    state: LauncherState,
    streamer: DefaultExtnStreamer,
}

/// Event processor used for cases where a certain Extension Capability is required to be ready.
/// Bootstrap uses the [WaitForStatusReadyEventProcessor] to await during Device Connnection before starting the gateway.
impl LauncherLifecycleEventProcessor {
    pub fn new(state: LauncherState) -> LauncherLifecycleEventProcessor {
        LauncherLifecycleEventProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for LauncherLifecycleEventProcessor {
    type VALUE = LifecycleManagementEventRequest;
    type STATE = LauncherState;

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
impl ExtnEventProcessor for LauncherLifecycleEventProcessor {
    async fn process_event(
        state: Self::STATE,
        _msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> Option<bool> {
        let result = match extracted_message {
            LifecycleManagementEventRequest::Ready(ready) => {
                AppLauncher::ready(&state, &ready.parameters.app_id).await
            }
            LifecycleManagementEventRequest::Close(close) => {
                AppLauncher::close(&state, &close.parameters.app_id, close.parameters.reason).await
            }
            LifecycleManagementEventRequest::Finished(finished) => {
                AppLauncher::destroy(&state, &finished.parameters.app_id).await
            }
            LifecycleManagementEventRequest::Launch(launch) => {
                AppLauncher::launch(
                    &state,
                    LaunchRequest {
                        app_id: launch.parameters.app_id,
                        intent: launch.parameters.intent,
                    },
                )
                .await
            }
            LifecycleManagementEventRequest::Provide(p) => AppLauncher::provide(&state, p).await,
        };
        if let Err(e) = result {
            error!("Error during lifecycle management processing {:?}", e)
        }
        None
    }
}
