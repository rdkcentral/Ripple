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

use jsonrpsee::{
    core::{Error, RpcResult},
    types::error::CallError,
};
use ripple_sdk::{
    api::{
        firebolt::fb_general::{ListenRequest, ListenerResponse},
        gateway::rpc_gateway_api::CallContext,
    },
    tokio::sync::oneshot,
};

use crate::{
    service::apps::app_events::{AppEventDecorator, AppEvents},
    state::platform_state::PlatformState,
};

pub use ripple_sdk::utils::rpc_utils::rpc_err;

pub const FIRE_BOLT_DEEPLINK_ERROR_CODE: i32 = -40400;
pub const DOWNSTREAM_SERVICE_UNAVAILABLE_ERROR_CODE: i32 = -50200;
pub const SESSION_NO_INTENT_ERROR_CODE: i32 = -40000;

/// Awaits a oneshot to respond. If the oneshot fails to repond, creates a generic
/// RPC internal error
pub async fn rpc_await_oneshot<T>(rx: oneshot::Receiver<T>) -> RpcResult<T> {
    match rx.await {
        Ok(v) => Ok(v),
        Err(e) => Err(jsonrpsee::core::error::Error::Custom(format!(
            "Internal failure: {:?}",
            e
        ))),
    }
}

/// listener for events any events.
pub async fn rpc_add_event_listener(
    state: &PlatformState,
    ctx: CallContext,
    request: ListenRequest,
    event_name: &'static str,
) -> RpcResult<ListenerResponse> {
    let listen = request.listen;

    AppEvents::add_listener(state, event_name.to_string(), ctx, request);
    Ok(ListenerResponse {
        listening: listen,
        event: event_name.into(),
    })
}

/// listener for events any events.
pub async fn rpc_add_event_listener_with_decorator(
    state: &PlatformState,
    ctx: CallContext,
    request: ListenRequest,
    event_name: &'static str,
    decorator: Option<Box<dyn AppEventDecorator + Send + Sync>>,
) -> RpcResult<ListenerResponse> {
    let listen = request.listen;

    AppEvents::add_listener_with_decorator(state, event_name.to_string(), ctx, request, decorator);
    Ok(ListenerResponse {
        listening: listen,
        event: event_name.into(),
    })
}

pub fn rpc_downstream_service_err(msg: &str) -> jsonrpsee::core::error::Error {
    Error::Call(CallError::Custom {
        code: DOWNSTREAM_SERVICE_UNAVAILABLE_ERROR_CODE,
        message: msg.to_owned(),
        data: None,
    })
}
pub fn rpc_session_no_intent_err(msg: &str) -> jsonrpsee::core::error::Error {
    Error::Call(CallError::Custom {
        code: SESSION_NO_INTENT_ERROR_CODE,
        message: msg.to_owned(),
        data: None,
    })
}
pub fn rpc_navigate_reserved_app_err(msg: &str) -> jsonrpsee::core::error::Error {
    Error::Call(CallError::Custom {
        code: FIRE_BOLT_DEEPLINK_ERROR_CODE,
        message: msg.to_owned(),
        data: None,
    })
}
