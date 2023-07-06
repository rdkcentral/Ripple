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
use ripple_sdk::{
    api::{
        device::device_user_grants_data::{GrantEntry, GrantLifespan, GrantStatus},
        firebolt::fb_capabilities::FireboltPermission,
        usergrant_entry::{UserGrantInfo, UserGrantsStoreRequest},
    },
    async_trait::async_trait,
    extn::client::extn_processor::{
        DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
    },
    extn::extn_client_message::{ExtnMessage, ExtnResponse},
    log::debug,
    tokio::sync::mpsc::{Receiver as MReceiver, Sender as MSender},
};
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
    type VALUE = UserGrantsStoreRequest;
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

impl StoreUserGrantsProcessor {
    async fn process_get_request(
        state: &PlatformState,
        msg: ExtnMessage,
        app_id: String,
        permission: FireboltPermission,
    ) -> bool {
        let result = state
            .cap_state
            .grant_state
            .get_grant_status(&app_id, &permission);
        if let Some(granted) = result {
            return Self::respond(
                state.get_client().get_extn_client(),
                msg,
                ExtnResponse::Boolean(match granted {
                    GrantStatus::Allowed => true,
                    GrantStatus::Denied => false,
                }),
            )
            .await
            .is_ok();
        } else {
            return Self::respond(
                state.get_client().get_extn_client(),
                msg,
                ExtnResponse::None(()),
            )
            .await
            .is_ok();
        }
    }
    async fn process_set_request(
        state: &PlatformState,
        msg: ExtnMessage,
        user_grant_info: UserGrantInfo,
    ) -> bool {
        let app_id = user_grant_info.app_name.to_owned();
        let grant_entry = GrantEntry {
            role: user_grant_info.role,
            capability: user_grant_info.capability.to_owned(),
            status: Some(user_grant_info.status),
            lifespan: match user_grant_info.expiry_time {
                Some(_) => Some(GrantLifespan::Seconds),
                None => Some(GrantLifespan::Forever),
            },
            last_modified_time: user_grant_info.last_modified_time,
            lifespan_ttl_in_secs: user_grant_info
                .expiry_time
                .map(|epoch_duration| epoch_duration.as_secs()),
        };
        state
            .cap_state
            .grant_state
            .update_grant_entry(app_id, grant_entry);
        return Self::respond(
            state.get_client().get_extn_client(),
            msg,
            ExtnResponse::None(()),
        )
        .await
        .is_ok();
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
        match extracted_message {
            UserGrantsStoreRequest::GetUserGrants(app_id, permission) => {
                Self::process_get_request(&state, msg, app_id, permission).await
            }
            UserGrantsStoreRequest::SetUserGrants(user_grant_info) => {
                Self::process_set_request(&state, msg, user_grant_info).await
            }
        }
    }
}
