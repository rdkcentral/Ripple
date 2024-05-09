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
    api::{
        distributor::distributor_token::DistributorTokenRequest,
        firebolt::fb_authentication::TokenResult,
        session::{SessionAdjective, TokenType},
    },
    async_trait::async_trait,
    extn::{
        client::{
            extn_client::ExtnClient,
            extn_processor::{
                DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
            },
        },
        extn_client_message::ExtnResponse,
    },
    framework::ripple_contract::RippleContract,
};

pub struct DistributorTokenProcessor {
    client: ExtnClient,
    streamer: DefaultExtnStreamer,
}

impl DistributorTokenProcessor {
    pub fn new(client: ExtnClient) -> DistributorTokenProcessor {
        DistributorTokenProcessor {
            client,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for DistributorTokenProcessor {
    type STATE = ExtnClient;
    type VALUE = DistributorTokenRequest;

    fn get_state(&self) -> Self::STATE {
        self.client.clone()
    }

    fn receiver(
        &mut self,
    ) -> ripple_sdk::tokio::sync::mpsc::Receiver<ripple_sdk::extn::extn_client_message::ExtnMessage>
    {
        self.streamer.receiver()
    }

    fn sender(
        &self,
    ) -> ripple_sdk::tokio::sync::mpsc::Sender<ripple_sdk::extn::extn_client_message::ExtnMessage>
    {
        self.streamer.sender()
    }

    fn fulfills_mutiple(
        &self,
    ) -> Option<Vec<ripple_sdk::framework::ripple_contract::RippleContract>> {
        Some(vec![RippleContract::Session(SessionAdjective::Distributor)])
    }
}

#[async_trait]
impl ExtnRequestProcessor for DistributorTokenProcessor {
    fn get_client(&self) -> ExtnClient {
        self.client.clone()
    }
    async fn process_request(
        state: Self::STATE,
        msg: ripple_sdk::extn::extn_client_message::ExtnMessage,
        _extracted_message: Self::VALUE,
    ) -> bool {
        let token = ExtnResponse::Token(TokenResult {_type:TokenType::Distributor,expires:None,value:"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c".into(),scope:None,expires_in:None, token_type: None });
        Self::respond(state.clone(), msg, token).await.is_ok()
    }
}
