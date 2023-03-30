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
    api::{ firebolt::{fb_pin::{PinChallengeRequest, PIN_CHALLENGE_CAPABILITY}, provider::{ProviderResponsePayload, ProviderRequestPayload}}},
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::{ExtnMessage, ExtnResponse},
    },
    tokio::sync::{mpsc::{Receiver as MReceiver, Sender as MSender}, oneshot},
};

use crate::{state::platform_state::PlatformState, service::apps::provider_broker::{ProviderBrokerRequest, ProviderBroker}};

/// Supports processing of [Config] request from extensions and also
/// internal services.
#[derive(Debug)]
pub struct PinProcessor {
    state: PlatformState,
    streamer: DefaultExtnStreamer,
}

impl PinProcessor {
    pub fn new(state: PlatformState) -> PinProcessor {
        PinProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for PinProcessor {
    type STATE = PlatformState;
    type VALUE = PinChallengeRequest;
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
impl ExtnRequestProcessor for PinProcessor {
    fn get_client(&self) -> ripple_sdk::extn::client::extn_client::ExtnClient {
        self.state.get_client().get_extn_client()
    }

    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        let pin_request = extracted_message;
        let (session_tx, session_rx) = oneshot::channel::<ProviderResponsePayload>();
        let pr_msg = ProviderBrokerRequest {
            capability: String::from(PIN_CHALLENGE_CAPABILITY),
            method: String::from("challenge"),
            caller: pin_request.clone().requestor,
            request: ProviderRequestPayload::PinChallenge(pin_request),
            tx: session_tx,
            app_id: None,
        };
        ProviderBroker::invoke_method(&state, pr_msg).await;
        match session_rx.await {
            Ok(result) => match result.as_pin_challenge_response() {
                Some(res) => {
                    if let Ok(_) = Self::respond(state.get_client().get_extn_client(), msg.clone(), ExtnResponse::PinChallenge(res))
                    .await {
                        return true;
                    }
                },
                None => {},
            },
            Err(_) => {},
        }
        Self::handle_error(state.get_client().get_extn_client(), msg, ripple_sdk::utils::error::RippleError::Permission(ripple_sdk::api::gateway::rpc_error::DenyReason::Unpermitted)).await
    }
}
