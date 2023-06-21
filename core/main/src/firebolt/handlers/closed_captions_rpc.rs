// If not stated otherwise in this file or this component's license file the
// following copyright and licenses apply:
//
// Copyright 2023 RDK Management
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

use crate::{
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
        device_accessibility_data::{ClosedCaptionStyle, ClosedCaptionsSettings, FONT_FAMILY_LIST},
        device_peristence::{SetBoolProperty, SetF32Property, SetStringProperty, SetU32Property},
    },
    firebolt::fb_general::{ListenRequest, ListenerResponse},
    gateway::rpc_gateway_api::CallContext,
    storage_property::{
        StorageProperty, EVENT_CLOSED_CAPTIONS_BACKGROUND_COLOR,
        EVENT_CLOSED_CAPTIONS_BACKGROUND_OPACITY, EVENT_CLOSED_CAPTIONS_ENABLED,
        EVENT_CLOSED_CAPTIONS_FONT_COLOR, EVENT_CLOSED_CAPTIONS_FONT_EDGE,
        EVENT_CLOSED_CAPTIONS_FONT_EDGE_COLOR, EVENT_CLOSED_CAPTIONS_FONT_FAMILY,
        EVENT_CLOSED_CAPTIONS_FONT_OPACITY, EVENT_CLOSED_CAPTIONS_FONT_SIZE,
        EVENT_CLOSED_CAPTIONS_SETTINGS_CHANGED, EVENT_CLOSED_CAPTIONS_TEXT_ALIGN,
        EVENT_CLOSED_CAPTIONS_TEXT_ALIGN_VERTICAL,
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
        _ctx: &CallContext,
        _event_name: &str,
        _val_in: &Value,
    ) -> Result<Value, AppEventDecorationError> {
        use crate::processor::storage::storage_manager::StorageManager as SM;
        use StorageProperty::*;
        let enabled_res = ClosedcaptionsImpl::closed_captions_settings_enabled(&ps).await;
        if let Err(_) = enabled_res {
            return Err(AppEventDecorationError {});
        }
        let styles: ClosedCaptionStyle = ClosedCaptionStyle {
            font_family: SM::get_string(&ps, ClosedCaptionsFontFamily).await?,
            font_size: SM::get_number_as_f32(&ps, ClosedCaptionsFontSize).await?,
            font_color: SM::get_string(&ps, ClosedCaptionsFontColor).await?,
            font_edge: SM::get_string(&ps, ClosedCaptionsFontEdge).await?,
            font_edge_color: SM::get_string(&ps, ClosedCaptionsFontEdgeColor).await?,
            font_opacity: SM::get_number_as_u32(&ps, ClosedCaptionsFontOpacity).await?,
            background_color: SM::get_string(&ps, ClosedCaptionsBackgroundColor).await?,
            background_opacity: SM::get_number_as_u32(&ps, ClosedCaptionsBackgroundOpacity).await?,
            text_align: SM::get_string(&ps, ClosedCaptionsTextAlign).await?,
            text_align_vertical: SM::get_string(&ps, ClosedCaptionsTextAlignVertical).await?,
        };
        Ok(serde_json::to_value(ClosedCaptionsSettings {
            enabled: enabled_res.unwrap(),
            styles: styles,
        })
        .unwrap())
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

    #[method(name = "closedcaptions.enabled")]
    async fn closed_captions_settings_enabled_rpc(&self, _ctx: CallContext) -> RpcResult<bool>;
    #[method(name = "closedcaptions.setEnabled")]
    async fn closed_captions_settings_enabled_set(
        &self,
        _ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onEnabledChanged")]
    async fn closed_captions_settings_enabled_changed(
        &self,
        _ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.fontFamily")]
    async fn closed_captions_settings_font_family(&self, _ctx: CallContext) -> RpcResult<String>;
    #[method(name = "closedcaptions.setFontFamily")]
    async fn closed_captions_settings_font_family_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onFontFamilyChanged")]
    async fn closed_captions_settings_font_family_changed(
        &self,
        _ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.fontSize")]
    async fn closed_captions_settings_font_size(&self, _ctx: CallContext) -> RpcResult<f32>;
    #[method(name = "closedcaptions.setFontSize")]
    async fn closed_captions_settings_font_size_set(
        &self,
        _ctx: CallContext,
        set_request: SetF32Property,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onFontSizeChanged")]
    async fn closed_captions_settings_font_size_changed(
        &self,
        _ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.fontColor")]
    async fn closed_captions_settings_font_color(&self, _ctx: CallContext) -> RpcResult<String>;
    #[method(name = "closedcaptions.setFontColor")]
    async fn closed_captions_settings_font_color_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onFontColorChanged")]
    async fn closed_captions_settings_font_color_changed(
        &self,
        _ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.fontEdge")]
    async fn closed_captions_settings_font_edge(&self, _ctx: CallContext) -> RpcResult<String>;
    #[method(name = "closedcaptions.setFontEdge")]
    async fn closed_captions_settings_font_edge_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onFontEdgeChanged")]
    async fn closed_captions_settings_font_edge_changed(
        &self,
        _ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.fontEdgeColor")]
    async fn closed_captions_settings_font_edge_color(
        &self,
        _ctx: CallContext,
    ) -> RpcResult<String>;
    #[method(name = "closedcaptions.setFontEdgeColor")]
    async fn closed_captions_settings_font_edge_color_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onFontEdgeColorChanged")]
    async fn closed_captions_settings_font_edge_color_changed(
        &self,
        _ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.fontOpacity")]
    async fn closed_captions_settings_font_opacity(&self, _ctx: CallContext) -> RpcResult<u32>;
    #[method(name = "closedcaptions.setFontOpacity")]
    async fn closed_captions_settings_font_opacity_set(
        &self,
        _ctx: CallContext,
        set_request: SetU32Property,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onFontOpacityChanged")]
    async fn closed_captions_settings_font_opacity_changed(
        &self,
        _ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.backgroundColor")]
    async fn closed_captions_settings_background_color(
        &self,
        _ctx: CallContext,
    ) -> RpcResult<String>;
    #[method(name = "closedcaptions.setBackgroundColor")]
    async fn closed_captions_settings_background_color_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onBackgroundColorChanged")]
    async fn closed_captions_settings_background_color_changed(
        &self,
        _ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.backgroundOpacity")]
    async fn closed_captions_settings_background_opacity(
        &self,
        _ctx: CallContext,
    ) -> RpcResult<u32>;
    #[method(name = "closedcaptions.setBackgroundOpacity")]
    async fn closed_captions_settings_background_opacity_set(
        &self,
        _ctx: CallContext,
        set_request: SetU32Property,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onBackgroundOpacityChanged")]
    async fn closed_captions_settings_background_opacity_changed(
        &self,
        _ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.textAlign")]
    async fn closed_captions_settings_text_align(&self, _ctx: CallContext) -> RpcResult<String>;
    #[method(name = "closedcaptions.setTextAlign")]
    async fn closed_captions_settings_text_align_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onTextAlignChanged")]
    async fn closed_captions_settings_text_align_changed(
        &self,
        _ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.textAlignVertical")]
    async fn closed_captions_settings_text_align_vertical(
        &self,
        _ctx: CallContext,
    ) -> RpcResult<String>;
    #[method(name = "closedcaptions.setTextAlignVertical")]
    async fn closed_captions_settings_text_align_vertical_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onTextAlignVerticalChanged")]
    async fn closed_captions_settings_text_align_vertical_changed(
        &self,
        _ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
}

#[derive(Debug)]
pub struct ClosedcaptionsImpl {
    pub state: PlatformState,
}

impl ClosedcaptionsImpl {
    fn is_font_family_supported(font_family: &str) -> bool {
        FONT_FAMILY_LIST.contains(&font_family)
    }

    pub async fn closed_captions_settings_enabled(state: &PlatformState) -> RpcResult<bool> {
        StorageManager::get_bool(state, StorageProperty::ClosedCaptionsEnabled).await
    }
}

#[async_trait]
impl ClosedcaptionsServer for ClosedcaptionsImpl {
    async fn closed_captions_settings(
        &self,
        ctx: CallContext,
    ) -> RpcResult<ClosedCaptionsSettings> {
        let cc_styles: ClosedCaptionStyle = ClosedCaptionStyle {
            font_family: self
                .closed_captions_settings_font_family(ctx.clone())
                .await?,
            font_size: self.closed_captions_settings_font_size(ctx.clone()).await?,
            font_color: self
                .closed_captions_settings_font_color(ctx.clone())
                .await?,
            font_edge: self.closed_captions_settings_font_edge(ctx.clone()).await?,
            font_edge_color: self
                .closed_captions_settings_font_edge_color(ctx.clone())
                .await?,
            font_opacity: self
                .closed_captions_settings_font_opacity(ctx.clone())
                .await?,
            background_color: self
                .closed_captions_settings_background_color(ctx.clone())
                .await?,
            background_opacity: self
                .closed_captions_settings_background_opacity(ctx.clone())
                .await?,
            text_align: self
                .closed_captions_settings_text_align(ctx.clone())
                .await?,
            text_align_vertical: self
                .closed_captions_settings_text_align_vertical(ctx.clone())
                .await?,
        };
        Ok(ClosedCaptionsSettings {
            enabled: self
                .closed_captions_settings_enabled_rpc(ctx.clone())
                .await?,
            styles: cc_styles,
        })
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

    async fn closed_captions_settings_enabled_rpc(&self, _ctx: CallContext) -> RpcResult<bool> {
        StorageManager::get_bool(&self.state, StorageProperty::ClosedCaptionsEnabled).await
    }

    async fn closed_captions_settings_enabled_set(
        &self,
        _ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        StorageManager::set_bool(
            &self.state,
            StorageProperty::ClosedCaptionsEnabled,
            set_request.value,
            None,
        )
        .await
    }

    async fn closed_captions_settings_enabled_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(&self.state, ctx, request, EVENT_CLOSED_CAPTIONS_ENABLED).await
    }

    async fn closed_captions_settings_font_family(&self, _ctx: CallContext) -> RpcResult<String> {
        StorageManager::get_string(&self.state, StorageProperty::ClosedCaptionsFontFamily).await
    }

    async fn closed_captions_settings_font_family_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        /*
         * Not implemented as a custom serde because this is not a custom datatype.
         */
        if ClosedcaptionsImpl::is_font_family_supported(set_request.value.as_str()) {
            StorageManager::set_string(
                &self.state,
                StorageProperty::ClosedCaptionsFontFamily,
                set_request.value,
                None,
            )
            .await
        } else {
            Err(jsonrpsee::core::Error::Custom(
                "Font family Not supported".to_owned(),
            ))
        }
    }

    async fn closed_captions_settings_font_family_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(&self.state, ctx, request, EVENT_CLOSED_CAPTIONS_FONT_FAMILY).await
    }

    async fn closed_captions_settings_font_size(&self, _ctx: CallContext) -> RpcResult<f32> {
        StorageManager::get_number_as_f32(&self.state, StorageProperty::ClosedCaptionsFontSize)
            .await
    }

    async fn closed_captions_settings_font_size_set(
        &self,
        _ctx: CallContext,
        set_request: SetF32Property,
    ) -> RpcResult<()> {
        if set_request.value >= 0.5 && set_request.value <= 2.0 {
            StorageManager::set_number_as_f32(
                &self.state,
                StorageProperty::ClosedCaptionsFontSize,
                set_request.value,
                None,
            )
            .await?;
            Ok(())
        } else {
            Err(jsonrpsee::core::error::Error::Custom(
                "Invalid Value for set font".to_owned(),
            ))
        }
    }

    async fn closed_captions_settings_font_size_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(&self.state, ctx, request, EVENT_CLOSED_CAPTIONS_FONT_SIZE).await
    }

    async fn closed_captions_settings_font_color(&self, _ctx: CallContext) -> RpcResult<String> {
        StorageManager::get_string(&self.state, StorageProperty::ClosedCaptionsFontColor).await
    }

    async fn closed_captions_settings_font_color_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        StorageManager::set_string(
            &self.state,
            StorageProperty::ClosedCaptionsFontColor,
            set_request.value,
            None,
        )
        .await
    }

    async fn closed_captions_settings_font_color_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(&self.state, ctx, request, EVENT_CLOSED_CAPTIONS_FONT_COLOR).await
    }

    async fn closed_captions_settings_font_edge(&self, _ctx: CallContext) -> RpcResult<String> {
        StorageManager::get_string(&self.state, StorageProperty::ClosedCaptionsFontEdge).await
    }

    async fn closed_captions_settings_font_edge_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        StorageManager::set_string(
            &self.state,
            StorageProperty::ClosedCaptionsFontEdge,
            set_request.value,
            None,
        )
        .await
    }

    async fn closed_captions_settings_font_edge_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(&self.state, ctx, request, EVENT_CLOSED_CAPTIONS_FONT_EDGE).await
    }

    async fn closed_captions_settings_font_edge_color(
        &self,
        _ctx: CallContext,
    ) -> RpcResult<String> {
        StorageManager::get_string(&self.state, StorageProperty::ClosedCaptionsFontEdgeColor).await
    }

    async fn closed_captions_settings_font_edge_color_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        StorageManager::set_string(
            &self.state,
            StorageProperty::ClosedCaptionsFontEdgeColor,
            set_request.value,
            None,
        )
        .await
    }

    async fn closed_captions_settings_font_edge_color_changed(
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

    async fn closed_captions_settings_font_opacity(&self, _ctx: CallContext) -> RpcResult<u32> {
        StorageManager::get_number_as_u32(&self.state, StorageProperty::ClosedCaptionsFontOpacity)
            .await
    }

    async fn closed_captions_settings_font_opacity_set(
        &self,
        _ctx: CallContext,
        set_request: SetU32Property,
    ) -> RpcResult<()> {
        StorageManager::set_number_as_u32(
            &self.state,
            StorageProperty::ClosedCaptionsFontOpacity,
            set_request.value,
            None,
        )
        .await?;
        Ok(())
    }

    async fn closed_captions_settings_font_opacity_changed(
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

    async fn closed_captions_settings_background_color(
        &self,
        _ctx: CallContext,
    ) -> RpcResult<String> {
        StorageManager::get_string(&self.state, StorageProperty::ClosedCaptionsBackgroundColor)
            .await
    }

    async fn closed_captions_settings_background_color_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        StorageManager::set_string(
            &self.state,
            StorageProperty::ClosedCaptionsBackgroundColor,
            set_request.value,
            None,
        )
        .await
    }

    async fn closed_captions_settings_background_color_changed(
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

    async fn closed_captions_settings_background_opacity(
        &self,
        _ctx: CallContext,
    ) -> RpcResult<u32> {
        StorageManager::get_number_as_u32(
            &self.state,
            StorageProperty::ClosedCaptionsBackgroundOpacity,
        )
        .await
    }

    async fn closed_captions_settings_background_opacity_set(
        &self,
        _ctx: CallContext,
        set_request: SetU32Property,
    ) -> RpcResult<()> {
        StorageManager::set_number_as_u32(
            &self.state,
            StorageProperty::ClosedCaptionsBackgroundOpacity,
            set_request.value,
            None,
        )
        .await?;
        Ok(())
    }

    async fn closed_captions_settings_background_opacity_changed(
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

    async fn closed_captions_settings_text_align(&self, _ctx: CallContext) -> RpcResult<String> {
        StorageManager::get_string(&self.state, StorageProperty::ClosedCaptionsTextAlign).await
    }

    async fn closed_captions_settings_text_align_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        StorageManager::set_string(
            &self.state,
            StorageProperty::ClosedCaptionsTextAlign,
            set_request.value,
            None,
        )
        .await
    }

    async fn closed_captions_settings_text_align_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(&self.state, ctx, request, EVENT_CLOSED_CAPTIONS_TEXT_ALIGN).await
    }

    async fn closed_captions_settings_text_align_vertical(
        &self,
        _ctx: CallContext,
    ) -> RpcResult<String> {
        StorageManager::get_string(
            &self.state,
            StorageProperty::ClosedCaptionsTextAlignVertical,
        )
        .await
    }

    async fn closed_captions_settings_text_align_vertical_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        StorageManager::set_string(
            &self.state,
            StorageProperty::ClosedCaptionsTextAlignVertical,
            set_request.value,
            None,
        )
        .await
    }

    async fn closed_captions_settings_text_align_vertical_changed(
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
}

pub struct ClosedcaptionsRPCProvider;
impl RippleRPCProvider<ClosedcaptionsImpl> for ClosedcaptionsRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<ClosedcaptionsImpl> {
        (ClosedcaptionsImpl { state }).into_rpc()
    }
}
