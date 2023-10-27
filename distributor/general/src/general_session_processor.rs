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
        device::device_request::AccountToken,
        session::{
            AccountSession, AccountSessionRequest, AccountSessionTokenRequest, ProvisionRequest,
        },
    },
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
    framework::file_store::FileStore,
    log::error,
    utils::error::RippleError,
};
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone)]
pub struct DistSessionState {
    client: ExtnClient,
    session: Arc<RwLock<FileStore<AccountSession>>>,
}

fn get_privacy_path(saved_dir: String) -> String {
    format!("{}/{}", saved_dir, "dist_session")
}

impl DistSessionState {
    fn new(client: ExtnClient, path: String) -> Self {
        let path = get_privacy_path(path);
        let store = if let Ok(v) = FileStore::load(path.clone()) {
            v
        } else {
            FileStore::new(
                path,
                AccountSession {
                    id: "general".into(),
                    token: "general".into(),
                    account_id: "general".into(),
                    device_id: "general".into(),
                },
            )
        };

        Self {
            client,
            session: Arc::new(RwLock::new(store)),
        }
    }
}

pub struct DistributorSessionProcessor {
    state: DistSessionState,
    streamer: DefaultExtnStreamer,
}

impl DistributorSessionProcessor {
    pub fn new(client: ExtnClient, path: String) -> DistributorSessionProcessor {
        DistributorSessionProcessor {
            state: DistSessionState::new(client, path),
            streamer: DefaultExtnStreamer::new(),
        }
    }

    async fn get_token(mut state: DistSessionState, msg: ExtnMessage) -> bool {
        let session = state.session.read().unwrap().value.clone();
        if let Err(e) = state
            .client
            .respond(
                msg.clone(),
                ripple_sdk::extn::extn_client_message::ExtnResponse::AccountSession(
                    ripple_sdk::api::session::AccountSessionResponse::AccountSession(session),
                ),
            )
            .await
        {
            error!("Error sending back response {:?}", e);
            return false;
        }

        Self::handle_error(state.clone().client, msg, RippleError::ExtnError).await
    }

    async fn set_token(
        state: DistSessionState,
        msg: ExtnMessage,
        token: AccountSessionTokenRequest,
    ) -> bool {
        {
            let mut session = state.session.write().unwrap();
            session.value.token = token.token;
            session.sync();
        }
        Self::ack(state.client, msg).await.is_ok()
    }

    async fn provision(
        state: DistSessionState,
        msg: ExtnMessage,
        provision: ProvisionRequest,
    ) -> bool {
        {
            let mut session = state.session.write().unwrap();
            session.value.account_id = provision.account_id;
            session.value.device_id = provision.device_id;
            if let Some(distributor) = provision.distributor_id {
                session.value.id = distributor;
            }

            session.sync();
        }
        Self::ack(state.client, msg).await.is_ok()
    }

    async fn get_accesstoken(mut state: DistSessionState, msg: ExtnMessage) -> bool {
        let device_token = AccountToken {
            // Mock invalidated token for validation
            token: "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9".into(),
            expires: 0,
        };
        if let Err(e) = state
            .client
            .respond(
                msg.clone(),
                ripple_sdk::extn::extn_client_message::ExtnResponse::AccountSession(
                    ripple_sdk::api::session::AccountSessionResponse::AccountSessionToken(
                        device_token,
                    ),
                ),
            )
            .await
        {
            error!("Error sending back response {:?}", e);
            return false;
        }

        Self::handle_error(state.clone().client, msg, RippleError::ExtnError).await
    }
}

impl ExtnStreamProcessor for DistributorSessionProcessor {
    type STATE = DistSessionState;
    type VALUE = AccountSessionRequest;

    fn get_state(&self) -> Self::STATE {
        self.state.clone()
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
}

#[async_trait]
impl ExtnRequestProcessor for DistributorSessionProcessor {
    fn get_client(&self) -> ExtnClient {
        self.state.clone().client
    }
    async fn process_request(
        state: Self::STATE,
        msg: ripple_sdk::extn::extn_client_message::ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        match extracted_message {
            AccountSessionRequest::Get => Self::get_token(state.clone(), msg).await,
            AccountSessionRequest::Provision(p) => Self::provision(state.clone(), msg, p).await,
            AccountSessionRequest::SetAccessToken(s) => Self::set_token(state, msg, s).await,
            AccountSessionRequest::GetAccessToken => {
                Self::get_accesstoken(state.clone(), msg).await
            }
            AccountSessionRequest::Subscribe => Self::ack(state.clone().client, msg).await.is_ok(),
        }
    }
}
