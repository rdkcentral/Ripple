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
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    types::error::CallError,
    RpcModule,
};
use std::collections::HashMap;

use ripple_sdk::{
    api::{
        firebolt::{
            fb_capabilities::JSON_RPC_STANDARD_ERROR_INVALID_PARAMS,
            fb_metrics::{
                self, hashmap_to_param_vec, Action, BehavioralMetricContext,
                BehavioralMetricPayload, CategoryType, ErrorType, InternalInitializeParams,
                InternalInitializeResponse, MediaEnded, MediaLoadStart, MediaPause, MediaPlay,
                MediaPlaying, MediaPositionType, MediaProgress, MediaRateChanged,
                MediaRenditionChanged, MediaSeeked, MediaSeeking, MediaWaiting, MetricsError, Page,
                Param, SignIn, SignOut, StartContent, StopContent, Version,
            },
            fb_telemetry::{self},
        },
        gateway::rpc_gateway_api::CallContext,
    },
    log::trace,
};

use serde::Deserialize;

use crate::{
    firebolt::rpc::RippleRPCProvider, processor::metrics_processor::send_metric,
    state::platform_state::PlatformState, utils::rpc_utils::rpc_err,
};

use ripple_sdk::api::firebolt::fb_metrics::SemanticVersion;

//const LAUNCH_COMPLETED_SEGMENT: &'static str = "LAUNCH_COMPLETED";

#[derive(Deserialize, Debug)]
pub struct PageParams {
    #[serde(rename = "pageId")]
    pub page_id: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ActionParams {
    pub category: CategoryType,
    #[serde(rename = "type")]
    pub action_type: String,
    pub parameters: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct StartContentParams {
    #[serde(rename = "entityId")]
    pub entity_id: Option<String>,
}
#[derive(Deserialize, Debug, Clone)]
pub struct StopContentParams {
    #[serde(rename = "entityId")]
    pub entity_id: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ErrorParams {
    #[serde(rename = "type")]
    pub error_type: ErrorType,
    pub code: String,
    pub description: String,
    pub visible: bool,
    pub parameters: Option<Vec<Param>>,
}
// fn validate_metrics_action_type(metrics_action: &str) -> RpcResult<bool> {
//     match metrics_action.len() {
//         1..=256 => Ok(true),
//         _ => {
//             return Err(jsonrpsee::core::Error::Call(CallError::Custom {
//                 code: JSON_RPC_STANDARD_ERROR_INVALID_PARAMS,
//                 message: "metrics.action.action_type out of range".to_string(),
//                 data: None,
//             }))
//         }
//     }
// }
pub const ERROR_MEDIA_POSITION_OUT_OF_RANGE: &str = "absolute media position out of range";
pub const ERROR_BAD_ABSOLUTE_MEDIA_POSITION: &str =
    "absolute media position must not contain any numbers to the right of the decimal point.";
/*
implement this: https://developer.comcast.com/firebolt-apis/core-sdk/v0.9.0/metrics#mediaposition
*/
fn convert_to_media_position_type(media_position: Option<f32>) -> RpcResult<MediaPositionType> {
    match media_position {
        Some(position) => {
            if (0.0..=0.999).contains(&position) {
                Ok(MediaPositionType::PercentageProgress(position))
            } else {
                if position.fract() != 0.0 {
                    return Err(jsonrpsee::core::Error::Call(CallError::Custom {
                        code: JSON_RPC_STANDARD_ERROR_INVALID_PARAMS,
                        message: ERROR_BAD_ABSOLUTE_MEDIA_POSITION.to_string(),
                        data: None,
                    }));
                };
                let abs_position = position.round() as i32;

                if (1..=86400).contains(&abs_position) {
                    Ok(MediaPositionType::AbsolutePosition(abs_position))
                } else {
                    Err(jsonrpsee::core::Error::Call(CallError::Custom {
                        code: JSON_RPC_STANDARD_ERROR_INVALID_PARAMS,
                        message: ERROR_MEDIA_POSITION_OUT_OF_RANGE.to_string(),
                        data: None,
                    }))
                }
            }
        }
        None => Ok(MediaPositionType::None),
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct MediaLoadStartParams {
    #[serde(rename = "entityId")]
    pub entity_id: String,
}
#[derive(Deserialize, Debug, Clone)]
pub struct MediaPlayParams {
    #[serde(rename = "entityId")]
    pub entity_id: String,
}
#[derive(Deserialize, Debug, Clone)]
pub struct MediaPlayingParams {
    #[serde(rename = "entityId")]
    pub entity_id: String,
}
#[derive(Deserialize, Debug, Clone)]
pub struct MediaPauseParams {
    #[serde(rename = "entityId")]
    pub entity_id: String,
}
#[derive(Deserialize, Debug, Clone)]
pub struct MediaWaitingParams {
    #[serde(rename = "entityId")]
    pub entity_id: String,
}
#[derive(Deserialize, Debug, Clone)]
pub struct MediaProgressParams {
    #[serde(rename = "entityId")]
    pub entity_id: String,
    pub progress: Option<f32>,
}
#[derive(Deserialize, Debug, Clone)]
pub struct MediaSeekingParams {
    #[serde(rename = "entityId")]
    pub entity_id: String,
    pub target: Option<f32>,
}
#[derive(Deserialize, Debug, Clone)]
pub struct MediaSeekedParams {
    #[serde(rename = "entityId")]
    pub entity_id: String,
    pub position: Option<f32>,
}
#[derive(Deserialize, Debug, Clone)]
pub struct MediaRateChangeParams {
    #[serde(rename = "entityId")]
    pub entity_id: String,
    pub rate: u32,
}
#[derive(Deserialize, Debug, Clone)]
pub struct MediaRenditionChangeParams {
    #[serde(rename = "entityId")]
    pub entity_id: String,
    pub bitrate: u32,
    pub width: u32,
    pub height: u32,
    pub profile: Option<String>,
}
#[derive(Deserialize, Debug, Clone)]
pub struct MediaEndedParams {
    #[serde(rename = "entityId")]
    pub entity_id: String,
}
//https://developer.comcast.com/firebolt/core/sdk/latest/api/metrics
#[rpc(server)]
pub trait Metrics {
    #[method(name = "metrics.startContent")]
    async fn start_content(
        &self,
        ctx: CallContext,
        page_params: StartContentParams,
    ) -> RpcResult<bool>;
    #[method(name = "metrics.stopContent")]
    async fn stop_content(
        &self,
        ctx: CallContext,
        stop_content_params: StopContentParams,
    ) -> RpcResult<bool>;
    #[method(name = "metrics.page")]
    async fn page(&self, ctx: CallContext, page_params: PageParams) -> RpcResult<bool>;
    #[method(name = "metrics.action")]
    async fn action(&self, ctx: CallContext, action_params: ActionParams) -> RpcResult<bool>;
    #[method(name = "metrics.error")]
    async fn error(&self, ctx: CallContext, error_params: ErrorParams) -> RpcResult<bool>;
    #[method(name = "metrics.ready")]
    async fn ready(&self, ctx: CallContext) -> RpcResult<bool>;
    #[method(name = "metrics.signin", aliases=["metrics.signIn"])]
    async fn sign_in(&self, ctx: CallContext) -> RpcResult<bool>;
    #[method(name = "metrics.signout", aliases=["metrics.signOut"])]
    async fn sign_out(&self, ctx: CallContext) -> RpcResult<bool>;
    #[method(name = "internal.initialize")]
    async fn internal_initialize(
        &self,
        ctx: CallContext,
        internal_initialize_params: InternalInitializeParams,
    ) -> RpcResult<InternalInitializeResponse>;
    #[method(name = "metrics.mediaLoadStart")]
    async fn media_load_start(
        &self,
        ctx: CallContext,
        media_load_start_params: MediaLoadStartParams,
    ) -> RpcResult<bool>;
    #[method(name = "metrics.mediaPlay")]
    async fn media_play(
        &self,
        ctx: CallContext,
        media_play_params: MediaPlayParams,
    ) -> RpcResult<bool>;
    #[method(name = "metrics.mediaPlaying")]
    async fn media_playing(
        &self,
        ctx: CallContext,
        media_playing_params: MediaPlayingParams,
    ) -> RpcResult<bool>;
    #[method(name = "metrics.mediaPause")]
    async fn media_pause(
        &self,
        ctx: CallContext,
        media_pause_params: MediaPauseParams,
    ) -> RpcResult<bool>;
    #[method(name = "metrics.mediaWaiting")]
    async fn media_waiting(
        &self,
        ctx: CallContext,
        media_waiting_params: MediaWaitingParams,
    ) -> RpcResult<bool>;
    #[method(name = "metrics.mediaProgress")]
    async fn media_progress(
        &self,
        ctx: CallContext,
        media_progress_params: MediaProgressParams,
    ) -> RpcResult<bool>;
    #[method(name = "metrics.mediaSeeking")]
    async fn media_seeking(
        &self,
        ctx: CallContext,
        media_seeking_params: MediaSeekingParams,
    ) -> RpcResult<bool>;
    #[method(name = "metrics.mediaSeeked")]
    async fn media_seeked(
        &self,
        ctx: CallContext,
        media_seeked_params: MediaSeekedParams,
    ) -> RpcResult<bool>;
    #[method(name = "metrics.mediaRateChange")]
    async fn media_rate_change(
        &self,
        ctx: CallContext,
        media_rate_changed_params: MediaRateChangeParams,
    ) -> RpcResult<bool>;
    #[method(name = "metrics.mediaRenditionChange")]
    async fn media_rendition_change(
        &self,
        ctx: CallContext,
        media_rendition_change_params: MediaRenditionChangeParams,
    ) -> RpcResult<bool>;
    #[method(name = "metrics.mediaEnded")]
    async fn media_ended(
        &self,
        ctx: CallContext,
        media_ended_params: MediaEndedParams,
    ) -> RpcResult<bool>;
}

#[derive(Debug, Clone)]
pub struct MetricsImpl {
    state: PlatformState,
}

impl From<ActionParams> for CategoryType {
    fn from(action_params: ActionParams) -> Self {
        action_params.category
    }
}

impl From<ErrorParams> for ErrorType {
    fn from(params: ErrorParams) -> Self {
        params.error_type
    }
}

#[async_trait]
impl MetricsServer for MetricsImpl {
    async fn start_content(
        &self,
        ctx: CallContext,
        page_params: StartContentParams,
    ) -> RpcResult<bool> {
        let start_content = BehavioralMetricPayload::StartContent(StartContent {
            context: ctx.clone().into(),
            entity_id: page_params.entity_id,
        });

        trace!("metrics.startContent={:?}", start_content);
        match send_metric(&self.state, start_content, &ctx).await {
            Ok(_) => Ok(true),
            Err(_) => Err(rpc_err("parse error")),
        }
    }

    async fn stop_content(
        &self,
        ctx: CallContext,
        stop_content_params: StopContentParams,
    ) -> RpcResult<bool> {
        let stop_content = BehavioralMetricPayload::StopContent(StopContent {
            context: ctx.clone().into(),
            entity_id: stop_content_params.entity_id,
        });
        trace!("metrics.stopContent={:?}", stop_content);
        match send_metric(&self.state, stop_content, &ctx).await {
            Ok(_) => Ok(true),
            Err(_) => Err(rpc_err("parse error")),
        }
    }
    async fn page(&self, ctx: CallContext, page_params: PageParams) -> RpcResult<bool> {
        let page = BehavioralMetricPayload::Page(Page {
            context: ctx.clone().into(),
            page_id: page_params.page_id,
        });
        trace!("metrics.page={:?}", page);
        match send_metric(&self.state, page, &ctx).await {
            Ok(_) => Ok(true),
            Err(_) => Err(rpc_err("parse error")),
        }
    }
    async fn action(&self, ctx: CallContext, action_params: ActionParams) -> RpcResult<bool> {
        //call it and let it blow up
        //let _ = validate_metrics_action_type(&action_params.action_type)?;
        let p_type = action_params.clone();

        let action = BehavioralMetricPayload::Action(Action {
            context: ctx.clone().into(),
            category: action_params.into(),
            parameters: hashmap_to_param_vec(p_type.parameters),
            _type: p_type.action_type,
        });
        trace!("metrics.action={:?}", action);

        match send_metric(&self.state, action, &ctx).await {
            Ok(_) => Ok(true),
            Err(_) => Err(rpc_err("parse error")),
        }
    }
    async fn ready(&self, ctx: CallContext) -> RpcResult<bool> {
        let data = BehavioralMetricPayload::Ready(fb_metrics::Ready {
            context: BehavioralMetricContext::from(ctx.clone()),
            ttmu_ms: 12,
        });
        trace!("metrics.action = {:?}", data);
        match send_metric(&self.state, data, &ctx).await {
            Ok(_) => Ok(true),
            Err(_) => Err(rpc_err("parse error")),
        }
    }

    async fn error(&self, ctx: CallContext, error_params: ErrorParams) -> RpcResult<bool> {
        let app_id = ctx.app_id.clone();
        let error_message = BehavioralMetricPayload::Error(MetricsError {
            context: ctx.clone().into(),
            error_type: error_params.clone().into(),
            code: error_params.code.clone(),
            description: error_params.description.clone(),
            visible: error_params.visible,
            parameters: error_params.parameters.clone(),
            durable_app_id: app_id.clone(),
            third_party_error: true,
        });
        trace!("metrics.error={:?}", error_message);
        match send_metric(&self.state, error_message, &ctx).await {
            Ok(_) => Ok(true),
            Err(_) => Err(rpc_err("parse error")),
        }
    }
    async fn sign_in(&self, ctx: CallContext) -> RpcResult<bool> {
        let data = BehavioralMetricPayload::SignIn(SignIn {
            context: ctx.clone().into(),
        });
        trace!("metrics.action = {:?}", data);
        Ok(send_metric(&self.state, data, &ctx).await.is_ok())
    }

    async fn sign_out(&self, ctx: CallContext) -> RpcResult<bool> {
        let data = BehavioralMetricPayload::SignOut(SignOut {
            context: ctx.clone().into(),
        });
        trace!("metrics.action = {:?}", data);
        Ok(send_metric(&self.state, data, &ctx).await.is_ok())
    }

    async fn internal_initialize(
        &self,
        ctx: CallContext,
        internal_initialize_params: InternalInitializeParams,
    ) -> RpcResult<InternalInitializeResponse> {
        let data = fb_telemetry::InternalInitialize {
            app_id: ctx.app_id,
            ripple_session_id: ctx.session_id.clone(),
            app_session_id: Some(ctx.session_id),
            semantic_version: internal_initialize_params.value.to_string(),
        };
        trace!("metrics.action = {:?}", data);
        let readable_result = internal_initialize_params
            .value
            .readable
            .replace("SDK", "FEE");
        let internal_initialize_resp = Version {
            major: internal_initialize_params.value.major,
            minor: internal_initialize_params.value.minor,
            patch: internal_initialize_params.value.patch,
            readable: readable_result,
        };
        Ok(InternalInitializeResponse {
            name: String::from("Default Result"),
            value: SemanticVersion {
                version: internal_initialize_resp,
            },
        })
    }
    async fn media_load_start(
        &self,
        ctx: CallContext,
        media_load_start_params: MediaLoadStartParams,
    ) -> RpcResult<bool> {
        let media_load_start_message = BehavioralMetricPayload::MediaLoadStart(MediaLoadStart {
            context: ctx.clone().into(),
            entity_id: media_load_start_params.entity_id,
        });
        trace!("metrics.media_load_start={:?}", media_load_start_message);
        match send_metric(&self.state, media_load_start_message, &ctx).await {
            Ok(_) => Ok(true),
            Err(_) => Err(rpc_err("parse error")),
        }
    }
    async fn media_play(
        &self,
        ctx: CallContext,
        media_play_params: MediaPlayParams,
    ) -> RpcResult<bool> {
        let media_play_message = BehavioralMetricPayload::MediaPlay(MediaPlay {
            context: ctx.clone().into(),
            entity_id: media_play_params.entity_id,
        });
        trace!("metrics.media_play={:?}", media_play_message);
        match send_metric(&self.state, media_play_message, &ctx).await {
            Ok(_) => Ok(true),
            Err(_) => Err(rpc_err("parse error")),
        }
    }
    async fn media_playing(
        &self,
        ctx: CallContext,
        media_playing_params: MediaPlayingParams,
    ) -> RpcResult<bool> {
        let media_playing = BehavioralMetricPayload::MediaPlaying(MediaPlaying {
            context: ctx.clone().into(),
            entity_id: media_playing_params.entity_id,
        });
        trace!("metrics.media_playing={:?}", media_playing);
        match send_metric(&self.state, media_playing, &ctx).await {
            Ok(_) => Ok(true),
            Err(_) => Err(rpc_err("parse error")),
        }
    }
    async fn media_pause(
        &self,
        ctx: CallContext,
        media_pause_params: MediaPauseParams,
    ) -> RpcResult<bool> {
        let media_pause = BehavioralMetricPayload::MediaPause(MediaPause {
            context: ctx.clone().into(),
            entity_id: media_pause_params.entity_id,
        });
        trace!("metrics.media_pause={:?}", media_pause);
        match send_metric(&self.state, media_pause, &ctx).await {
            Ok(_) => Ok(true),
            Err(_) => Err(rpc_err("parse error")),
        }
    }
    async fn media_waiting(
        &self,
        ctx: CallContext,
        media_waiting_params: MediaWaitingParams,
    ) -> RpcResult<bool> {
        let media_waiting = BehavioralMetricPayload::MediaWaiting(MediaWaiting {
            context: ctx.clone().into(),
            entity_id: media_waiting_params.entity_id,
        });
        trace!("metrics.media_waiting={:?}", media_waiting);
        match send_metric(&self.state, media_waiting, &ctx).await {
            Ok(_) => Ok(true),
            Err(_) => Err(rpc_err("parse error")),
        }
    }
    async fn media_progress(
        &self,
        ctx: CallContext,
        media_progress_params: MediaProgressParams,
    ) -> RpcResult<bool> {
        let progress = convert_to_media_position_type(media_progress_params.progress)?;
        let media_progress = BehavioralMetricPayload::MediaProgress(MediaProgress {
            context: ctx.clone().into(),
            entity_id: media_progress_params.entity_id,
            progress: Some(progress),
        });
        trace!("metrics.media_progress={:?}", media_progress);
        match send_metric(&self.state, media_progress, &ctx).await {
            Ok(_) => Ok(true),
            Err(_) => Err(rpc_err("parse error")),
        }
    }
    async fn media_seeking(
        &self,
        ctx: CallContext,
        media_seeking_params: MediaSeekingParams,
    ) -> RpcResult<bool> {
        let target = convert_to_media_position_type(media_seeking_params.target)?;

        let media_seeking = BehavioralMetricPayload::MediaSeeking(MediaSeeking {
            context: ctx.clone().into(),
            entity_id: media_seeking_params.entity_id,
            target: Some(target),
        });
        trace!("metrics.media_seeking={:?}", media_seeking);
        match send_metric(&self.state, media_seeking, &ctx).await {
            Ok(_) => Ok(true),
            Err(_) => Err(rpc_err("parse error")),
        }
    }
    async fn media_seeked(
        &self,
        ctx: CallContext,
        media_seeked_params: MediaSeekedParams,
    ) -> RpcResult<bool> {
        let position = convert_to_media_position_type(media_seeked_params.position).unwrap();
        let media_seeked = BehavioralMetricPayload::MediaSeeked(MediaSeeked {
            context: ctx.clone().into(),
            entity_id: media_seeked_params.entity_id,
            position: Some(position),
        });
        trace!("metrics.media_seeked={:?}", media_seeked);
        match send_metric(&self.state, media_seeked, &ctx).await {
            Ok(_) => Ok(true),
            Err(_) => Err(rpc_err("parse error")),
        }
    }
    async fn media_rate_change(
        &self,
        ctx: CallContext,
        media_rate_changed_params: MediaRateChangeParams,
    ) -> RpcResult<bool> {
        let media_rate_change = BehavioralMetricPayload::MediaRateChanged(MediaRateChanged {
            context: ctx.clone().into(),
            entity_id: media_rate_changed_params.entity_id,
            rate: media_rate_changed_params.rate,
        });
        trace!("metrics.media_seeked={:?}", media_rate_change);
        match send_metric(&self.state, media_rate_change, &ctx).await {
            Ok(_) => Ok(true),
            Err(_) => Err(rpc_err("parse error")),
        }
    }
    async fn media_rendition_change(
        &self,
        ctx: CallContext,
        media_rendition_change_params: MediaRenditionChangeParams,
    ) -> RpcResult<bool> {
        let media_rendition_change =
            BehavioralMetricPayload::MediaRenditionChanged(MediaRenditionChanged {
                context: ctx.clone().into(),
                entity_id: media_rendition_change_params.entity_id,
                bitrate: media_rendition_change_params.bitrate,
                height: media_rendition_change_params.height,
                profile: media_rendition_change_params.profile,
                width: media_rendition_change_params.width,
            });
        trace!(
            "metrics.media_rendition_change={:?}",
            media_rendition_change
        );
        match send_metric(&self.state, media_rendition_change, &ctx).await {
            Ok(_) => Ok(true),
            Err(_) => Err(rpc_err("parse error")),
        }
    }
    async fn media_ended(
        &self,
        ctx: CallContext,
        media_ended_params: MediaEndedParams,
    ) -> RpcResult<bool> {
        let media_ended = BehavioralMetricPayload::MediaEnded(MediaEnded {
            context: ctx.clone().into(),
            entity_id: media_ended_params.entity_id,
        });
        trace!("metrics.media_ended={:?}", media_ended);

        match send_metric(&self.state, media_ended, &ctx).await {
            Ok(_) => Ok(true),
            Err(_) => Err(rpc_err("parse error")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MetricsRPCProvider;
impl RippleRPCProvider<MetricsImpl> for MetricsRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<MetricsImpl> {
        (MetricsImpl { state }).into_rpc()
    }
}
