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
    api::session::EventAdjective, framework::ripple_contract::RippleContract,
    utils::error::RippleError,
};

use crate::{
    events::thunder_event_processor::ThunderEventHandlerProvider,
    ripple_sdk::{
        api::device::device_events::{DeviceEvent, DeviceEventRequest},
        async_trait::async_trait,
        extn::{
            client::{
                extn_client::ExtnClient,
                extn_processor::{
                    DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
                },
            },
            extn_client_message::ExtnMessage,
        },
        tokio::sync::mpsc,
    },
    thunder_state::ThunderState,
};

use super::events::thunder_event_handlers::{AudioChangedEvent, HDCPEventHandler};

#[derive(Debug)]
pub struct ThunderOpenEventsProcessor {
    state: ThunderState,
    streamer: DefaultExtnStreamer,
}

impl ThunderOpenEventsProcessor {
    pub fn new(state: ThunderState) -> ThunderOpenEventsProcessor {
        ThunderOpenEventsProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for ThunderOpenEventsProcessor {
    type STATE = ThunderState;
    type VALUE = DeviceEventRequest;

    fn get_state(&self) -> Self::STATE {
        self.state.clone()
    }

    fn receiver(&mut self) -> mpsc::Receiver<ExtnMessage> {
        self.streamer.receiver()
    }

    fn sender(&self) -> mpsc::Sender<ExtnMessage> {
        self.streamer.sender()
    }
    fn fulfills_mutiple(&self) -> Option<Vec<RippleContract>> {
        Some(vec![
            RippleContract::DeviceEvents(EventAdjective::Input),
            RippleContract::DeviceEvents(EventAdjective::VoiceGuidance),
            RippleContract::DeviceEvents(EventAdjective::Audio),
        ])
    }
}

#[async_trait]
impl ExtnRequestProcessor for ThunderOpenEventsProcessor {
    fn get_client(&self) -> ExtnClient {
        self.state.get_client()
    }

    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        let event = extracted_message.clone().event;
        let listen = extracted_message.clone().subscribe;
        let callback_type = extracted_message.clone().callback_type;
        let id = callback_type.get_id();
        if let Some(v) = match event {
            DeviceEvent::AudioChanged => Some(state.handle_listener(
                listen,
                id.clone(),
                AudioChangedEvent::provide(id, callback_type),
            )),
            DeviceEvent::InputChanged => Some(state.handle_listener(
                listen,
                id.clone(),
                HDCPEventHandler::provide(id, callback_type),
            )),
        } {
            v.await;
            Self::ack(state.get_client(), msg).await.is_ok()
        } else {
            Self::handle_error(state.get_client(), msg, RippleError::InvalidInput).await
        }
    }
}
