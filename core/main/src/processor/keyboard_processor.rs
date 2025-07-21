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
    api::firebolt::{
        fb_capabilities::DenyReason,
        fb_keyboard::{KeyboardSessionRequest, KEYBOARD_PROVIDER_CAPABILITY},
        provider::{ProviderRequestPayload, ProviderResponsePayload},
    },
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::{ExtnMessage, ExtnResponse},
    },
    tokio::sync::{
        mpsc::{Receiver as MReceiver, Sender as MSender},
        oneshot,
    },
};

use crate::{
    service::apps::provider_broker::{ProviderBroker, ProviderBrokerRequest},
    state::platform_state::PlatformState,
};

/// Supports processing of Keyboard request from extensions and also
/// internal services.
#[derive(Debug)]
pub struct KeyboardProcessor {
    state: PlatformState,
    streamer: DefaultExtnStreamer,
}

impl KeyboardProcessor {
    pub fn new(state: PlatformState) -> KeyboardProcessor {
        KeyboardProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for KeyboardProcessor {
    type STATE = PlatformState;
    type VALUE = KeyboardSessionRequest;
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
impl ExtnRequestProcessor for KeyboardProcessor {
    fn get_client(&self) -> ripple_sdk::extn::client::extn_client::ExtnClient {
        self.state.get_client().get_extn_client()
    }

    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        let method = String::from(extracted_message._type.to_provider_method());
        let (session_tx, session_rx) = oneshot::channel::<ProviderResponsePayload>();
        let pr_msg = ProviderBrokerRequest {
            capability: KEYBOARD_PROVIDER_CAPABILITY.to_string(),
            method,
            caller: extracted_message.clone().ctx.into(),
            request: ProviderRequestPayload::KeyboardSession(extracted_message),
            tx: session_tx,
            app_id: None,
        };
        ProviderBroker::invoke_method(state.clone(), pr_msg).await;
        if let Ok(result) = session_rx.await {
            if let Some(keyboard_response) = result.as_keyboard_result() {
                if Self::respond(
                    state.get_client().get_extn_client(),
                    msg.clone(),
                    ExtnResponse::Keyboard(keyboard_response),
                )
                .await
                .is_ok()
                {
                    return true;
                }
            }
        }
        Self::handle_error(
            state.get_client().get_extn_client(),
            msg,
            ripple_sdk::utils::error::RippleError::Permission(DenyReason::Unpermitted),
        )
        .await
    }
}
