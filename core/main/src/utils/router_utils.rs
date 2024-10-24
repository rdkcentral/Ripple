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
        apps::EffectiveTransport,
        gateway::rpc_gateway_api::{ApiMessage, JsonRpcApiResponse, RpcRequest},
    },
    extn::extn_client_message::{ExtnMessage, ExtnResponse},
    log::{error, trace},
    serde_json::{self, Result as SResult},
    utils::error::RippleError,
};

use crate::state::{platform_state::PlatformState, session_state::Session};

pub async fn return_api_message_for_transport(
    session: Session,
    msg: ApiMessage,
    state: PlatformState,
) {
    match session.get_transport() {
        EffectiveTransport::Websocket => {
            if let Err(e) = session.send_json_rpc(msg).await {
                error!("Error while responding back message {:?}", e)
            }
        }
        EffectiveTransport::Bridge(container_id) => {
            if let Err(e) = state.send_to_bridge(container_id, msg).await {
                error!("Error sending event to bridge {:?}", e);
            }
        }
    }
}

pub fn return_extn_response(msg: ApiMessage, extn_msg: ExtnMessage) {
    let callback = match extn_msg.clone().callback {
        Some(cb) => cb,
        None => {
            error!("No valid callbacks");
            return;
        }
    };

    let r: SResult<JsonRpcApiResponse> = serde_json::from_str(&msg.jsonrpc_msg);

    if let Ok(resp) = r {
        let response_value = if let Some(result) = resp.result {
            result
        } else if let Some(error) = resp.error {
            error
        } else {
            serde_json::to_value(RippleError::InvalidOutput).unwrap()
        };

        let return_value = ExtnResponse::Value(response_value);
        if let Ok(response) = extn_msg.get_response(return_value) {
            if let Err(e) = callback.try_send(response.into()) {
                error!("Error while sending back rpc request for extn {:?}", e);
            }
        } else {
            error!("Not a Request object {:?}", extn_msg);
        }
    }
}

pub fn get_rpc_header_with_status(request: &RpcRequest, status_code: i32) -> String {
    format!(
        "{},{},{}",
        request.ctx.app_id, request.ctx.method, status_code
    )
}

pub fn get_rpc_header(request: &RpcRequest) -> String {
    format!("{},{}", request.ctx.app_id, request.ctx.method)
}

pub fn add_telemetry_status_code(original_ref: &str, status_code: &str) -> String {
    format!("{},{}", original_ref, status_code)
}

pub fn capture_stage(request: &mut RpcRequest, stage: &str) {
    let duration = request.stats.update_stage(stage);
    trace!(
        "Firebolt processing stage: {},{},{},{}",
        request.ctx.app_id,
        request.ctx.method,
        stage,
        duration
    )
}
