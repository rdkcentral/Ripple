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
    api::observability::analytics::AnalyticsEvent,
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnEventProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::ExtnMessage,
    },
    tokio::sync::mpsc::{Receiver as MReceiver, Sender as MSender},
    utils::error::RippleError,
};
use serde::{Deserialize, Serialize};

use crate::{client::thunder_plugin::ThunderPlugin, thunder_state::ThunderState};

#[derive(Debug)]
pub struct ThunderAnalyticsProcessor {
    state: ThunderState,
    streamer: DefaultExtnStreamer,
}

impl ThunderAnalyticsProcessor {
    pub fn new(state: ThunderState) -> ThunderAnalyticsProcessor {
        println!("*** _DEBUG: ThunderAnalyticsProcessor::new: entry");
        ThunderAnalyticsProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for ThunderAnalyticsProcessor {
    type VALUE = AnalyticsEvent;
    type STATE = ThunderState;

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
impl ExtnEventProcessor for ThunderAnalyticsProcessor {
    async fn process_event(
        state: Self::STATE,
        _msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> Option<bool> {
        println!(
            "*** _DEBUG: ThunderAnalyticsProcessor::process_event: extracted_message={:?}",
            extracted_message
        );
        match extracted_message {
            AnalyticsEvent::SendMetrics(data) => {
                println!(
                    "*** _DEBUG: ThunderAnalyticsProcessor::process_event: data={:?}",
                    data
                );
            }
        }

        None
    }
}
