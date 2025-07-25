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
    api::storage_property::StorageManagerRequest,
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::{ExtnMessage, ExtnResponse},
    },
    tokio::sync::mpsc::{Receiver as MReceiver, Sender as MSender},
};

use crate::state::platform_state::PlatformState;

use super::storage_manager::StorageManager;

/// Supports processing of [Config] request from extensions and also
/// internal services.
#[derive(Debug)]
pub struct StorageManagerProcessor {
    state: PlatformState,
    streamer: DefaultExtnStreamer,
}

impl StorageManagerProcessor {
    pub fn new(state: PlatformState) -> StorageManagerProcessor {
        StorageManagerProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for StorageManagerProcessor {
    type STATE = PlatformState;
    type VALUE = StorageManagerRequest;
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
impl ExtnRequestProcessor for StorageManagerProcessor {
    fn get_client(&self) -> ripple_sdk::extn::client::extn_client::ExtnClient {
        self.state.get_client().get_extn_client()
    }

    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        let client = state.get_client().get_extn_client();
        match extracted_message {
            StorageManagerRequest::GetBool(key, default_value) => {
                let result = StorageManager::get_bool(state, key)
                    .await
                    .unwrap_or(default_value);
                Self::respond(client, msg, ExtnResponse::Boolean(result))
                    .await
                    .is_ok()
            }
            StorageManagerRequest::GetString(key) => {
                if let Ok(result) = StorageManager::get_string(state, key).await {
                    Self::respond(client, msg, ExtnResponse::String(result))
                        .await
                        .is_ok()
                } else {
                    Self::respond(client, msg, ExtnResponse::None(()))
                        .await
                        .is_ok()
                }
            }
        }
    }
}
