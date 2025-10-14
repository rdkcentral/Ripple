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
    tokio::sync::mpsc::Sender,
    types::{AppId, ConnectionId, SessionId}, // Import our newtypes for type-safe session identifiers
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
    session_map: Arc<RwLock<HashMap<ConnectionId, Session>>>,
    account_session: Arc<RwLock<Option<AccountSession>>>,
    pending_sessions: Arc<RwLock<HashMap<AppId, Option<PendingSessionInfo>>>>,
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

    pub fn get_app_id(&self, session_id: &SessionId) -> Option<String> {
        let session_map = self.session_map.read().unwrap();
        // Convert SessionId to ConnectionId - in the context of this codebase, they often use the same value
        let connection_id = ConnectionId::new_unchecked(session_id.as_str().to_string());
        if let Some(session) = session_map.get(&connection_id) {
            return Some(session.get_app_id());
        }
        None
    }

    pub fn get_app_id_for_connection(&self, connection_id: &ConnectionId) -> Option<String> {
        let session_map = self.session_map.read().unwrap();
        if let Some(session) = session_map.get(connection_id) {
            return Some(session.get_app_id());
        }
        None
    }

    pub fn has_session(&self, ctx: &CallContext) -> bool {
        let connection_id = ConnectionId::new_unchecked(ctx.get_id());
        self.session_map
            .read()
            .unwrap()
            .contains_key(&connection_id)
    }

    pub fn add_session(&self, id: ConnectionId, session: Session) {
        let mut session_state = self.session_map.write().unwrap();
        session_state.insert(id, session);
    }

    // Legacy compatibility method - TODO: Remove once all callers are updated
    pub fn add_session_legacy(&self, id: String, session: Session) {
        let connection_id = ConnectionId::new_unchecked(id);
        self.add_session(connection_id, session);
    }

    pub fn clear_session(&self, id: &ConnectionId) {
        let mut session_state = self.session_map.write().unwrap();
        session_state.remove(id);
    }

    // Legacy compatibility method - TODO: Remove once all callers are updated
    pub fn clear_session_legacy(&self, id: &str) {
        let connection_id = ConnectionId::new_unchecked(id.to_string());
        self.clear_session(&connection_id);
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
            let connection_id = ConnectionId::new_unchecked(cid.clone());
            session_state.get(&connection_id).cloned()
        } else {
            let connection_id = ConnectionId::new_unchecked(ctx.session_id.clone());
            session_state.get(&connection_id).cloned()
        }
    }

    pub fn get_session_for_connection_id(&self, cid: &ConnectionId) -> Option<Session> {
        let session_state = self.session_map.read().unwrap();
        session_state.get(cid).cloned()
    }

    // Legacy compatibility method - TODO: Remove once all callers are updated
    pub fn get_session_for_connection_id_legacy(&self, cid: &str) -> Option<Session> {
        let connection_id = ConnectionId::new_unchecked(cid.to_string());
        self.get_session_for_connection_id(&connection_id)
    }

    pub fn add_pending_session(&self, app_id: AppId, info: Option<PendingSessionInfo>) {
        let mut pending_sessions = self.pending_sessions.write().unwrap();
        if info.is_none() && pending_sessions.get(&app_id).is_some() {
            return;
        }
        pending_sessions.insert(app_id, info);
    }

    pub fn clear_pending_session(&self, app_id: &AppId) {
        self.pending_sessions.write().unwrap().remove(app_id);
    }

    // Legacy compatibility methods for pending sessions - TODO: Remove once all callers are updated
    pub fn add_pending_session_legacy(&self, app_id: String, info: Option<PendingSessionInfo>) {
        let app_id_typed = AppId::new_unchecked(app_id);
        self.add_pending_session(app_id_typed, info);
    }

    pub fn clear_pending_session_legacy(&self, app_id: &str) {
        let app_id_typed = AppId::new_unchecked(app_id.to_owned());
        self.clear_pending_session(&app_id_typed);
    }
}
