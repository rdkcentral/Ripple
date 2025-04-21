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
        firebolt::fb_capabilities::FireboltPermission,
        gateway::rpc_gateway_api::{ApiMessage, ApiProtocol, RpcRequest},
        observability::log_signal::LogSignal,
    },
    log::{debug, error},
    tokio,
};

use serde_json::json;

use crate::state::session_state::Session;

use super::endpoint_broker::BrokerRequest;

#[derive(Debug, Clone, Default)]
pub struct ProvideBrokerState {
    capability_map: Arc<RwLock<HashMap<String, Session>>>,
}

pub enum ProviderResult {
    Session(Session),
    Registered,
    NotAvailable(String),
}

impl ProvideBrokerState {
    pub fn check_provider_request(
        &self,
        request: &RpcRequest,
        permission: &Vec<FireboltPermission>,
        session: Option<Session>,
    ) -> Option<ProviderResult> {
        debug!("Method {}", request.method);
        if request.method.contains(".provide") {
            debug!("inside method before session");
            if let Some(s) = session {
                debug!("inside session before permission {:?}", permission);
                if let Some(p) = Self::get_permission(permission) {
                    {
                        debug!("adding permission {}", p);
                        let mut cap_map = self.capability_map.write().unwrap();
                        let _ = cap_map.insert(p.clone(), s.clone());
                        let _ = cap_map
                            .insert(format!("{}.{}", p, request.ctx.app_id.clone()), s.clone());
                    }
                    debug!("return registered");
                    return Some(ProviderResult::Registered);
                }
            }
        } else if let Some(p) = Self::get_permission(permission) {
            debug!("Checking session  {}", p);
            if let Some(session) = { self.capability_map.read().unwrap().get(&p).cloned() } {
                debug!("Returning session");
                return Some(ProviderResult::Session(session));
            }
            return Some(ProviderResult::NotAvailable(p));
        }

        None
    }

    fn get_permission(permission: &[FireboltPermission]) -> Option<String> {
        if !permission.is_empty() {
            if let Some(p) = permission.first() {
                return Some(p.cap.as_str());
            }
        }
        None
    }
    pub fn format_provider_message(rpc_request: &RpcRequest, id: u64) -> ApiMessage {
        let r = if let Some(p) = rpc_request.get_params() {
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": rpc_request.ctx.method,
                "params": p
            })
        } else {
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": rpc_request.ctx.method
            })
        };
        ApiMessage::new(
            ApiProtocol::JsonRpc,
            serde_json::to_string(&r).unwrap(),
            "".into(),
        )
    }

    pub fn send_to_provider(request: BrokerRequest, id: u64, session: Session) {
        tokio::spawn(async move {
            LogSignal::new(
                "provider_broker_state".into(),
                "sending to provider".into(),
                request.clone(),
            )
            .emit_debug();
            if let Err(e) = session
                .send_json_rpc(Self::format_provider_message(&request.rpc, id))
                .await
            {
                LogSignal::new(
                    "provider_broker_state".into(),
                    "error sending to provider".into(),
                    request.clone(),
                )
                .with_diagnostic_context_item("err", &format!("{:?}", e))
                .emit_debug();
                error!("Couldnt send Provider request {:?}", e)
            }
        });
    }
}
