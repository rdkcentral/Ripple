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

use crate::{
    api::gateway::rpc_gateway_api::{CallContext, RpcRequest},
    log::{debug, error, trace},
    processor::rpc_router::{RouterState, RpcRouter},
    service::service_message::{JsonRpcMessage, ServiceMessage},
    utils::error::RippleError,
};
use tokio::sync::mpsc::Sender as MSender;

pub fn route_service_message(
    sender: &MSender<ServiceMessage>,
    state: &RouterState,
    sm: ServiceMessage,
) -> Result<(), RippleError> {
    trace!("Received Service Message: {:#?}", sm);
    match sm.message {
        JsonRpcMessage::Request(json_rpc_request) => {
            let ctx = sm.context.as_ref().map_or_else(CallContext::default, |v| {
                serde_json::from_value(v.clone()).unwrap_or_default()
            });
            let req: RpcRequest = RpcRequest {
                ctx: ctx.clone(),
                method: json_rpc_request.method,
                params_json: RpcRequest::prepend_ctx(json_rpc_request.params, &ctx.clone()),
            };

            let sender = sender.clone();
            let state_clone = state.clone();
            tokio::spawn(async move {
                let router_state = state_clone.clone();
                let resp = RpcRouter::resolve_route(req.clone(), &router_state).await;

                match resp {
                    Ok(msg) => {
                        debug!("Service Request resolved successfully: response {}", msg);

                        let mut msg: JsonRpcMessage = serde_json::from_str(&msg).unwrap();
                        msg.set_id(json_rpc_request.id.clone());

                        let sm_resp = ServiceMessage {
                            message: msg,
                            context: sm.context.clone(),
                        };
                        let _ = sender.try_send(sm_resp).map_err(|e| {
                            error!("Error sending service response: {:?}", e);
                            RippleError::InvalidInput
                        });
                    }
                    Err(e) => {
                        error!("Error resolving service route: {:?}", e);
                        let sm_resp = ServiceMessage::new_error(
                            -32603,
                            e.to_string(),
                            None,
                            json_rpc_request.id.clone(),
                        );
                        let _ = sender.try_send(sm_resp).map_err(|e| {
                            error!("Error sending service error response: {:?}", e);
                            RippleError::InvalidInput
                        });
                    }
                }
            });
        }
        JsonRpcMessage::Notification(_json_rpc_notification) => {}
        JsonRpcMessage::Success(_json_rpc_success) => {}
        JsonRpcMessage::Error(json_rpc_error) => {
            error!("Received Service Error: {:?}", json_rpc_error);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::service_message::{Id, JsonRpcMessage, JsonRpcRequest, ServiceMessage};
    use serde_json::json;
    use tokio::sync::mpsc;

    fn dummy_router_state() -> RouterState {
        RouterState::default()
    }

    #[tokio::test]
    async fn test_route_service_message_request() {
        let (tx, mut _rx) = mpsc::channel(1);
        let state = dummy_router_state();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "test_method".to_string(),
            params: Some(json!({"foo": "bar"})),
            id: Id::Number(1),
        };
        let sm = ServiceMessage {
            message: JsonRpcMessage::Request(req),
            context: None,
        };
        let _ = route_service_message(&tx, &state, sm);
    }

    #[tokio::test]
    async fn test_route_service_message_notification() {
        let (tx, mut rx) = mpsc::channel(1);
        let state = dummy_router_state();
        let sm = ServiceMessage::new_notification("notify".to_string(), None);
        let res = route_service_message(&tx, &state, sm);
        assert!(res.is_ok());
        // No message should be sent for notification
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_route_service_message_success() {
        let (tx, mut rx) = mpsc::channel(1);
        let state = dummy_router_state();
        let sm = ServiceMessage::new_success(json!({"ok": true}), Id::Number(2));
        let res = route_service_message(&tx, &state, sm);
        assert!(res.is_ok());
        // No message should be sent for success
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_route_service_message_error() {
        let (tx, mut rx) = mpsc::channel(1);
        let state = dummy_router_state();
        let sm = ServiceMessage::new_error(-1, "fail".to_string(), None, Id::Number(3));
        let res = route_service_message(&tx, &state, sm);
        assert!(res.is_ok());
        // No message should be sent for error
        assert!(rx.try_recv().is_err());
    }
}
