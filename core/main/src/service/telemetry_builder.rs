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
        firebolt::{
            fb_metrics::{ErrorParams, InternalInitializeParams, SystemErrorParams},
            fb_telemetry::{
                AppLoadStart, AppLoadStop, FireboltEvent, FireboltInteraction, InternalInitialize,
                TelemetryAppError, TelemetryPayload, TelemetrySignIn, TelemetrySignOut,
                TelemetrySystemError,
            },
        },
        gateway::rpc_gateway_api::{ApiMessage, CallContext, RpcRequest},
    },
    chrono::{DateTime, Utc},
    framework::RippleResponse,
    log::{error, trace},
};
use serde_json::Value;

use crate::state::platform_state::PlatformState;

pub struct TelemetryBuilder;
include!(concat!(env!("OUT_DIR"), "/version.rs"));

impl TelemetryBuilder {
    pub async fn send_app_load_start(
        ps: PlatformState,
        app_id: String,
        app_version: Option<String>,
        start_time: Option<DateTime<Utc>>,
    ) {
        if let Err(e) = Self::send_telemetry(
            ps.clone(),
            TelemetryPayload::AppLoadStart(AppLoadStart {
                app_id,
                app_version,
                start_time: start_time.unwrap_or_default().timestamp_millis(),
                ripple_session_id: ps.get_device_session_id().await,
                ripple_version: ps
                    .version
                    .clone()
                    .unwrap_or(String::from(SEMVER_LIGHTWEIGHT)),
                ripple_context: None,
            }),
        )
        .await
        {
            error!("send_telemetry={:?}", e)
        }
    }

    pub async fn send_app_load_stop(ps: PlatformState, app_id: String, success: bool) {
        if let Err(e) = Self::send_telemetry(
            ps.clone(),
            TelemetryPayload::AppLoadStop(AppLoadStop {
                app_id,
                stop_time: Utc::now().timestamp_millis(),
                app_session_id: None,
                ripple_session_id: ps.get_device_session_id().await,
                success,
            }),
        )
        .await
        {
            error!("send_telemetry={:?}", e)
        }
    }

    pub async fn update_session_id_and_send_telemetry(
        ps: PlatformState,
        mut t: TelemetryPayload,
    ) -> RippleResponse {
        let session_id = ps.get_device_session_id().await;
        t.update_session_id(session_id);
        Self::send_telemetry(ps.clone(), t).await
    }

    pub async fn send_telemetry(ps: PlatformState, t: TelemetryPayload) -> RippleResponse {
        trace!("send_telemetry: t={:?}", t);

        let client = ps.get_client().get_extn_client();
        let mut result = Ok(());
        for id in ps.get_listeners().await {
            if let Err(e) = client.send_event_with_id(&id, t.clone()) {
                error!("telemetry_send_error target={} event={:?}", id, t.clone());
                result = Err(e)
            }
        }
        result
    }

    pub async fn send_ripple_telemetry(ps: PlatformState) {
        Self::send_app_load_start(
            ps.clone(),
            "ripple".to_string(),
            Some(
                ps.version
                    .clone()
                    .unwrap_or(String::from(SEMVER_LIGHTWEIGHT)),
            ),
            Some(ps.get_metrics_start_time().await),
        )
        .await;
        Self::send_app_load_stop(ps.clone(), "ripple".to_string(), true).await;
    }

    pub async fn send_error(ps: PlatformState, app_id: String, error_params: ErrorParams) {
        let mut app_error: TelemetryAppError = error_params.into();
        app_error.ripple_session_id = ps.get_device_session_id().await;
        app_error.app_id = app_id;

        if let Err(e) = Self::send_telemetry(ps, TelemetryPayload::AppError(app_error)).await {
            error!("send_telemetry={:?}", e)
        }
    }

    pub async fn send_system_error(ps: PlatformState, error_params: SystemErrorParams) {
        let mut system_error: TelemetrySystemError = error_params.into();
        system_error.ripple_session_id = ps.get_device_session_id().await;

        if let Err(e) = Self::send_telemetry(ps, TelemetryPayload::SystemError(system_error)).await
        {
            error!("send_telemetry={:?}", e)
        }
    }

    pub async fn send_sign_in(ps: PlatformState, ctx: &CallContext) {
        if let Err(e) = Self::send_telemetry(
            ps.clone(),
            TelemetryPayload::SignIn(TelemetrySignIn {
                app_id: ctx.app_id.to_owned(),
                ripple_session_id: ps.get_device_session_id().await,
                app_session_id: Some(ctx.session_id.to_owned()),
            }),
        )
        .await
        {
            error!("send_telemetry={:?}", e)
        }
    }

    pub async fn send_sign_out(ps: PlatformState, ctx: &CallContext) {
        if let Err(e) = Self::send_telemetry(
            ps.clone(),
            TelemetryPayload::SignOut(TelemetrySignOut {
                app_id: ctx.app_id.to_owned(),
                ripple_session_id: ps.get_device_session_id().await,
                app_session_id: Some(ctx.session_id.to_owned()),
            }),
        )
        .await
        {
            error!("send_telemetry={:?}", e)
        }
    }

    pub async fn internal_initialize(
        ps: PlatformState,
        ctx: &CallContext,
        params: &InternalInitializeParams,
    ) {
        if let Err(e) = Self::send_telemetry(
            ps.clone(),
            TelemetryPayload::InternalInitialize(InternalInitialize {
                app_id: ctx.app_id.to_owned(),
                ripple_session_id: ps.get_device_session_id().await,
                app_session_id: Some(ctx.session_id.to_owned()),
                semantic_version: params.version.to_string(),
            }),
        )
        .await
        {
            error!("send_telemetry={:?}", e)
        }
    }

    pub async fn send_fb_tt(
        ps: PlatformState,
        req: RpcRequest,
        tt: i64,
        success: bool,
        resp: &ApiMessage,
    ) {
        let ctx = req.ctx;
        let method = req.method;
        let params = if let Ok(mut p) = serde_json::from_str::<Vec<Value>>(&req.params_json) {
            if p.len() > 1 {
                // remove call context
                let _ = p.remove(0);
                Some(serde_json::to_string(&p).unwrap())
            } else {
                None
            }
        } else {
            None
        };
        let response = serde_json::to_string(resp).unwrap_or_default();
        if let Err(e) = Self::send_telemetry(
            ps.clone(),
            TelemetryPayload::FireboltInteraction(FireboltInteraction {
                app_id: ctx.app_id.to_owned(),
                ripple_session_id: ps.get_device_session_id().await,
                app_session_id: Some(ctx.session_id),
                tt,
                method,
                params,
                success,
                response,
            }),
        )
        .await
        {
            error!("send_telemetry={:?}", e)
        }
    }

    pub async fn send_fb_event(ps: PlatformState, event: &str, result: Value) {
        if let Err(e) = Self::send_telemetry(
            ps.clone(),
            TelemetryPayload::FireboltEvent(FireboltEvent {
                event_name: event.into(),
                result,
            }),
        )
        .await
        {
            error!("send_fb_event: e={:?}", e)
        }
    }
}
