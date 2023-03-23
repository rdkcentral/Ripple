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
    extn::{
        client::extn_client::ExtnClient,
        extn_client_message::{ExtnMessage, ExtnPayloadProvider},
    },
    utils::error::RippleError,
};
use url::Url;

use crate::client::thunder_client::ThunderClient;

#[derive(Debug, Clone)]
pub struct ThunderBootstrapStateWithConfig {
    pub extn_client: ExtnClient,
    pub url: Url,
    pub pool_size: u32,
}

#[derive(Debug, Clone)]
pub struct ThunderBootstrapStateWithClient {
    pub prev: ThunderBootstrapStateWithConfig,
    pub state: ThunderState,
}

#[derive(Debug, Clone)]
pub struct ThunderState {
    extn_client: ExtnClient,
    thunder_client: ThunderClient,
}

impl ThunderState {
    pub fn new(extn_client: ExtnClient, thunder_client: ThunderClient) -> ThunderState {
        ThunderState {
            extn_client,
            thunder_client,
        }
    }

    pub fn get_thunder_client(&self) -> ThunderClient {
        self.thunder_client.clone()
    }

    pub fn get_client(&self) -> ExtnClient {
        self.extn_client.clone()
    }

    pub async fn send_payload(
        &self,
        payload: impl ExtnPayloadProvider,
    ) -> Result<ExtnMessage, RippleError> {
        self.extn_client.clone().request(payload).await
    }
}
