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
        distributor::distributor_privacy::DataEventType,
        firebolt::{
            fb_metrics::{
                AppDataGovernanceState, BehavioralMetricContext, BehavioralMetricPayload,
                BehavioralMetricRequest, MetricsPayload, MetricsRequest,
            },
            fb_telemetry::OperationalMetricRequest,
        },
        gateway::rpc_gateway_api::CallContext,
    },
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::ExtnMessage,
    },
    framework::RippleResponse,
    log::debug,
    tokio::sync::mpsc::{Receiver as MReceiver, Sender as MSender},
};

use crate::{
    service::{data_governance::DataGovernance, telemetry_builder::TelemetryBuilder},
    state::platform_state::PlatformState,
    SEMVER_LIGHTWEIGHT,
};

pub async fn send_metric(
    platform_state: &PlatformState,
    mut payload: BehavioralMetricPayload,
    ctx: &CallContext,
) -> RippleResponse {
    // TODO use _ctx for any governance stuff
    let drop_data = update_app_context(platform_state, ctx, &mut payload).await;
    /*
    not opted in, or configured out, do nothing
    */
    if drop_data {
        debug!("drop data is true, not sending BI metrics");
        return Ok(());
    }
    if let Some(session) = platform_state.session_state.get_account_session() {
        let request = BehavioralMetricRequest {
            context: Some(platform_state.metrics.get_context()),
            payload,
            session,
        };
        return platform_state
            .get_client()
            .send_extn_request_transient(request);
    }
    Err(ripple_sdk::utils::error::RippleError::ProcessorError)
}

pub async fn update_app_context(
    ps: &PlatformState,
    ctx: &CallContext,
    payload: &mut BehavioralMetricPayload,
) -> bool {
    let mut context: BehavioralMetricContext = ctx.clone().into();
    if let Some(app) = ps.app_manager_state.get(&ctx.app_id) {
        context.app_session_id = app.loaded_session_id.to_owned();
        context.app_user_session_id = app.active_session_id;
        context.app_version = ps
            .version
            .clone()
            .unwrap_or(String::from(SEMVER_LIGHTWEIGHT));
    }
    if let Some(session) = ps.session_state.get_account_session() {
        context.partner_id = session.id;
    }
    let (tags, drop_data) =
        DataGovernance::resolve_tags(ps, ctx.app_id.clone(), DataEventType::BusinessIntelligence)
            .await;
    let tag_name_set = tags.iter().map(|tag| tag.tag_name.clone()).collect();
    context.governance_state = Some(AppDataGovernanceState::new(tag_name_set));

    payload.update_context(context);

    match payload {
        BehavioralMetricPayload::Ready(_) => {
            TelemetryBuilder::send_app_load_stop(ps, ctx.app_id.clone(), true)
        }
        BehavioralMetricPayload::SignIn(_) => TelemetryBuilder::send_sign_in(ps, ctx),
        BehavioralMetricPayload::SignOut(_) => TelemetryBuilder::send_sign_out(ps, ctx),
        _ => {}
    }
    drop_data
}
pub async fn send_metric_for_app_state_change(
    ps: &PlatformState,
    mut payload: BehavioralMetricPayload,
    app_id: &str,
) -> RippleResponse {
    match payload {
        BehavioralMetricPayload::AppStateChange(_) | BehavioralMetricPayload::Error(_) => {
            let (tags, drop_data) = DataGovernance::resolve_tags(
                ps,
                app_id.to_string(),
                DataEventType::BusinessIntelligence,
            )
            .await;
            let tag_name_set = tags.iter().map(|tag| tag.tag_name.clone()).collect();

            if drop_data {
                debug!("drop data is true, not sending BI metrics");
                return Ok(());
            }

            let mut context: BehavioralMetricContext = payload.get_context();
            let session = ps.session_state.get_account_session();
            if let Some(session) = session {
                if let Some(app) = ps.app_manager_state.get(app_id) {
                    context.app_session_id = app.loaded_session_id.to_owned();
                    context.app_user_session_id = app.active_session_id;
                    context.app_version = ps
                        .version
                        .clone()
                        .unwrap_or(String::from(SEMVER_LIGHTWEIGHT));
                }
                context.governance_state = Some(AppDataGovernanceState::new(tag_name_set));
                context.partner_id = session.clone().id;
                payload.update_context(context);

                let request = BehavioralMetricRequest {
                    context: Some(ps.metrics.get_context()),
                    payload,
                    session,
                };
                return ps.get_client().send_extn_request_transient(request);
            }
            Err(ripple_sdk::utils::error::RippleError::ProcessorError)
        }
        _ => Ok(()),
    }
}
/// Supports processing of Metrics request from extensions and forwards the metrics accordingly.
#[derive(Debug)]
pub struct MetricsProcessor {
    state: PlatformState,
    streamer: DefaultExtnStreamer,
}

impl MetricsProcessor {
    pub fn new(state: PlatformState) -> MetricsProcessor {
        MetricsProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for MetricsProcessor {
    type STATE = PlatformState;
    type VALUE = MetricsRequest;
    fn get_state(&self) -> Self::STATE {
        self.state.clone()
    }

    fn sender(&self) -> MSender<ExtnMessage> {
        self.streamer.sender()
    }

    fn receiver(&mut self) -> MReceiver<ExtnMessage> {
        self.streamer.receiver()
    }
}

#[async_trait]
impl ExtnRequestProcessor for MetricsProcessor {
    fn get_client(&self) -> ripple_sdk::extn::client::extn_client::ExtnClient {
        self.state.get_client().get_extn_client()
    }

    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        let client = state.get_client().get_extn_client();
        match extracted_message.payload {
            MetricsPayload::BehaviorMetric(b, c) => {
                return match send_metric(&state, b, &c).await {
                    Ok(_) => Self::ack(client, msg).await.is_ok(),
                    Err(e) => Self::handle_error(client, msg, e).await,
                }
            }
            MetricsPayload::TelemetryPayload(t) => {
                TelemetryBuilder::update_session_id_and_send_telemetry(&state, t).is_ok()
            }
            MetricsPayload::OperationalMetric(_) => true,
        }
    }
}

/// Supports processing of Metrics request from extensions and forwards the metrics accordingly.
#[derive(Debug)]
pub struct OpMetricsProcessor {
    state: PlatformState,
    streamer: DefaultExtnStreamer,
}

impl OpMetricsProcessor {
    pub fn new(state: PlatformState) -> OpMetricsProcessor {
        OpMetricsProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for OpMetricsProcessor {
    type STATE = PlatformState;
    type VALUE = OperationalMetricRequest;
    fn get_state(&self) -> Self::STATE {
        self.state.clone()
    }

    fn sender(&self) -> MSender<ExtnMessage> {
        self.streamer.sender()
    }

    fn receiver(&mut self) -> MReceiver<ExtnMessage> {
        self.streamer.receiver()
    }
}

#[async_trait]
impl ExtnRequestProcessor for OpMetricsProcessor {
    fn get_client(&self) -> ripple_sdk::extn::client::extn_client::ExtnClient {
        self.state.get_client().get_extn_client()
    }

    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        let requestor = msg.requestor.to_string();
        match extracted_message {
            OperationalMetricRequest::Subscribe => state
                .metrics
                .operational_telemetry_listener(&requestor, true),
            OperationalMetricRequest::UnSubscribe => state
                .metrics
                .operational_telemetry_listener(&requestor, true),
            _ => (),
        }
        Self::ack(state.get_client().get_extn_client(), msg)
            .await
            .is_ok()
    }
}
