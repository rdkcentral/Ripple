use std::collections::HashMap;

use dpab::core::model::metrics::{
    hashmap_to_param_vec, Action, ActionType, BehavioralMetricContext,
    CategoryType, EosBehavioralMetric, Error, ErrorType, MediaEnded, MediaLoadStart, MediaPause,
    MediaPlay, MediaPlaying, MediaPositionType, MediaProgress, MediaRateChanged,
    MediaRenditionChanged, MediaSeeked, MediaSeeking, MediaWaiting, Page, Param, StartContent,
    StopContent,
};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    types::error::CallError,
    RpcModule,
};

use serde::Deserialize;

use tokio::sync::oneshot;
use tracing::trace;

use crate::{
    api::rpc::rpc_gateway::{CallContext, RPCProvider},
    apps::app_mgr::{AppError, AppManagerResponse, AppMethod, AppRequest},
    helpers::{
        error_util::JSON_RPC_STANDARD_ERROR_INVALID_PARAMS, metrics_helper::MetricsHelper,
        ripple_helper::IRippleHelper,
    },
    managers::capability_manager::{
        CapClassifiedRequest, FireboltCap, IGetLoadedCaps, RippleHandlerCaps,
    },
};
use crate::{
    helpers::ripple_helper::{RippleHelper, RippleHelperFactory, RippleHelperType},
    platform_state::PlatformState,
};

const LAUNCH_COMPLETED_SEGMENT: &'static str = "LAUNCH_COMPLETED";

#[derive(Deserialize, Debug)]
pub struct PageParams {
    #[serde(rename = "pageId")]
    pub page_id: String,
}

#[derive(Deserialize, Debug, Clone)]
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
fn validate_metrics_action_type(metrics_action: &str) -> RpcResult<bool> {
    match metrics_action.len() {
        1..=256 => Ok(true),
        _ => {
            return Err(jsonrpsee::core::Error::Call(CallError::Custom {
                code: JSON_RPC_STANDARD_ERROR_INVALID_PARAMS,
                message: "metrics.action.action_type out of range".to_string(),
                data: None,
            }))
        }
    }
}
pub const ERROR_MEDIA_POSITION_OUT_OF_RANGE: &str = "absolute media position out of range";
pub const ERROR_BAD_ABSOLUTE_MEDIA_POSITION: &str =
    "absolute media position must not contain any numbers to the right of the decimal point.";
/*
implement this: https://developer.comcast.com/firebolt-apis/core-sdk/v0.9.0/metrics#mediaposition
*/
fn convert_to_media_position_type(media_position: Option<f32>) -> RpcResult<MediaPositionType> {
    match media_position {
        Some(position) => {
            if position >= 0.0 && position <= 0.999 {
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

                if abs_position >= 1 && abs_position <= 86400 {
                    Ok(MediaPositionType::AbsolutePosition(abs_position))
                } else {
                    return Err(jsonrpsee::core::Error::Call(CallError::Custom {
                        code: JSON_RPC_STANDARD_ERROR_INVALID_PARAMS,
                        message: ERROR_MEDIA_POSITION_OUT_OF_RANGE.to_string(),
                        data: None,
                    }));
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
pub struct MetricsImpl<IRippleHelper> {
    pub ripple_helper: Box<IRippleHelper>,
    pub metrics_helper: Box<MetricsHelper>,
}

impl From<CallContext> for BehavioralMetricContext {
    fn from(call_context: CallContext) -> Self {
        BehavioralMetricContext {
            app_id: call_context.app_id,
            app_version: call_context.session_id.clone(),
            partner_id: call_context.session_id,
        }
    }
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
impl MetricsServer for MetricsImpl<RippleHelper> {
    async fn start_content(
        &self,
        ctx: CallContext,
        page_params: StartContentParams,
    ) -> RpcResult<bool> {
        let start_content = EosBehavioralMetric::StartContent(StartContent {
            context: ctx.into(),
            entity_id: page_params.entity_id,
        });
        self.metrics_helper.send(start_content.clone()).await;
        trace!("metrics.startContent={:?}", start_content);
        Ok(true)
    }
    async fn stop_content(
        &self,
        ctx: CallContext,
        stop_content_params: StopContentParams,
    ) -> RpcResult<bool> {
        let stop_content = EosBehavioralMetric::StopContent(StopContent {
            context: ctx.into(),
            entity_id: stop_content_params.entity_id,
        });
        self.metrics_helper.send(stop_content.clone()).await;
        trace!("metrics.stopContent={:?}", stop_content);
        Ok(true)
    }
    async fn page(&self, ctx: CallContext, page_params: PageParams) -> RpcResult<bool> {
        let page = EosBehavioralMetric::Page(Page {
            context: ctx.into(),
            page_id: page_params.page_id,
        });
        self.metrics_helper.send(page.clone()).await;
        trace!("metrics.page={:?}", page);
        Ok(true)
    }
    async fn action(&self, ctx: CallContext, action_params: ActionParams) -> RpcResult<bool> {
        //call it and let it blow up
        let _ = validate_metrics_action_type(&action_params.action_type)?;

        let action = EosBehavioralMetric::Action(Action {
            context: ctx.into(),
            category: action_params.clone().into(),
            parameters: hashmap_to_param_vec(action_params.parameters),
            _type: action_params.action_type.clone(),
        });
        self.metrics_helper.send(action.clone()).await;
        trace!("metrics.action={:?}", action);
        Ok(true)
    }
    async fn ready(&self, ctx: CallContext) -> RpcResult<bool> {
        self.metrics_helper
            .app_done_loading(ctx.app_id, true, None, None)
            .await;
        Ok(true)
    }

    async fn error(&self, ctx: CallContext, error_params: ErrorParams) -> RpcResult<bool> {
        let app_id = ctx.app_id.clone();
        let error_message = EosBehavioralMetric::Error(Error {
            context: ctx.into(),
            error_type: error_params.clone().into(),
            code: error_params.code.clone(),
            description: error_params.description.clone(),
            visible: error_params.visible.clone(),
            parameters: error_params.parameters.clone(),
        });
        self.metrics_helper.send(error_message.clone()).await;
        self.metrics_helper.app_error(app_id, error_params).await;
        trace!("metrics.error={:?}", error_message);
        Ok(true)
    }
    async fn media_load_start(
        &self,
        ctx: CallContext,
        media_load_start_params: MediaLoadStartParams,
    ) -> RpcResult<bool> {
        let media_load_start_message = EosBehavioralMetric::MediaLoadStart(MediaLoadStart {
            context: ctx.into(),
            entity_id: media_load_start_params.entity_id,
        });
        self.metrics_helper
            .send(media_load_start_message.clone())
            .await;
        trace!("metrics.media_load_start={:?}", media_load_start_message);
        Ok(true)
    }
    async fn media_play(
        &self,
        ctx: CallContext,
        media_play_params: MediaPlayParams,
    ) -> RpcResult<bool> {
        let media_play_message = EosBehavioralMetric::MediaPlay(MediaPlay {
            context: ctx.into(),
            entity_id: media_play_params.entity_id,
        });
        self.metrics_helper.send(media_play_message.clone()).await;
        trace!("metrics.media_play={:?}", media_play_message);
        Ok(true)
    }
    async fn media_playing(
        &self,
        ctx: CallContext,
        media_playing_params: MediaPlayingParams,
    ) -> RpcResult<bool> {
        let media_playing = EosBehavioralMetric::MediaPlaying(MediaPlaying {
            context: ctx.into(),
            entity_id: media_playing_params.entity_id,
        });
        self.metrics_helper.send(media_playing.clone()).await;
        trace!("metrics.media_playing={:?}", media_playing);
        Ok(true)
    }
    async fn media_pause(
        &self,
        ctx: CallContext,
        media_pause_params: MediaPauseParams,
    ) -> RpcResult<bool> {
        let media_pause = EosBehavioralMetric::MediaPause(MediaPause {
            context: ctx.into(),
            entity_id: media_pause_params.entity_id,
        });
        self.metrics_helper.send(media_pause.clone()).await;
        trace!("metrics.media_pause={:?}", media_pause);
        Ok(true)
    }
    async fn media_waiting(
        &self,
        ctx: CallContext,
        media_waiting_params: MediaWaitingParams,
    ) -> RpcResult<bool> {
        let media_waiting = EosBehavioralMetric::MediaWaiting(MediaWaiting {
            context: ctx.into(),
            entity_id: media_waiting_params.entity_id,
        });
        self.metrics_helper.send(media_waiting.clone()).await;
        trace!("metrics.media_waiting={:?}", media_waiting);
        Ok(true)
    }
    async fn media_progress(
        &self,
        ctx: CallContext,
        media_progress_params: MediaProgressParams,
    ) -> RpcResult<bool> {
        let progress = convert_to_media_position_type(media_progress_params.progress)?;
        let media_progress = EosBehavioralMetric::MediaProgress(MediaProgress {
            context: ctx.into(),
            entity_id: media_progress_params.entity_id,
            progress: Some(progress),
        });
        self.metrics_helper.send(media_progress.clone()).await;
        trace!("metrics.media_progress={:?}", media_progress);
        Ok(true)
    }
    async fn media_seeking(
        &self,
        ctx: CallContext,
        media_seeking_params: MediaSeekingParams,
    ) -> RpcResult<bool> {
        let target = convert_to_media_position_type(media_seeking_params.target)?;

        let media_seeking = EosBehavioralMetric::MediaSeeking(MediaSeeking {
            context: ctx.into(),
            entity_id: media_seeking_params.entity_id,
            target: Some(target),
        });
        self.metrics_helper.send(media_seeking.clone()).await;
        trace!("metrics.media_seeking={:?}", media_seeking);
        Ok(true)
    }
    async fn media_seeked(
        &self,
        ctx: CallContext,
        media_seeked_params: MediaSeekedParams,
    ) -> RpcResult<bool> {
        let position = convert_to_media_position_type(media_seeked_params.position).unwrap();
        let media_seeked = EosBehavioralMetric::MediaSeeked(MediaSeeked {
            context: ctx.into(),
            entity_id: media_seeked_params.entity_id,
            position: Some(position),
        });
        self.metrics_helper.send(media_seeked.clone()).await;
        trace!("metrics.media_seeked={:?}", media_seeked);
        Ok(true)
    }
    async fn media_rate_change(
        &self,
        ctx: CallContext,
        media_rate_changed_params: MediaRateChangeParams,
    ) -> RpcResult<bool> {
        let media_rate_change = EosBehavioralMetric::MediaRateChanged(MediaRateChanged {
            context: ctx.into(),
            entity_id: media_rate_changed_params.entity_id,
            rate: media_rate_changed_params.rate,
        });
        self.metrics_helper.send(media_rate_change.clone()).await;
        trace!("metrics.media_seeked={:?}", media_rate_change);
        Ok(true)
    }
    async fn media_rendition_change(
        &self,
        ctx: CallContext,
        media_rendition_change_params: MediaRenditionChangeParams,
    ) -> RpcResult<bool> {
        let media_rendition_change =
            EosBehavioralMetric::MediaRenditionChanged(MediaRenditionChanged {
                context: ctx.into(),
                entity_id: media_rendition_change_params.entity_id,
                bitrate: media_rendition_change_params.bitrate,
                height: media_rendition_change_params.height,
                profile: media_rendition_change_params.profile,
                width: media_rendition_change_params.width,
            });
        self.metrics_helper
            .send(media_rendition_change.clone())
            .await;
        trace!(
            "metrics.media_rendition_change={:?}",
            media_rendition_change
        );
        Ok(true)
    }
    async fn media_ended(
        &self,
        ctx: CallContext,
        media_ended_params: MediaEndedParams,
    ) -> RpcResult<bool> {
        let media_ended = EosBehavioralMetric::MediaEnded(MediaEnded {
            context: ctx.into(),
            entity_id: media_ended_params.entity_id,
        });
        self.metrics_helper.send(media_ended.clone()).await;
        trace!("metrics.media_ended={:?}", media_ended);
        Ok(true)
    }
}
#[derive(Debug, Clone)]
pub struct MetricsRippleProvider;

pub struct MetricsCapHandler;
impl IGetLoadedCaps for MetricsCapHandler {
    fn get_loaded_caps(&self) -> RippleHandlerCaps {
        RippleHandlerCaps {
            caps: Some(vec![CapClassifiedRequest::Supported(vec![
                FireboltCap::Short("metrics:general".into()),
                FireboltCap::Short("metrics:media".into()),
            ])]),
        }
    }
}

/*
Called once at startup
*/
impl RPCProvider<MetricsImpl<RippleHelper>, MetricsCapHandler> for MetricsRippleProvider {
    fn provide(
        self,
        rhf: Box<RippleHelperFactory>,
        platform_state: PlatformState,
    ) -> (RpcModule<MetricsImpl<RippleHelper>>, MetricsCapHandler) {
        /*
        Get sift endpoint stuff and register callback for context changes
        */
        let ripple_helper = rhf.clone().get(self.get_helper_variant());

        let provider = MetricsImpl {
            ripple_helper: ripple_helper.clone(),
            metrics_helper: Box::new(MetricsHelper::new(Box::new(platform_state.clone()))),
        };

        (provider.into_rpc(), MetricsCapHandler)
    }

    fn get_helper_variant(self) -> Vec<RippleHelperType> {
        vec![
            RippleHelperType::Dab,
            RippleHelperType::Dpab,
            RippleHelperType::Cap,
            RippleHelperType::AppManager,
            RippleHelperType::MetricsManager,
        ]
    }
}
