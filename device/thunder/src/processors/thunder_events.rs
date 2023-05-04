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

use thunder_ripple_sdk::{
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

use super::events::thunder_event_handlers::{
    AudioChangedEvent, HDCPEventHandler, HDREventHandler, NetworkEventHandler,
    ScreenResolutionEventHandler, SystemPowerStateChangeEventHandler, VideoResolutionEventHandler,
    VoiceGuidanceEnabledChangedEventHandler,
};

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
        let id = extracted_message.clone().id;
        let listen = extracted_message.clone().subscribe;
        let v = match event {
            DeviceEvent::AudioChanged => {
                state.handle_listener(listen, id.clone(), AudioChangedEvent::provide(id))
            }
            DeviceEvent::HdrChanged => {
                state.handle_listener(listen, id.clone(), HDREventHandler::provide(id))
            }
            DeviceEvent::InputChanged => {
                state.handle_listener(listen, id.clone(), HDCPEventHandler::provide(id))
            }
            DeviceEvent::NetworkChanged => {
                state.handle_listener(listen, id.clone(), NetworkEventHandler::provide(id))
            }
            DeviceEvent::ScreenResolutionChanged => state.handle_listener(
                listen,
                id.clone(),
                ScreenResolutionEventHandler::provide(id),
            ),
            DeviceEvent::VideoResolutionChanged => {
                state.handle_listener(listen, id.clone(), VideoResolutionEventHandler::provide(id))
            }
            DeviceEvent::SystemPowerStateChanged => state.handle_listener(
                listen,
                id.clone(),
                SystemPowerStateChangeEventHandler::provide(id),
            ),
            DeviceEvent::VoiceGuidanceChanged => state.handle_listener(
                listen,
                id.clone(),
                VoiceGuidanceEnabledChangedEventHandler::provide(id),
            ),
        };
        v.await;
        Self::ack(state.get_client(), msg).await.is_ok()
    }
}
