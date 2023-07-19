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
        firebolt::fb_metrics::{
            BehavioralMetricContext, BehavioralMetricPayload, BehavioralMetricRequest,
        },
        gateway::rpc_gateway_api::CallContext,
    },
    extn::extn_client_message::ExtnResponse,
    framework::RippleResponse,
};

use crate::state::platform_state::PlatformState;

pub async fn send_metric(
    platform_state: &PlatformState,
    payload: BehavioralMetricPayload,
    _ctx: &CallContext,
) -> RippleResponse {
    // TODO use _ctx for any governance stuff
    let session = platform_state.session_state.get_account_session();
    if let Some(session) = session {
        let request = BehavioralMetricRequest {
            context: Some(platform_state.metrics.get_context()),
            payload,
            session,
        };

        if let Ok(resp) = platform_state.get_client().send_extn_request(request).await {
            if let Some(ExtnResponse::Boolean(b)) = resp.payload.extract() {
                if b {
                    return Ok(());
                }
            }
        }
    }
    Err(ripple_sdk::utils::error::RippleError::ProcessorError)
}

pub fn get_app_context(
    _platform_state: &PlatformState,
    ctx: &CallContext,
) -> BehavioralMetricContext {
    // TODO: Add logic for getting initial and loaded session from app manager and add governance
    ctx.clone().into()
}
