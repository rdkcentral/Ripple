use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use ripple_sdk::{
    api::gateway::rpc_gateway_api::ApiMessage, tokio::sync::mpsc::Sender, utils::error::RippleError,
};

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
    session_map: Arc<RwLock<HashMap<String, Sender<ApiMessage>>>>,
    app_id_session_map: Arc<RwLock<HashMap<String, String>>>,
}

impl SessionState {
    pub fn add_app_id(&self, session_id: String, app_id: String) {
        let mut app_id_state = self.app_id_session_map.write().unwrap();
        app_id_state.insert(session_id, app_id);
    }

    pub fn get_app_id(&self, session_id: String) -> Option<String> {
        let app_id_state = self.app_id_session_map.read().unwrap();
        app_id_state.get(&session_id).cloned()
    }

    pub fn has_sender(&self, session_id: String) -> bool {
        self.session_map.read().unwrap().contains_key(&session_id)
    }

    pub fn get_sender(&self, session_id: String) -> Option<Sender<ApiMessage>> {
        let session_map = self.session_map.read().unwrap();
        let v = session_map.get(&session_id);
        if v.is_some() {
            return Some(v.unwrap().clone());
        }
        None
    }

    pub fn add_session(&self, session_id: String, sender: Sender<ApiMessage>) {
        let mut session_state = self.session_map.write().unwrap();
        session_state.insert(session_id, sender);
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
        if let Some(sender) = session_state.get(&session_id) {
            if let Ok(_) = sender.send(msg).await {
                return Ok(());
            } else {
                return Err(RippleError::SendFailure);
            }
        }
        Err(RippleError::SenderMissing)
    }
}
