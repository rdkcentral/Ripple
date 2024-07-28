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
    broker::broker_utils::BrokerUtils,
    firebolt::rpc::RippleRPCProvider,
    processor::storage::storage_manager::StorageManager,
    service::apps::app_events::{AppEventDecorationError, AppEventDecorator},
    state::platform_state::PlatformState,
    utils::rpc_utils::{rpc_add_event_listener, rpc_add_event_listener_with_decorator},
};

use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    RpcModule,
};

use ripple_sdk::api::{
    device::{
        device_accessibility_data::{
            ClosedCaptionStyle, ClosedCaptionsSettings, FONT_EDGE_LIST, FONT_FAMILY_LIST,
        },
        device_peristence::SetPropertyOpt,
    },
    firebolt::{
        fb_general::{ListenRequest, ListenerResponse},
        fb_localization::SetPreferredAudioLanguage,
    },
    gateway::rpc_gateway_api::CallContext,
    storage_property::{
        StorageProperty as SP, EVENT_CC_PREFERRED_LANGUAGES,
        EVENT_CLOSED_CAPTIONS_BACKGROUND_COLOR, EVENT_CLOSED_CAPTIONS_BACKGROUND_OPACITY,
        EVENT_CLOSED_CAPTIONS_FONT_COLOR, EVENT_CLOSED_CAPTIONS_FONT_EDGE,
        EVENT_CLOSED_CAPTIONS_FONT_EDGE_COLOR, EVENT_CLOSED_CAPTIONS_FONT_FAMILY,
        EVENT_CLOSED_CAPTIONS_FONT_OPACITY, EVENT_CLOSED_CAPTIONS_FONT_SIZE,
        EVENT_CLOSED_CAPTIONS_SETTINGS_CHANGED, EVENT_CLOSED_CAPTIONS_TEXT_ALIGN,
        EVENT_CLOSED_CAPTIONS_TEXT_ALIGN_VERTICAL, EVENT_CLOSED_CAPTIONS_WINDOW_COLOR,
        EVENT_CLOSED_CAPTIONS_WINDOW_OPACITY,
    },
};
use serde_json::Value;

#[derive(Clone)]
struct CCEventDecorator {}

#[async_trait]
impl AppEventDecorator for CCEventDecorator {
    async fn decorate(
        &self,
        ps: &PlatformState,
        ctx: &CallContext,
        _event_name: &str,
        _val_in: &Value,
    ) -> Result<Value, AppEventDecorationError> {
        let settings = ClosedcaptionsImpl::get_cc_settings(ps, ctx).await?;
        Ok(serde_json::to_value(settings).unwrap_or_default())
    }

    fn dec_clone(&self) -> Box<dyn AppEventDecorator + Send + Sync> {
        Box::new(self.clone())
    }
}

#[rpc(server)]
pub trait Closedcaptions {
    #[method(name = "accessibility.closedCaptionsSettings", aliases = ["accessibility.closedCaptions"])]
    async fn closed_captions_settings(
        &self,
        _ctx: CallContext,
    ) -> RpcResult<ClosedCaptionsSettings>;
    #[method(name = "accessibility.onClosedCaptionsSettingsChanged")]
    async fn on_closed_captions_settings_changed(
        &self,
        _ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.fontFamily")]
    async fn font_family(&self, _ctx: CallContext) -> RpcResult<Option<String>>;
    #[method(name = "closedcaptions.setFontFamily")]
    async fn font_family_set(
        &self,
        _ctx: CallContext,
        request: SetPropertyOpt<String>,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onFontFamilyChanged")]
    async fn font_family_changed(
        &self,
        _ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.fontSize")]
    async fn font_size(&self, _ctx: CallContext) -> RpcResult<Option<f32>>;
    #[method(name = "closedcaptions.setFontSize")]
    async fn font_size_set(&self, _ctx: CallContext, request: SetPropertyOpt<f32>)
        -> RpcResult<()>;
    #[method(name = "closedcaptions.onFontSizeChanged")]
    async fn font_size_changed(
        &self,
        _ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.fontColor")]
    async fn font_color(&self, _ctx: CallContext) -> RpcResult<Option<String>>;
    #[method(name = "closedcaptions.setFontColor")]
    async fn font_color_set(
        &self,
        _ctx: CallContext,
        request: SetPropertyOpt<String>,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onFontColorChanged")]
    async fn font_color_changed(
        &self,
        _ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.fontEdge")]
    async fn font_edge(&self, _ctx: CallContext) -> RpcResult<Option<String>>;
    #[method(name = "closedcaptions.setFontEdge")]
    async fn font_edge_set(
        &self,
        _ctx: CallContext,
        request: SetPropertyOpt<String>,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onFontEdgeChanged")]
    async fn font_edge_changed(
        &self,
        _ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.fontEdgeColor")]
    async fn font_edge_color(&self, _ctx: CallContext) -> RpcResult<Option<String>>;
    #[method(name = "closedcaptions.setFontEdgeColor")]
    async fn font_edge_color_set(
        &self,
        _ctx: CallContext,
        request: SetPropertyOpt<String>,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onFontEdgeColorChanged")]
    async fn font_edge_color_changed(
        &self,
        _ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.fontOpacity")]
    async fn font_opacity(&self, _ctx: CallContext) -> RpcResult<Option<u32>>;
    #[method(name = "closedcaptions.setFontOpacity")]
    async fn font_opacity_set(
        &self,
        _ctx: CallContext,
        request: SetPropertyOpt<u32>,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onFontOpacityChanged")]
    async fn font_opacity_changed(
        &self,
        _ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.backgroundColor")]
    async fn background_color(&self, _ctx: CallContext) -> RpcResult<Option<String>>;
    #[method(name = "closedcaptions.setBackgroundColor")]
    async fn background_color_set(
        &self,
        _ctx: CallContext,
        request: SetPropertyOpt<String>,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onBackgroundColorChanged")]
    async fn background_color_changed(
        &self,
        _ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.backgroundOpacity")]
    async fn background_opacity(&self, _ctx: CallContext) -> RpcResult<Option<u32>>;
    #[method(name = "closedcaptions.setBackgroundOpacity")]
    async fn background_opacity_set(
        &self,
        _ctx: CallContext,
        request: SetPropertyOpt<u32>,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onBackgroundOpacityChanged")]
    async fn background_opacity_changed(
        &self,
        _ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.windowColor")]
    async fn window_color(&self, _ctx: CallContext) -> RpcResult<Option<String>>;
    #[method(name = "closedcaptions.setWindowColor")]
    async fn window_color_set(
        &self,
        _ctx: CallContext,
        request: SetPropertyOpt<String>,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onWindowColorChanged")]
    async fn window_color_changed(
        &self,
        _ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.windowOpacity")]
    async fn window_opacity(&self, _ctx: CallContext) -> RpcResult<Option<u32>>;
    #[method(name = "closedcaptions.setWindowOpacity")]
    async fn window_opacity_set(
        &self,
        _ctx: CallContext,
        request: SetPropertyOpt<u32>,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onWindowOpacityChanged")]
    async fn window_opacity_changed(
        &self,
        _ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.textAlign")]
    async fn text_align(&self, _ctx: CallContext) -> RpcResult<Option<String>>;
    #[method(name = "closedcaptions.setTextAlign")]
    async fn text_align_set(
        &self,
        _ctx: CallContext,
        request: SetPropertyOpt<String>,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onTextAlignChanged")]
    async fn text_align_changed(
        &self,
        _ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.textAlignVertical")]
    async fn text_align_vertical(&self, _ctx: CallContext) -> RpcResult<Option<String>>;
    #[method(name = "closedcaptions.setTextAlignVertical")]
    async fn text_align_vertical_set(
        &self,
        _ctx: CallContext,
        request: SetPropertyOpt<String>,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onTextAlignVerticalChanged")]
    async fn text_align_vertical_changed(
        &self,
        _ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.preferredLanguages")]
    async fn cc_preferred_languages(&self, _ctx: CallContext) -> RpcResult<Vec<String>>;
    #[method(name = "closedcaptions.setPreferredLanguages")]
    async fn cc_preferred_languages_set(
        &self,
        ctx: CallContext,
        set_request: SetPreferredAudioLanguage,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onPreferredLanguagesChanged")]
    async fn on_cc_preferred_languages(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
}

#[derive(Debug)]
pub struct ClosedcaptionsImpl {
    pub state: PlatformState,
}

impl ClosedcaptionsImpl {
    pub async fn get_cc_settings(
        ps: &PlatformState,
        call_context: &CallContext,
    ) -> RpcResult<ClosedCaptionsSettings> {
        use ClosedcaptionsImpl as CI;
        use SP::*;
        let enabled = ClosedcaptionsImpl::cc_enabled(ps, call_context).await?;
        let styles: ClosedCaptionStyle = ClosedCaptionStyle {
            font_family: CI::get_string(ps, ClosedCaptionsFontFamily).await?,
            font_size: CI::get_number_as_f32(ps, ClosedCaptionsFontSize).await?,
            font_color: CI::get_string(ps, ClosedCaptionsFontColor).await?,
            font_edge: CI::get_string(ps, ClosedCaptionsFontEdge).await?,
            font_edge_color: CI::get_string(ps, ClosedCaptionsFontEdgeColor).await?,
            font_opacity: CI::get_number_as_u32(ps, ClosedCaptionsFontOpacity).await?,
            background_color: CI::get_string(ps, ClosedCaptionsBackgroundColor).await?,
            background_opacity: CI::get_number_as_u32(ps, ClosedCaptionsBackgroundOpacity).await?,
            window_color: CI::get_string(ps, ClosedCaptionsWindowColor).await?,
            window_opacity: CI::get_number_as_u32(ps, ClosedCaptionsWindowOpacity).await?,
            text_align: CI::get_string(ps, ClosedCaptionsTextAlign).await?,
            text_align_vertical: CI::get_string(ps, ClosedCaptionsTextAlignVertical).await?,
        };
        let preferred_languages = StorageManager::get_vec_string(ps, SP::CCPreferredLanguages)
            .await
            .unwrap_or(Vec::new());
        Ok(ClosedCaptionsSettings {
            enabled,
            styles,
            preferred_languages,
        })
    }

    pub async fn get_string(ps: &PlatformState, property: SP) -> RpcResult<Option<String>> {
        match StorageManager::get_string(ps, property).await {
            Ok(val) => Ok(Some(val)),
            Err(_err) => {
                //err=Call(Custom { code: -50300, message: "ClosedCaptions.fontFamily is not available", data: None })
                // XXX: For now returning None for all errors
                // We should fix StorageManager to return PROPERTY_NOT_FOUND for get calls and filter out here
                Ok(None)
            }
        }
    }

    pub async fn get_number_as_u32(ps: &PlatformState, property: SP) -> RpcResult<Option<u32>> {
        match StorageManager::get_number_as_u32(ps, property).await {
            Ok(val) => Ok(Some(val)),
            _ => Ok(None),
        }
    }

    pub async fn get_number_as_f32(ps: &PlatformState, property: SP) -> RpcResult<Option<f32>> {
        match StorageManager::get_number_as_f32(ps, property).await {
            Ok(val) => Ok(Some(val)),
            _ => Ok(None),
        }
    }

    pub async fn set_string(
        ps: &PlatformState,
        property: SP,
        value: SetPropertyOpt<String>,
    ) -> RpcResult<()> {
        match value.value {
            Some(val) => StorageManager::set_string(ps, property, val, None).await,
            None => StorageManager::delete_key(ps, property).await,
        }
    }

    pub async fn set_f32(
        ps: &PlatformState,
        property: SP,
        request: SetPropertyOpt<f32>,
    ) -> RpcResult<()> {
        match request.value {
            Some(val) => StorageManager::set_number_as_f32(ps, property, val, None).await,
            None => StorageManager::delete_key(ps, property).await,
        }
    }

    pub async fn set_u32(
        ps: &PlatformState,
        property: SP,
        request: SetPropertyOpt<u32>,
    ) -> RpcResult<()> {
        match request.value {
            Some(val) => StorageManager::set_number_as_u32(ps, property, val, None).await,
            None => StorageManager::delete_key(ps, property).await,
        }
    }

    pub async fn set_opacity(
        ps: &PlatformState,
        property: SP,
        request: SetPropertyOpt<u32>,
    ) -> RpcResult<()> {
        if let Some(value) = request.value {
            if value > 100 {
                return Err(jsonrpsee::core::error::Error::Custom(
                    "Invalid Value for opacity".to_owned(),
                ));
            }
        }

        ClosedcaptionsImpl::set_u32(ps, property, request).await
    }

    fn is_font_family_supported(value: Option<String>) -> bool {
        match value {
            Some(val) => FONT_FAMILY_LIST.contains(&val.as_str()),
            None => true,
        }
    }

    fn is_font_edge_supported(value: Option<String>) -> bool {
        match value {
            Some(val) => FONT_EDGE_LIST.contains(&val.as_str()),
            None => true,
        }
    }

    pub async fn cc_enabled(state: &PlatformState, call_context: &CallContext) -> RpcResult<bool> {
        BrokerUtils::brokered_rpc_call::<bool>(
            state,
            call_context.clone(),
            "closedCaptions.enabled".into(),
        )
        .await
    }
}

#[async_trait]
impl ClosedcaptionsServer for ClosedcaptionsImpl {
    async fn closed_captions_settings(
        &self,
        ctx: CallContext,
    ) -> RpcResult<ClosedCaptionsSettings> {
        let settings = ClosedcaptionsImpl::get_cc_settings(&self.state, &ctx).await?;
        Ok(settings)
    }

    async fn on_closed_captions_settings_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener_with_decorator(
            &self.state,
            ctx,
            request,
            EVENT_CLOSED_CAPTIONS_SETTINGS_CHANGED,
            Some(Box::new(CCEventDecorator {})),
        )
        .await
    }

    async fn font_family(&self, _ctx: CallContext) -> RpcResult<Option<String>> {
        ClosedcaptionsImpl::get_string(&self.state, SP::ClosedCaptionsFontFamily).await
    }

    async fn font_family_set(
        &self,
        _ctx: CallContext,
        request: SetPropertyOpt<String>,
    ) -> RpcResult<()> {
        /*
         * Not implemented as a custom serde because this is not a custom datatype.
         */
        if ClosedcaptionsImpl::is_font_family_supported(request.value.clone()) {
            ClosedcaptionsImpl::set_string(&self.state, SP::ClosedCaptionsFontFamily, request).await
        } else {
            Err(jsonrpsee::core::Error::Custom(
                "Font family not supported".to_owned(),
            ))
        }
    }

    async fn font_family_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(&self.state, ctx, request, EVENT_CLOSED_CAPTIONS_FONT_FAMILY).await
    }

    async fn font_size(&self, _ctx: CallContext) -> RpcResult<Option<f32>> {
        ClosedcaptionsImpl::get_number_as_f32(&self.state, SP::ClosedCaptionsFontSize).await
    }

    async fn font_size_set(
        &self,
        _ctx: CallContext,
        request: SetPropertyOpt<f32>,
    ) -> RpcResult<()> {
        if let Some(value) = request.value {
            if !(0.5..=2.0).contains(&value) {
                return Err(jsonrpsee::core::error::Error::Custom(
                    "Invalid Value for set font".to_owned(),
                ));
            }
        }

        ClosedcaptionsImpl::set_f32(&self.state, SP::ClosedCaptionsFontSize, request).await
    }

    async fn font_size_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(&self.state, ctx, request, EVENT_CLOSED_CAPTIONS_FONT_SIZE).await
    }

    async fn font_color(&self, _ctx: CallContext) -> RpcResult<Option<String>> {
        ClosedcaptionsImpl::get_string(&self.state, SP::ClosedCaptionsFontColor).await
    }

    async fn font_color_set(
        &self,
        _ctx: CallContext,
        request: SetPropertyOpt<String>,
    ) -> RpcResult<()> {
        ClosedcaptionsImpl::set_string(&self.state, SP::ClosedCaptionsFontColor, request).await
    }

    async fn font_color_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(&self.state, ctx, request, EVENT_CLOSED_CAPTIONS_FONT_COLOR).await
    }

    async fn font_edge(&self, _ctx: CallContext) -> RpcResult<Option<String>> {
        ClosedcaptionsImpl::get_string(&self.state, SP::ClosedCaptionsFontEdge).await
    }

    async fn font_edge_set(
        &self,
        _ctx: CallContext,
        request: SetPropertyOpt<String>,
    ) -> RpcResult<()> {
        if ClosedcaptionsImpl::is_font_edge_supported(request.value.clone()) {
            ClosedcaptionsImpl::set_string(&self.state, SP::ClosedCaptionsFontEdge, request).await
        } else {
            Err(jsonrpsee::core::Error::Custom(
                "Font edge not supported".to_owned(),
            ))
        }
    }

    async fn font_edge_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(&self.state, ctx, request, EVENT_CLOSED_CAPTIONS_FONT_EDGE).await
    }

    async fn font_edge_color(&self, _ctx: CallContext) -> RpcResult<Option<String>> {
        ClosedcaptionsImpl::get_string(&self.state, SP::ClosedCaptionsFontEdgeColor).await
    }

    async fn font_edge_color_set(
        &self,
        _ctx: CallContext,
        request: SetPropertyOpt<String>,
    ) -> RpcResult<()> {
        ClosedcaptionsImpl::set_string(&self.state, SP::ClosedCaptionsFontEdgeColor, request).await
    }

    async fn font_edge_color_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(
            &self.state,
            ctx,
            request,
            EVENT_CLOSED_CAPTIONS_FONT_EDGE_COLOR,
        )
        .await
    }

    async fn font_opacity(&self, _ctx: CallContext) -> RpcResult<Option<u32>> {
        ClosedcaptionsImpl::get_number_as_u32(&self.state, SP::ClosedCaptionsFontOpacity).await
    }

    async fn font_opacity_set(
        &self,
        _ctx: CallContext,
        request: SetPropertyOpt<u32>,
    ) -> RpcResult<()> {
        ClosedcaptionsImpl::set_opacity(&self.state, SP::ClosedCaptionsFontOpacity, request).await
    }

    async fn font_opacity_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(
            &self.state,
            ctx,
            request,
            EVENT_CLOSED_CAPTIONS_FONT_OPACITY,
        )
        .await
    }

    async fn background_color(&self, _ctx: CallContext) -> RpcResult<Option<String>> {
        ClosedcaptionsImpl::get_string(&self.state, SP::ClosedCaptionsBackgroundColor).await
    }

    async fn background_color_set(
        &self,
        _ctx: CallContext,
        request: SetPropertyOpt<String>,
    ) -> RpcResult<()> {
        ClosedcaptionsImpl::set_string(&self.state, SP::ClosedCaptionsBackgroundColor, request)
            .await
    }

    async fn background_color_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(
            &self.state,
            ctx,
            request,
            EVENT_CLOSED_CAPTIONS_BACKGROUND_COLOR,
        )
        .await
    }

    async fn background_opacity(&self, _ctx: CallContext) -> RpcResult<Option<u32>> {
        ClosedcaptionsImpl::get_number_as_u32(&self.state, SP::ClosedCaptionsBackgroundOpacity)
            .await
    }

    async fn background_opacity_set(
        &self,
        _ctx: CallContext,
        request: SetPropertyOpt<u32>,
    ) -> RpcResult<()> {
        ClosedcaptionsImpl::set_opacity(&self.state, SP::ClosedCaptionsBackgroundOpacity, request)
            .await
    }

    async fn background_opacity_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(
            &self.state,
            ctx,
            request,
            EVENT_CLOSED_CAPTIONS_BACKGROUND_OPACITY,
        )
        .await
    }

    async fn window_color(&self, _ctx: CallContext) -> RpcResult<Option<String>> {
        ClosedcaptionsImpl::get_string(&self.state, SP::ClosedCaptionsWindowColor).await
    }

    async fn window_color_set(
        &self,
        _ctx: CallContext,
        request: SetPropertyOpt<String>,
    ) -> RpcResult<()> {
        ClosedcaptionsImpl::set_string(&self.state, SP::ClosedCaptionsWindowColor, request).await
    }

    async fn window_color_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(
            &self.state,
            ctx,
            request,
            EVENT_CLOSED_CAPTIONS_WINDOW_COLOR,
        )
        .await
    }

    async fn window_opacity(&self, _ctx: CallContext) -> RpcResult<Option<u32>> {
        ClosedcaptionsImpl::get_number_as_u32(&self.state, SP::ClosedCaptionsWindowOpacity).await
    }

    async fn window_opacity_set(
        &self,
        _ctx: CallContext,
        request: SetPropertyOpt<u32>,
    ) -> RpcResult<()> {
        ClosedcaptionsImpl::set_opacity(&self.state, SP::ClosedCaptionsWindowOpacity, request).await
    }

    async fn window_opacity_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(
            &self.state,
            ctx,
            request,
            EVENT_CLOSED_CAPTIONS_WINDOW_OPACITY,
        )
        .await
    }

    async fn text_align(&self, _ctx: CallContext) -> RpcResult<Option<String>> {
        ClosedcaptionsImpl::get_string(&self.state, SP::ClosedCaptionsTextAlign).await
    }

    async fn text_align_set(
        &self,
        _ctx: CallContext,
        request: SetPropertyOpt<String>,
    ) -> RpcResult<()> {
        ClosedcaptionsImpl::set_string(&self.state, SP::ClosedCaptionsTextAlign, request).await
    }

    async fn text_align_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(&self.state, ctx, request, EVENT_CLOSED_CAPTIONS_TEXT_ALIGN).await
    }

    async fn text_align_vertical(&self, _ctx: CallContext) -> RpcResult<Option<String>> {
        ClosedcaptionsImpl::get_string(&self.state, SP::ClosedCaptionsTextAlignVertical).await
    }

    async fn text_align_vertical_set(
        &self,
        _ctx: CallContext,
        request: SetPropertyOpt<String>,
    ) -> RpcResult<()> {
        ClosedcaptionsImpl::set_string(&self.state, SP::ClosedCaptionsTextAlignVertical, request)
            .await
    }

    async fn text_align_vertical_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(
            &self.state,
            ctx,
            request,
            EVENT_CLOSED_CAPTIONS_TEXT_ALIGN_VERTICAL,
        )
        .await
    }

    async fn cc_preferred_languages(&self, _ctx: CallContext) -> RpcResult<Vec<String>> {
        Ok(
            StorageManager::get_vec_string(&self.state, SP::CCPreferredLanguages)
                .await
                .unwrap_or(Vec::new()),
        )
    }

    async fn cc_preferred_languages_set(
        &self,
        _ctx: CallContext,
        set_request: SetPreferredAudioLanguage,
    ) -> RpcResult<()> {
        StorageManager::set_vec_string(
            &self.state,
            SP::CCPreferredLanguages,
            set_request.get_string(),
            None,
        )
        .await
    }

    async fn on_cc_preferred_languages(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(&self.state, ctx, request, EVENT_CC_PREFERRED_LANGUAGES).await
    }
}

pub struct ClosedcaptionsRPCProvider;
impl RippleRPCProvider<ClosedcaptionsImpl> for ClosedcaptionsRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<ClosedcaptionsImpl> {
        (ClosedcaptionsImpl { state }).into_rpc()
    }
}
