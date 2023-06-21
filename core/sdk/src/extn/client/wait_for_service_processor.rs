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

use crate::{
    api::status_update::ExtnStatus,
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnEventProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::ExtnMessage,
        extn_id::ExtnId,
    },
    log::error,
    tokio::sync::{mpsc::Receiver as MReceiver, mpsc::Sender as MSender},
};

#[derive(Debug, Clone)]
pub struct WaitForState {
    capability: ExtnId,
    sender: MSender<ExtnStatus>,
}

#[derive(Debug)]
pub struct WaitForStatusReadyEventProcessor {
    state: WaitForState,
    streamer: DefaultExtnStreamer,
}

/// Event processor used for cases where a certain Extension Capability is required to be ready.
/// Bootstrap uses the [WaitForStatusReadyEventProcessor] to await during Device Connnection before starting the gateway.
impl WaitForStatusReadyEventProcessor {
    pub fn new(
        capability: ExtnId,
        sender: MSender<ExtnStatus>,
    ) -> WaitForStatusReadyEventProcessor {
        WaitForStatusReadyEventProcessor {
            state: WaitForState { capability, sender },
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for WaitForStatusReadyEventProcessor {
    type VALUE = ExtnStatus;
    type STATE = WaitForState;

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
impl ExtnEventProcessor for WaitForStatusReadyEventProcessor {
    async fn process_event(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> Option<bool> {
        if msg.requestor.to_string().eq(&state.capability.to_string()) {
            match extracted_message {
                ExtnStatus::Ready => {
                    if let Err(_) = state.sender.send(ExtnStatus::Ready).await {
                        error!("Failure to wait status message")
                    }
                    return Some(true);
                }
                _ => {}
            }
        }
        None
    }
}
