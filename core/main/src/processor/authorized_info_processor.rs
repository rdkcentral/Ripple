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
    api::caps::CapsRequest,
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::{ExtnMessage, ExtnResponse},
    },
    tokio::sync::mpsc::Sender,
};

use crate::state::platform_state::PlatformState;

/// Processor to service incoming RPC Requests used by extensions and other local rpc handlers for aliasing.
#[derive(Debug)]
pub struct AuthorizedInfoProcessor {
    state: PlatformState,
    streamer: DefaultExtnStreamer,
}

impl AuthorizedInfoProcessor {
    pub fn new(state: PlatformState) -> AuthorizedInfoProcessor {
        AuthorizedInfoProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for AuthorizedInfoProcessor {
    type STATE = PlatformState;
    type VALUE = CapsRequest;
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
impl ExtnRequestProcessor for AuthorizedInfoProcessor {
    fn get_client(&self) -> ripple_sdk::extn::client::extn_client::ExtnClient {
        self.state.get_client().get_extn_client()
    }

    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        match extracted_message {
            CapsRequest::Permitted(app_id, request) => {
                println!("@@@NNA...CapsRequest::Permitted is invoked here..");
                let result = state
                    .cap_state
                    .permitted_state
                    .check_multiple(&app_id, request);
                Self::respond(
                    state.get_client().get_extn_client(),
                    msg,
                    ExtnResponse::BoolMap(result),
                )
                .await
                .is_ok()
            }
            CapsRequest::Supported(request) => {
                println!("@@@NNA...CapsRequest::Supported invoked here");
                let result = state.cap_state.generic.check_for_processor(request);
                println!("@@@NNA...CapsRequest::Supported result: {:?}", result);
                Self::respond(
                    state.get_client().get_extn_client(),
                    msg,
                    ExtnResponse::BoolMap(result),
                )
                .await
                .is_ok()
            }
        }
    }
}
