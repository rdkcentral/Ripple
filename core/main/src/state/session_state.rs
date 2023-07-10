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

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use ripple_sdk::{
    api::{
        apps::EffectiveTransport, gateway::rpc_gateway_api::ApiMessage, session::AccountSession,
    },
    tokio::sync::mpsc::Sender,
    utils::error::RippleError,
};

#[derive(Debug, Clone)]
pub struct SessionData {
    app_id: String,
    transport: EffectiveTransport,
}

#[derive(Debug, Clone)]
pub struct Session {
    sender: Option<Sender<ApiMessage>>,
    data: SessionData,
}

impl Session {
    pub fn new(
        app_id: String,
        sender: Option<Sender<ApiMessage>>,
        transport: EffectiveTransport,
    ) -> Session {
        Session {
            sender,
            data: SessionData { app_id, transport },
        }
    }

    pub fn get_sender(&self) -> Option<Sender<ApiMessage>> {
        self.sender.clone()
    }

    pub async fn send_json_rpc(&self, msg: ApiMessage) -> Result<(), RippleError> {
        if let Some(sender) = self.get_sender() {
            if let Ok(_) = sender.send(msg).await {
                return Ok(());
            }
        }
        Err(RippleError::SendFailure)
    }

    fn get_app_id(&self) -> String {
        self.data.app_id.clone()
    }

    pub fn get_transport(&self) -> EffectiveTransport {
        self.data.clone().transport
    }
}

/// Session state encapsulates the session table with mappings to Application identifier and
/// callback senders.
///
/// # Examples
///
/// ### To add an App Id
/// ```
/// let session_state = SessionState::default();
/// session_state("1234-1234".into(), "SomeCoolAppId".into());
/// ```

#[derive(Debug, Clone, Default)]
pub struct SessionState {
    session_map: Arc<RwLock<HashMap<String, Session>>>,
    account_session: Arc<RwLock<Option<AccountSession>>>,
}

impl SessionState {
    pub fn insert_account_session(&self, account_session: AccountSession) {
        let mut session_state = self.account_session.write().unwrap();
        let _ = session_state.insert(account_session);
    }

    pub fn get_account_session(&self) -> Option<AccountSession> {
        let session_state = self.account_session.read().unwrap();
        if let Some(session) = session_state.clone() {
            return Some(session);
        }

        None
    }

    pub fn get_app_id(&self, session_id: String) -> Option<String> {
        let session_map = self.session_map.read().unwrap();
        if let Some(session) = session_map.get(&session_id) {
            return Some(session.get_app_id());
        }
        None
    }

    pub fn has_session(&self, session_id: String) -> bool {
        self.session_map.read().unwrap().contains_key(&session_id)
    }

    pub fn add_session(&self, session_id: String, session: Session) {
        let mut session_state = self.session_map.write().unwrap();
        session_state.insert(session_id, session);
    }

    pub fn clear_session(&self, session_id: &str) {
        let mut session_state = self.session_map.write().unwrap();
        session_state.remove(session_id);
    }

    pub fn clean_account_session(&self) {
        let mut session_state = self.account_session.write().unwrap();
        let _ = session_state.take();
    }

    pub fn get_session(&self, session_id: &str) -> Option<Session> {
        let session_state = self.session_map.read().unwrap();
        session_state.get(session_id).cloned()
    }
}
