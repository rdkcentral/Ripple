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

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use ripple_sdk::{
    api::{
        gateway::rpc_gateway_api::ApiMessage, session::RippleSession,
    },
    tokio::sync::mpsc::Sender,
    utils::error::RippleError,
};

#[derive(Debug, Clone)]
pub struct SessionData {
    app_id: String,
}

#[derive(Debug, Clone)]
pub struct Session {
    transport: Sender<ApiMessage>,
    data: SessionData,
}

impl Session {
    pub fn new(app_id: String, transport: Sender<ApiMessage>) -> Session {
        Session {
            transport,
            data: SessionData { app_id },
        }
    }

    pub async fn send(&self, msg: ApiMessage) -> Result<(), RippleError> {
        if let Ok(_) = self.transport.send(msg).await {
            return Ok(());
        } else {
            return Err(RippleError::SendFailure);
        }
    }

    fn get_app_id(&self) -> String {
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
    ripple_session: Arc<RwLock<Option<RippleSession>>>
}

impl SessionState {
    pub fn insert_ripple_session(&self, ripple_session: RippleSession) {
        let mut session_state = self.ripple_session.write().unwrap();
        let _ = session_state.insert(ripple_session);
    }

    pub fn get_ripple_session(&self) -> Option<RippleSession> {
        let session_state = self.ripple_session.read().unwrap();
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

    pub fn get_sender(&self, session_id: String) -> Option<Session> {
        let session_map = self.session_map.read().unwrap();
        session_map.get(&session_id).cloned()
    }

    pub fn add_session(&self, session_id: String, session: Session) {
        let mut session_state = self.session_map.write().unwrap();
        session_state.insert(session_id, session);
    }

    pub fn clear_session(&self, session_id: String) {
        let mut session_state = self.session_map.write().unwrap();
        session_state.remove(&session_id);
    }

    pub async fn send_message(
        &self,
        session_id: String,
        msg: ApiMessage,
    ) -> Result<(), RippleError> {
        let session_state = self.session_map.read().unwrap();
        if let Some(session) = session_state.get(&session_id) {
            return session.send(msg).await;
        }
        Err(RippleError::SenderMissing)
    }
}
