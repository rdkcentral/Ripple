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
    api::gateway::rpc_gateway_api::{ApiMessage, RpcRequest},
    extn::{client::extn_client::ExtnClient, extn_client_message::ExtnMessage},
    log::trace,
};

use crate::state::ops_metrics_state::OpMetricState;

pub fn return_extn_response(msg: ApiMessage, extn_msg: ExtnMessage, client: ExtnClient) {
    client.respond_with_api_message(extn_msg, msg);
}

pub fn get_rpc_header_with_status(request: &RpcRequest, status_code: i32) -> Option<String> {
    Some(format!(
        "{},{},{}",
        request.ctx.app_id, request.ctx.method, status_code
    ))
}

pub fn get_rpc_header(request: &RpcRequest) -> String {
    format!("{},{}", request.ctx.app_id, request.ctx.method)
}

pub fn add_telemetry_status_code(original_ref: &str, status_code: &str) -> Option<String> {
    Some(format!("{},{}", original_ref, status_code))
}

pub fn capture_stage(metrics_state: &OpMetricState, request: &RpcRequest, stage: &str) {
    let mut state = metrics_state.clone();
    let duration = state.update_api_stage(&request.ctx.request_id, stage);

    trace!(
        "Firebolt processing stage: {},{},{},{}",
        request.ctx.app_id,
        request.ctx.method,
        stage,
        duration
    )
}
