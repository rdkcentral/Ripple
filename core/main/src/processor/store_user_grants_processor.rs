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
use crate::state::platform_state::PlatformState;
use ripple_sdk::api::device::device_user_grants_data::GrantEntry;
use ripple_sdk::api::device::device_user_grants_data::GrantLifespan;
use ripple_sdk::api::firebolt::fb_capabilities::CapabilityRole;
use ripple_sdk::api::usergrant_entry::UserGrantInfo;
use ripple_sdk::async_trait::async_trait;
use ripple_sdk::extn::client::extn_processor::DefaultExtnStreamer;
use ripple_sdk::extn::client::extn_processor::ExtnRequestProcessor;
use ripple_sdk::extn::client::extn_processor::ExtnStreamProcessor;
use ripple_sdk::extn::client::extn_processor::ExtnStreamer;
use ripple_sdk::extn::extn_client_message::ExtnMessage;
use ripple_sdk::extn::extn_client_message::ExtnResponse;
use ripple_sdk::log::{debug, info};

use ripple_sdk::tokio::sync::mpsc::{Receiver as MReceiver, Sender as MSender};
#[derive(Debug)]
pub struct StoreUserGrantsProcessor {
    state: PlatformState,
    streamer: DefaultExtnStreamer,
}

impl StoreUserGrantsProcessor {
    pub fn new(state: PlatformState) -> StoreUserGrantsProcessor {
        StoreUserGrantsProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for StoreUserGrantsProcessor {
    type STATE = PlatformState;
    type VALUE = UserGrantInfo;
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
impl ExtnRequestProcessor for StoreUserGrantsProcessor {
    fn get_client(&self) -> ripple_sdk::extn::client::extn_client::ExtnClient {
        self.state.get_client().get_extn_client()
    }

    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        debug!("process request: Received message: {:?}", extracted_message);
        let app_id = extracted_message.app_name.to_owned();
        let grant_entry = GrantEntry {
            role: extracted_message.role,
            capability: extracted_message.capability.to_owned(),
            status: Some(extracted_message.status),
            lifespan: match extracted_message.expiry_time {
                Some(_) => Some(GrantLifespan::Seconds),
                None => Some(GrantLifespan::Forever),
            },
            last_modified_time: extracted_message.last_modified_time,
            lifespan_ttl_in_secs: extracted_message
                .expiry_time
                .map(|epoch_duration| epoch_duration.as_secs()),
        };
        state
            .cap_state
            .grant_state
            .update_grant_entry(app_id, grant_entry);
        Self::respond(
            state.get_client().get_extn_client(),
            msg,
            ExtnResponse::None(()),
        )
        .await
        .is_ok();
        true
    }
}
