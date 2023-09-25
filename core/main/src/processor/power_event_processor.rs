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
    api::apps::PowerStateEventRequest,
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnEventProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::ExtnMessage,
    },
    tokio::sync::mpsc::Sender,
};

use crate::state::platform_state::PlatformState;

/// Processor to service incoming RPC Requests used by extensions and other local rpc handlers for aliasing.
#[derive(Debug)]
pub struct PowerStateEventProcessor {
    state: PlatformState,
    streamer: DefaultExtnStreamer,
}

impl PowerStateEventProcessor {
    pub fn new(state: PlatformState) -> PowerStateEventProcessor {
        PowerStateEventProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for PowerStateEventProcessor {
    type STATE = PlatformState;
    type VALUE = PowerStateEventRequest;
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
impl ExtnEventProcessor for PowerStateEventProcessor {
    async fn process_event(
        state: Self::STATE,
        _msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> Option<bool> {
        println!("^^^^^ in power state event processor process event");
        // match extracted_message.clone() {
        //     AppEventRequest::Emit(event) => {
        //         if let Some(app_id) = event.clone().app_id {
        //             let event_name = &event.event_name;
        //             let result = &event.result;
        //             AppEvents::emit_to_app(&state, app_id, event_name, result).await;
        //         } else {
        //             AppEvents::emit_with_app_event(&state, extracted_message).await;
        //         }
        //     }
        //     AppEventRequest::Register(ctx, event, request) => {
        //         AppEvents::add_listener(&state, event, ctx, request);
        //     }
        // }
        None
    }
}