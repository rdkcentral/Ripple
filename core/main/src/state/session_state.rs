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
        apps::AppSession,
        gateway::rpc_gateway_api::{ApiMessage, CallContext},
        session::{AccountSession, ProvisionRequest},
    },
    log::debug,
    tokio::sync::mpsc::Sender,
    utils::error::RippleError,
};

#[derive(Debug, Clone)]
pub struct SessionData {
    app_id: String,
}

#[derive(Debug, Clone)]
pub struct Session {
    sender: Option<Sender<ApiMessage>>,
    data: SessionData,
}

impl Session {
    pub fn new(app_id: String, sender: Option<Sender<ApiMessage>>) -> Session {
        Session {
            sender,
            data: SessionData { app_id },
        }
    }

    pub fn get_sender(&self) -> Option<Sender<ApiMessage>> {
        self.sender.clone()
    }

    pub async fn send_json_rpc(&self, msg: ApiMessage) -> Result<(), RippleError> {
        if let Some(sender) = self.get_sender() {
            if sender.send(msg).await.is_ok() {
                return Ok(());
            }
        }
        Err(RippleError::SendFailure)
    }

    pub fn get_app_id(&self) -> String {
        self.data.app_id.clone()
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
    pending_sessions: Arc<RwLock<HashMap<String, Option<PendingSessionInfo>>>>,
}

#[derive(Debug, Clone, Default)]
pub struct PendingSessionInfo {
    pub session: AppSession,
    pub loading: bool,
    pub session_id: Option<String>,
    pub loaded_session_id: Option<String>,
}

impl SessionState {
    pub fn insert_session_token(&self, token: String) {
        let mut session_state = self.account_session.write().unwrap();
        let account_session = session_state.take();
        if let Some(mut session) = account_session {
            session.token = token;
            let _ = session_state.insert(session);
        }
    }

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

    pub fn has_session(&self, ctx: &CallContext) -> bool {
        self.session_map.read().unwrap().contains_key(&ctx.get_id())
    }

    pub fn add_session(&self, id: String, session: Session) {
        let mut session_state = self.session_map.write().unwrap();
        debug!(
            "add_session capacity: {}: num sessions: {}",
            session_state.capacity(),
            session_state.len()
        );
        session_state.insert(id, session);
        session_state.shrink_to_fit();
    }

    pub fn clear_session(&self, id: &str) {
        let mut session_state = self.session_map.write().unwrap();
        session_state.remove(id);
    }

    pub fn update_account_session(&self, provision: ProvisionRequest) {
        let mut session_state = self.account_session.write().unwrap();
        let account_session = session_state.take();
        if let Some(mut session) = account_session {
            session.device_id = provision.device_id;
            session.account_id = provision.account_id;
            if let Some(id) = provision.distributor_id {
                session.id = id;
            }
            let _ = session_state.insert(session);
        }
    }

    pub fn get_session(&self, ctx: &CallContext) -> Option<Session> {
        let session_state = self.session_map.read().unwrap();
        if let Some(cid) = &ctx.cid {
            session_state.get(cid).cloned()
        } else {
            session_state.get(&ctx.session_id).cloned()
        }
    }

    pub fn get_session_for_connection_id(&self, cid: &str) -> Option<Session> {
        let session_state = self.session_map.read().unwrap();
        session_state.get(cid).cloned()
    }

    pub fn add_pending_session(&self, app_id: String, info: Option<PendingSessionInfo>) {
        let mut pending_sessions = self.pending_sessions.write().unwrap();
        if info.is_none() && pending_sessions.get(&app_id).is_some() {
            return;
        }
        pending_sessions.insert(app_id, info);
        pending_sessions.shrink_to_fit();
    }

    pub fn clear_pending_session(&self, app_id: &String) {
        self.pending_sessions.write().unwrap().remove(app_id);
    }
}
