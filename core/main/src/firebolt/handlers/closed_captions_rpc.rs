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
    firebolt::rpc::RippleRPCProvider, service::apps::provider_broker::ProviderBroker,
    state::platform_state::PlatformState, utils::rpc_utils::rpc_err,
};

use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    tracing::info,
    RpcModule,
};

use ripple_sdk::{
    api::{
        device::device_accessibility_data::{
            ClosedCaptionStyle, ClosedCaptionsSettings, GetStorageProperty, OpacityProperty,
            SetBoolProperty, SetF32Property, SetStorageProperty, SetStringProperty, StorageRequest,
            CLOSED_CAPTION_BACKGROUND_COLOR_CHANGED_EVENT,
            CLOSED_CAPTION_BACKGROUND_OPACITY_CHANGED_EVENT, CLOSED_CAPTION_ENABLED_CHANGED_EVENT,
            CLOSED_CAPTION_FONT_COLOR_CHANGED_EVENT, CLOSED_CAPTION_FONT_EDGE_CHANGED_EVENT,
            CLOSED_CAPTION_FONT_EDGE_COLOR_CHANGED_EVENT, CLOSED_CAPTION_FONT_FAMILY_CHANGED_EVENT,
            CLOSED_CAPTION_FONT_OPACITY_CHANGED_EVENT, CLOSED_CAPTION_FONT_SIZE_CHANGED_EVENT,
            CLOSED_CAPTION_SETTING_CHANGED_EVENT, CLOSED_CAPTION_TEXT_ALIGN_CHANGED_EVENT,
            CLOSED_CAPTION_TEXT_ALIGN_VERTICAL_CHANGED_EVENT,
        },
        firebolt::fb_general::{ListenRequest, ListenerResponse},
        gateway::rpc_gateway_api::CallContext,
    },
    extn::extn_client_message::ExtnResponse,
    serde_json::json,
};

#[rpc(server)]
pub trait Closedcaptions {
    #[method(name = "accessibility.closedCaptionsSettings")]
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
    async fn closed_captions_settings_font_size(&self, _ctx: CallContext) -> RpcResult<u32>;
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
        set_request: OpacityProperty,
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
        set_request: OpacityProperty,
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
    pub async fn on_request_app_event(
        &self,
        ctx: CallContext,
        request: ListenRequest,
        method: &'static str,
        event_name: &'static str,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        ProviderBroker::register_or_unregister_provider(
            &self.state,
            // TODO update with Firebolt Cap in later effort
            "xrn:firebolt:capability:accessibility:closedcaptions".into(),
            method.into(),
            event_name,
            ctx,
            request,
        )
        .await;

        Ok(ListenerResponse {
            listening: listen,
            event: event_name.into(),
        })
    }
}

#[async_trait]
impl ClosedcaptionsServer for ClosedcaptionsImpl {
    async fn closed_captions_settings(
        &self,
        _ctx: CallContext,
    ) -> RpcResult<ClosedCaptionsSettings> {
        info!("Accessibility.closedCaptionsSettings");
        let mut cc_styles: Vec<ClosedCaptionStyle> = Vec::new();
        cc_styles.push(ClosedCaptionStyle {
            font_family: self
                .closed_captions_settings_font_family(_ctx.clone())
                .await?,
            font_size: self
                .closed_captions_settings_font_size(_ctx.clone())
                .await?,
            font_color: self
                .closed_captions_settings_font_color(_ctx.clone())
                .await?,
            font_edge: self
                .closed_captions_settings_font_edge(_ctx.clone())
                .await?,
            font_edge_color: self
                .closed_captions_settings_font_edge_color(_ctx.clone())
                .await?,
            font_opacity: self
                .closed_captions_settings_font_opacity(_ctx.clone())
                .await?,
            background_color: self
                .closed_captions_settings_background_color(_ctx.clone())
                .await?,
            background_opacity: self
                .closed_captions_settings_background_opacity(_ctx.clone())
                .await?,
            text_align: self
                .closed_captions_settings_text_align(_ctx.clone())
                .await?,
            text_align_vertical: self
                .closed_captions_settings_text_align_vertical(_ctx.clone())
                .await?,
        });
        Ok(ClosedCaptionsSettings {
            enabled: self
                .closed_captions_settings_enabled_rpc(_ctx.clone())
                .await?,
            styles: cc_styles,
        })
    }

    async fn on_closed_captions_settings_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_app_event(
            ctx,
            request,
            "ClosedCaptionsSettingsChanged",
            CLOSED_CAPTION_SETTING_CHANGED_EVENT,
        )
        .await
    }

    async fn closed_captions_settings_font_family(&self, _ctx: CallContext) -> RpcResult<String> {
        info!("ClosedCaptions.fontFamily");
        let data = GetStorageProperty {
            namespace: "ClosedCaptions".to_string(),
            key: "fontFamily".to_string(),
        };
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(StorageRequest::Get(data))
            .await
        {
            if let Some(ExtnResponse::String(v)) = response.payload.clone().extract() {
                return Ok(v);
            }
        }
        Err(rpc_err("ClosedCaptions.fontFamily is not available"))
    }

    async fn closed_captions_settings_enabled_rpc(&self, _ctx: CallContext) -> RpcResult<bool> {
        info!("ClosedCaptions.enabled");
        let data = GetStorageProperty {
            namespace: "ClosedCaptions".to_string(),
            key: "enabled".to_string(),
        };
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(StorageRequest::Get(data))
            .await
        {
            if let Some(ExtnResponse::String(v)) = response.payload.clone().extract() {
                return Ok(v.parse::<bool>().unwrap());
            }
        }
        Err(rpc_err("ClosedCaptions.enabled is not available"))
    }

    async fn closed_captions_settings_enabled_set(
        &self,
        _ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        info!("ClosedCaptions.setEnabled");
        let data = SetStorageProperty {
            namespace: "ClosedCaptions".to_string(),
            key: "enabled".to_string(),
            value: json!(set_request.value),
        };
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(StorageRequest::Set(data))
            .await
        {
            if let Some(ExtnResponse::Boolean(_v)) = response.payload.clone().extract() {
                return Ok(());
            }
        }
        Err(rpc_err("FB error response TBD"))
    }

    async fn closed_captions_settings_enabled_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_app_event(
            ctx,
            request,
            "ClosedCaptionsSettingsEnabledChanged",
            CLOSED_CAPTION_ENABLED_CHANGED_EVENT,
        )
        .await
    }

    async fn closed_captions_settings_font_family_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        let data = SetStorageProperty {
            namespace: "ClosedCaptions".to_string(),
            key: "fontFamily".to_string(),
            value: json!(set_request.value),
        };
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(StorageRequest::Set(data))
            .await
        {
            if let Some(ExtnResponse::Boolean(_v)) = response.payload.clone().extract() {
                return Ok(());
            }
        }
        Err(rpc_err("FB error response TBD"))
    }

    async fn closed_captions_settings_font_family_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_app_event(
            ctx,
            request,
            "ClosedCaptionsSettingsFontFamilyChanged",
            CLOSED_CAPTION_FONT_FAMILY_CHANGED_EVENT,
        )
        .await
    }

    async fn closed_captions_settings_font_size(&self, _ctx: CallContext) -> RpcResult<u32> {
        info!("ClosedCaptions.fontSize");
        let data = GetStorageProperty {
            namespace: "ClosedCaptions".to_string(),
            key: "fontSize".to_string(),
        };
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(StorageRequest::Get(data))
            .await
        {
            if let Some(ExtnResponse::String(v)) = response.payload.clone().extract() {
                return Ok(v.parse::<u32>().unwrap());
            }
        }
        Err(rpc_err("ClosedCaptions.fontSize is not available"))
    }

    async fn closed_captions_settings_font_size_set(
        &self,
        _ctx: CallContext,
        set_request: SetF32Property,
    ) -> RpcResult<()> {
        info!("ClosedCaptions.setFontSize");
        let data = SetStorageProperty {
            namespace: "ClosedCaptions".to_string(),
            key: "fontSize".to_string(),
            value: json!(set_request.value),
        };

        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(StorageRequest::Set(data))
            .await
        {
            if let Some(ExtnResponse::Boolean(_v)) = response.payload.clone().extract() {
                return Ok(());
            }
        }
        Err(rpc_err("FB error response TBD"))
    }

    async fn closed_captions_settings_font_size_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_app_event(
            ctx,
            request,
            "ClosedCaptionsSettingsFontSizeChanged",
            CLOSED_CAPTION_FONT_SIZE_CHANGED_EVENT,
        )
        .await
    }

    async fn closed_captions_settings_font_color(&self, _ctx: CallContext) -> RpcResult<String> {
        info!("ClosedCaptions.fontColor");
        let data = GetStorageProperty {
            namespace: "ClosedCaptions".to_string(),
            key: "fontColor".to_string(),
        };
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(StorageRequest::Get(data))
            .await
        {
            if let Some(ExtnResponse::String(v)) = response.payload.clone().extract() {
                return Ok(v);
            }
        }
        Err(rpc_err("ClosedCaptions.fontColor is not available"))
    }

    async fn closed_captions_settings_font_color_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        info!("ClosedCaptions.setFontColor");
        let data = SetStorageProperty {
            namespace: "ClosedCaptions".to_string(),
            key: "fontColor".to_string(),
            value: json!(set_request.value),
        };
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(StorageRequest::Set(data))
            .await
        {
            if let Some(ExtnResponse::Boolean(_v)) = response.payload.clone().extract() {
                return Ok(());
            }
        }
        Err(rpc_err("FB error response TBD"))
    }

    async fn closed_captions_settings_font_color_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_app_event(
            ctx,
            request,
            "ClosedCaptionsSettingsFontColorChanged",
            CLOSED_CAPTION_FONT_COLOR_CHANGED_EVENT,
        )
        .await
    }

    async fn closed_captions_settings_font_edge(&self, _ctx: CallContext) -> RpcResult<String> {
        info!("ClosedCaptions.fontEdge");
        let data = GetStorageProperty {
            namespace: "ClosedCaptions".to_string(),
            key: "fontEdge".to_string(),
        };

        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(StorageRequest::Get(data))
            .await
        {
            if let Some(ExtnResponse::String(v)) = response.payload.clone().extract() {
                return Ok(v);
            }
        }
        Err(rpc_err("ClosedCaptions.fontEdge is not available"))
    }

    async fn closed_captions_settings_font_edge_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        info!("ClosedCaptions.setFontEdge");
        let data = SetStorageProperty {
            namespace: "ClosedCaptions".to_string(),
            key: "fontEdge".to_string(),
            value: json!(set_request.value),
        };
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(StorageRequest::Set(data))
            .await
        {
            if let Some(ExtnResponse::Boolean(_v)) = response.payload.clone().extract() {
                return Ok(());
            }
        }
        Err(rpc_err("FB error response TBD"))
    }

    async fn closed_captions_settings_font_edge_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_app_event(
            ctx,
            request,
            "ClosedCaptionsSettingsFontEdgeChanged",
            CLOSED_CAPTION_FONT_EDGE_CHANGED_EVENT,
        )
        .await
    }

    async fn closed_captions_settings_font_edge_color(
        &self,
        _ctx: CallContext,
    ) -> RpcResult<String> {
        info!("ClosedCaptions.fontEdgeColor");
        let data = GetStorageProperty {
            namespace: "ClosedCaptions".to_string(),
            key: "fontEdgeColor".to_string(),
        };
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(StorageRequest::Get(data))
            .await
        {
            if let Some(ExtnResponse::String(v)) = response.payload.clone().extract() {
                return Ok(v);
            }
        }
        Err(rpc_err("ClosedCaptions.fontOpacity is not available"))
    }

    async fn closed_captions_settings_font_edge_color_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        info!("ClosedCaptions.setFontEdgeColor");
        let data = SetStorageProperty {
            namespace: "ClosedCaptions".to_string(),
            key: "fontEdgeColor".to_string(),
            value: json!(set_request.value),
        };
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(StorageRequest::Set(data))
            .await
        {
            if let Some(ExtnResponse::Boolean(_v)) = response.payload.clone().extract() {
                return Ok(());
            }
        }
        Err(rpc_err("FB error response TBD"))
    }

    async fn closed_captions_settings_font_edge_color_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_app_event(
            ctx,
            request,
            "ClosedCaptionsSettingsFontEdgeColorChanged",
            CLOSED_CAPTION_FONT_EDGE_COLOR_CHANGED_EVENT,
        )
        .await
    }

    async fn closed_captions_settings_font_opacity(&self, _ctx: CallContext) -> RpcResult<u32> {
        info!("ClosedCaptions.fontOpacity");
        let data = GetStorageProperty {
            namespace: "ClosedCaptions".to_string(),
            key: "fontOpacity".to_string(),
        };
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(StorageRequest::Get(data))
            .await
        {
            if let Some(ExtnResponse::String(v)) = response.payload.clone().extract() {
                return Ok(v.parse::<u32>().unwrap());
            }
        }
        Err(rpc_err("ClosedCaptions.fontOpacity is not available"))
    }

    async fn closed_captions_settings_font_opacity_set(
        &self,
        _ctx: CallContext,
        set_request: OpacityProperty,
    ) -> RpcResult<()> {
        info!("ClosedCaptions.setFontOpacity");
        let data = SetStorageProperty {
            namespace: "ClosedCaptions".to_string(),
            key: "fontOpacity".to_string(),
            value: json!(set_request.value),
        };
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(StorageRequest::Set(data))
            .await
        {
            if let Some(ExtnResponse::Boolean(_v)) = response.payload.clone().extract() {
                return Ok(());
            }
        }
        Err(rpc_err("FB error response TBD"))
    }

    async fn closed_captions_settings_font_opacity_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_app_event(
            ctx,
            request,
            "ClosedCaptionsSettingsFontOpacityChanged",
            CLOSED_CAPTION_FONT_OPACITY_CHANGED_EVENT,
        )
        .await
    }

    async fn closed_captions_settings_background_color(
        &self,
        _ctx: CallContext,
    ) -> RpcResult<String> {
        info!("ClosedCaptions.backgroundColor");
        let data = GetStorageProperty {
            namespace: "ClosedCaptions".to_string(),
            key: "backgroundColor".to_string(),
        };
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(StorageRequest::Get(data))
            .await
        {
            if let Some(ExtnResponse::String(v)) = response.payload.clone().extract() {
                return Ok(v);
            }
        }
        Err(rpc_err("ClosedCaptions.backgroundColor is not available"))
    }

    async fn closed_captions_settings_background_color_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        info!("ClosedCaptions.setBackgroundColor");
        let data = SetStorageProperty {
            namespace: "ClosedCaptions".to_string(),
            key: "backgroundColor".to_string(),
            value: json!(set_request.value),
        };
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(StorageRequest::Set(data))
            .await
        {
            if let Some(ExtnResponse::Boolean(_v)) = response.payload.clone().extract() {
                return Ok(());
            }
        }
        Err(rpc_err("FB error response TBD"))
    }

    async fn closed_captions_settings_background_color_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_app_event(
            ctx,
            request,
            "ClosedCaptionsSettingsBackgroundColorChanged",
            CLOSED_CAPTION_BACKGROUND_COLOR_CHANGED_EVENT,
        )
        .await
    }

    async fn closed_captions_settings_background_opacity(
        &self,
        _ctx: CallContext,
    ) -> RpcResult<u32> {
        info!("ClosedCaptions.backgroundOpacity");
        let data = GetStorageProperty {
            namespace: "ClosedCaptions".to_string(),
            key: "backgroundOpacity".to_string(),
        };
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(StorageRequest::Get(data))
            .await
        {
            if let Some(ExtnResponse::String(v)) = response.payload.clone().extract() {
                return Ok(v.parse::<u32>().unwrap());
            }
        }
        Err(rpc_err("ClosedCaptions.backgroundOpacity is not available"))
    }

    async fn closed_captions_settings_background_opacity_set(
        &self,
        _ctx: CallContext,
        set_request: OpacityProperty,
    ) -> RpcResult<()> {
        info!("ClosedCaptions.setBackgroundOpacity");
        let data = SetStorageProperty {
            namespace: "ClosedCaptions".to_string(),
            key: "backgroundOpacity".to_string(),
            value: json!(set_request.value),
        };
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(StorageRequest::Set(data))
            .await
        {
            if let Some(ExtnResponse::Boolean(_v)) = response.payload.clone().extract() {
                return Ok(());
            }
        }
        Err(rpc_err("FB error response TBD"))
    }

    async fn closed_captions_settings_background_opacity_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_app_event(
            ctx,
            request,
            "ClosedCaptionsSettingsBackgroundOpacityChanged",
            CLOSED_CAPTION_BACKGROUND_OPACITY_CHANGED_EVENT,
        )
        .await
    }

    async fn closed_captions_settings_text_align(&self, _ctx: CallContext) -> RpcResult<String> {
        info!("ClosedCaptions.textAlign");
        let data = GetStorageProperty {
            namespace: "ClosedCaptions".to_string(),
            key: "textAlign".to_string(),
        };
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(StorageRequest::Get(data))
            .await
        {
            if let Some(ExtnResponse::String(v)) = response.payload.clone().extract() {
                return Ok(v);
            }
        }
        Err(rpc_err("ClosedCaptions.textAlign is not available"))
    }

    async fn closed_captions_settings_text_align_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        info!("ClosedCaptions.setTextAlign");
        let data = SetStorageProperty {
            namespace: "ClosedCaptions".to_string(),
            key: "textAlign".to_string(),
            value: json!(set_request.value),
        };
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(StorageRequest::Set(data))
            .await
        {
            if let Some(ExtnResponse::Boolean(_v)) = response.payload.clone().extract() {
                return Ok(());
            }
        }
        Err(rpc_err("FB error response TBD"))
    }

    async fn closed_captions_settings_text_align_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_app_event(
            ctx,
            request,
            "ClosedCaptionsSettingsTextAlignChanged",
            CLOSED_CAPTION_TEXT_ALIGN_CHANGED_EVENT,
        )
        .await
    }

    async fn closed_captions_settings_text_align_vertical(
        &self,
        _ctx: CallContext,
    ) -> RpcResult<String> {
        info!("ClosedCaptions.textAlignVertical");
        let data = GetStorageProperty {
            namespace: "ClosedCaptions".to_string(),
            key: "textAlignVertical".to_string(),
        };
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(StorageRequest::Get(data))
            .await
        {
            if let Some(ExtnResponse::String(v)) = response.payload.clone().extract() {
                return Ok(v);
            }
        }
        Err(rpc_err("ClosedCaptions.textAlignVertical is not available"))
    }

    async fn closed_captions_settings_text_align_vertical_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        info!("ClosedCaptions.setTextAlignVertical");
        let data = SetStorageProperty {
            namespace: "ClosedCaptions".to_string(),
            key: "textAlignVertical".to_string(),
            value: json!(set_request.value),
        };
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(StorageRequest::Set(data))
            .await
        {
            if let Some(ExtnResponse::Boolean(_v)) = response.payload.clone().extract() {
                return Ok(());
            }
        }
        Err(rpc_err("FB error response TBD"))
    }

    async fn closed_captions_settings_text_align_vertical_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_app_event(
            ctx,
            request,
            "ClosedCaptionsSettingsTextAlignVerticalChanged",
            CLOSED_CAPTION_TEXT_ALIGN_VERTICAL_CHANGED_EVENT,
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
