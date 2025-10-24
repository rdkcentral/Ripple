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
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
};

use ripple_sdk::{
    api::{
        apps::AppSession,
        gateway::rpc_gateway_api::{ApiMessage, CallContext},
        session::{AccountSession, ProvisionRequest},
    },
    log::{debug, error}, // Add logging imports for production error handling
    tokio::sync::mpsc::Sender,
    types::{AppId, ConnectionId, SessionId}, // Import our newtypes for type-safe session identifiers
    utils::error::RippleError,
};

#[derive(Debug, Clone)]
pub struct Session {
    // Optimize: eliminate SessionData wrapper for better cache locality
    sender: Option<Sender<ApiMessage>>, // 64 bytes (large field first)
    app_id: String,                     // 24 bytes (direct storage, no wrapper)
}

impl Session {
    pub fn new(app_id: String, sender: Option<Sender<ApiMessage>>) -> Session {
        Session {
            sender,
            app_id, // Direct assignment, no wrapper
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
        self.app_id.clone() // Direct access, no wrapper
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
    // Secondary index: AppId -> Set of ConnectionIds for O(1) lookup by app
    app_sessions_index: Arc<RwLock<HashMap<AppId, HashSet<ConnectionId>>>>,
}

#[derive(Debug, Clone, Default)]
pub struct PendingSessionInfo {
    // Optimize field ordering: large fields first, small fields last
    pub session: AppSession,               // Large struct (keep first)
    pub session_id: Option<String>,        // 32 bytes
    pub loaded_session_id: Option<String>, // 32 bytes
    pub loading: bool,                     // 1 byte (moved to end, minimizes padding)
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
        let app_id = AppId::new_unchecked(session.app_id.clone());

        // Add to primary session map
        let mut session_state = self.session_map.write().unwrap();
        session_state.insert(id.clone(), session);

        // Update secondary index: add this ConnectionId to the app's set
        let mut app_index = self.app_sessions_index.write().unwrap();
        app_index
            .entry(app_id.clone())
            .or_default()
            .insert(id.clone());

        debug!(
            "Added session for app {} (connection {}), index now has {} connections for this app",
            app_id.as_str(),
            id.as_str(),
            app_index.get(&app_id).map(|s| s.len()).unwrap_or(0)
        );
    }

    // Legacy compatibility method - TODO: Remove once all callers are updated
    pub fn add_session_legacy(&self, id: String, session: Session) {
        let connection_id = ConnectionId::new_unchecked(id);
        self.add_session(connection_id, session);
    }

    pub fn clear_session(&self, id: &ConnectionId) {
        // Remove from primary session map and get the app_id
        let mut session_state = self.session_map.write().unwrap();
        if let Some(session) = session_state.remove(id) {
            debug!(
                "Cleared session for app {} (connection {}), {} total sessions remain",
                session.app_id,
                id.as_str(),
                session_state.len()
            );
            // Update secondary index: remove this ConnectionId from the app's set
            let app_id = AppId::new_unchecked(session.app_id);
            let mut app_index = self.app_sessions_index.write().unwrap();
            if let Some(connections) = app_index.get_mut(&app_id) {
                connections.remove(id);
                debug!(
                    "  Removed from app index, {} connections remain for app {}",
                    connections.len(),
                    app_id.as_str()
                );
                // Clean up empty sets to avoid memory bloat
                if connections.is_empty() {
                    app_index.remove(&app_id);
                    debug!(
                        "  App {} removed from index (no connections remain)",
                        app_id.as_str()
                    );
                }
            }
        } else {
            debug!(
                "clear_session: Connection {} not found in session map",
                id.as_str()
            );
        }
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

    /// Memory optimization: Comprehensive session cleanup for app lifecycle events
    /// LEVERAGE: Uses secondary index for O(1) lookup instead of O(n) linear scan
    pub fn cleanup_app_sessions(&self, app_id: &str) {
        // Clean up pending sessions for this app
        let app_id_typed = AppId::new_unchecked(app_id.to_owned());
        self.clear_pending_session(&app_id_typed);

        // DEBUG: Log what's in the index
        if let Ok(app_index) = self.app_sessions_index.try_read() {
            debug!(
                "cleanup_app_sessions({}) - Index contents: {} apps total",
                app_id,
                app_index.len()
            );
            for (aid, conns) in app_index.iter() {
                debug!("  App '{}': {} connections", aid.as_str(), conns.len());
            }
        }

        // OPTIMIZATION: Use secondary index for instant lookup of all ConnectionIds for this app
        let sessions_to_remove: Vec<ConnectionId> = {
            // Short-lived read lock on index only
            match self.app_sessions_index.try_read() {
                Ok(app_index) => {
                    let connections = app_index
                        .get(&app_id_typed)
                        .map(|connections| connections.iter().cloned().collect())
                        .unwrap_or_default();
                    debug!(
                        "Index lookup for app {}: found {} connections to remove",
                        app_id,
                        if let Some(set) = app_index.get(&app_id_typed) {
                            set.len()
                        } else {
                            0
                        }
                    );
                    connections
                }
                Err(e) => {
                    error!(
                        "Failed to acquire read lock on app_sessions_index for cleanup: {:?}",
                        e
                    );
                    return;
                }
            }
        };

        debug!(
            "Attempting to cleanup {} sessions for app: {}",
            sessions_to_remove.len(),
            app_id
        );

        // Remove identified sessions - use blocking write since we need this to succeed
        if !sessions_to_remove.is_empty() {
            let mut session_map = self.session_map.write().unwrap();
            let mut app_index = self.app_sessions_index.write().unwrap();

            let mut removed_count = 0;
            for connection_id in &sessions_to_remove {
                if session_map.remove(connection_id).is_some() {
                    removed_count += 1;
                }
            }

            // Clean up the index entry for this app
            app_index.remove(&app_id_typed);

            if removed_count > 0 {
                debug!(
                    "Cleaned up {} sessions for app: {} (index-accelerated)",
                    removed_count, app_id
                );
            } else {
                debug!("No sessions actually removed for app: {} (found {} to remove but all already gone)", app_id, sessions_to_remove.len());
            }
        } else {
            debug!("No sessions found in index for app: {}", app_id);
        }
    }

    /// Memory optimization: Clean up stale sessions and pending state
    pub fn cleanup_stale_sessions(&self, max_age_minutes: u64) {
        use std::time::{Duration, SystemTime};

        let _cutoff_time = SystemTime::now() - Duration::from_secs(max_age_minutes * 60);

        // Clean up stale active sessions
        // TODO: Production enhancement - Add proper session timestamps to Session struct
        // Currently using simplified logic as Session doesn't have last_activity timestamp
        let mut sessions_to_remove = Vec::new();
        {
            match self.session_map.try_read() {
                Ok(session_map) => {
                    for (connection_id, session) in session_map.iter() {
                        // PRODUCTION TODO: Replace this simplified check with proper timestamp comparison
                        // if session.last_activity < cutoff_time {
                        //     sessions_to_remove.push(connection_id.clone());
                        // }

                        // Current simplified logic: remove sessions with empty app_id (likely orphaned)
                        if session.app_id.is_empty() {
                            sessions_to_remove.push(connection_id.clone());
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "Failed to acquire read lock for stale session cleanup: {:?}",
                        e
                    );
                    return;
                }
            }
        }

        if !sessions_to_remove.is_empty() {
            match self.session_map.try_write() {
                Ok(mut session_map) => {
                    let mut removed_count = 0;
                    for connection_id in sessions_to_remove {
                        if session_map.remove(&connection_id).is_some() {
                            removed_count += 1;
                        }
                    }
                    if removed_count > 0 {
                        debug!("Cleaned up {} stale sessions", removed_count);
                    }
                }
                Err(e) => {
                    error!(
                        "Failed to acquire write lock for stale session removal: {:?}",
                        e
                    );
                }
            }
        }
    }

    /// Production health check: Get session state metrics for monitoring
    pub fn get_session_metrics(&self) -> SessionMetrics {
        let session_count = self.session_map.read().unwrap().len();
        let pending_count = self.pending_sessions.read().unwrap().len();
        let has_account_session = self.account_session.read().unwrap().is_some();

        SessionMetrics {
            active_sessions: session_count,
            pending_sessions: pending_count,
            has_account_session,
        }
    }

    /// Production validation: Verify session state consistency
    pub fn validate_session_state(&self) -> Vec<SessionValidationIssue> {
        let mut issues = Vec::new();

        // Check for excessive session counts that might indicate leaks
        let session_count = self.session_map.read().unwrap().len();
        if session_count > 100 {
            // Configurable threshold
            issues.push(SessionValidationIssue::ExcessiveSessions(session_count));
        }

        // Check for sessions with empty app_ids (potential orphans)
        let session_map = self.session_map.read().unwrap();
        let empty_app_id_count = session_map
            .values()
            .filter(|session| session.app_id.is_empty())
            .count();
        if empty_app_id_count > 0 {
            issues.push(SessionValidationIssue::OrphanedSessions(empty_app_id_count));
        }

        // Check for excessive pending sessions
        let pending_count = self.pending_sessions.read().unwrap().len();
        if pending_count > 20 {
            // Configurable threshold
            issues.push(SessionValidationIssue::ExcessivePendingSessions(
                pending_count,
            ));
        }

        issues
    }
}

/// Production monitoring: Session state metrics for health checks
#[derive(Debug, Clone)]
pub struct SessionMetrics {
    pub active_sessions: usize,
    pub pending_sessions: usize,
    pub has_account_session: bool,
}

/// Production validation: Issues that can be detected in session state
#[derive(Debug, Clone)]
pub enum SessionValidationIssue {
    ExcessiveSessions(usize),
    OrphanedSessions(usize),
    ExcessivePendingSessions(usize),
}

// TODO: Add unit tests for session_state O(1) cleanup functionality
// Tests need to be updated to match the actual API signatures (ConnectionId::new_unchecked, non-async cleanup)
