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
    api::apps::AppEventRequest,
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnEventProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::ExtnMessage,
    },
    tokio::sync::mpsc::Sender,
};

use crate::{service::apps::app_events::AppEvents, state::platform_state::PlatformState};

/// Processor to service incoming RPC Requests used by extensions and other local rpc handlers for aliasing.
#[derive(Debug)]
pub struct AppEventsProcessor {
    state: PlatformState,
    streamer: DefaultExtnStreamer,
}

impl AppEventsProcessor {
    pub fn new(state: PlatformState) -> AppEventsProcessor {
        AppEventsProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for AppEventsProcessor {
    type STATE = PlatformState;
    type VALUE = AppEventRequest;
    fn get_state(&self) -> Self::STATE {
        self.state.clone()
    }

    fn sender(&self) -> Sender<ExtnMessage> {
        self.streamer.sender()
    }

    fn receiver(&mut self) -> ripple_sdk::tokio::sync::mpsc::Receiver<ExtnMessage> {
        self.streamer.receiver()
    }
}

#[async_trait]
impl ExtnEventProcessor for AppEventsProcessor {
    async fn process_event(
        state: Self::STATE,
        _msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> Option<bool> {
        match extracted_message.clone() {
            AppEventRequest::Emit(event) => {
                if let Some(app_id) = event.app_id {
                    let event_name = &event.event_name;
                    let result = &event.result;
                    AppEvents::emit_to_app(state.clone(), app_id, event_name, result).await;
                } else {
                    AppEvents::emit_with_app_event(state.clone(), extracted_message).await;
                }
            }
            AppEventRequest::Register(ctx, event, request) => {
                AppEvents::add_listener(state.clone(), event, ctx, request);
            }
        }
        None
    }
}
