// Copyright 2023 Comcast Cable Communications Management, LLC

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at

// http://www.apache.org/licenses/LICENSE-2.0

// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// SPDX-License-Identifier: Apache-2.0

use ripple_sdk::{
    api::device::device_request::NetworkState,
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
pub struct NetworkEventProcessor {
    state: PlatformState,
    streamer: DefaultExtnStreamer,
}

impl NetworkEventProcessor {
    pub fn new(state: PlatformState) -> NetworkEventProcessor {
        NetworkEventProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for NetworkEventProcessor {
    type STATE = PlatformState;
    type VALUE = NetworkState;
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
impl ExtnEventProcessor for NetworkEventProcessor {
    async fn process_event(
        _state: Self::STATE,
        _msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> Option<bool> {
        match extracted_message.clone() {
            NetworkState::Connected => {
                println!("Network is connected");
            }
            NetworkState::Disconnected => {
                println!("Network is disconnected")
            }
        }
        None
    }
}
