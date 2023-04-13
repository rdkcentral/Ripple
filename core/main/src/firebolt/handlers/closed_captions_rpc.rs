use crate::{
    api::rpc::rpc_gateway::{CallContext, RPCProvider},
    apps::app_events::{
        AppEventDecorationError, AppEventDecorator, AppEvents, AppEventsState, ListenRequest,
        ListenerResponse,
    },
    helpers::{
        ripple_helper::{IRippleHelper, RippleHelperFactory, RippleHelperType},
        serde_utils::opacity_serde,
    },
    managers::{
        capability_manager::{
            CapClassifiedRequest, FireboltCap, IGetLoadedCaps, RippleHandlerCaps,
        },
        event::event_administrator::{DabEventAdministrator, DabEventHandler},
        storage::{
            storage_manager::StorageManager,
            storage_property::{
                StorageProperty, EVENT_CLOSED_CAPTIONS_BACKGROUND_COLOR,
                EVENT_CLOSED_CAPTIONS_BACKGROUND_OPACITY, EVENT_CLOSED_CAPTIONS_ENABLED,
                EVENT_CLOSED_CAPTIONS_FONT_COLOR, EVENT_CLOSED_CAPTIONS_FONT_EDGE,
                EVENT_CLOSED_CAPTIONS_FONT_EDGE_COLOR, EVENT_CLOSED_CAPTIONS_FONT_FAMILY,
                EVENT_CLOSED_CAPTIONS_FONT_OPACITY, EVENT_CLOSED_CAPTIONS_FONT_SIZE,
                EVENT_CLOSED_CAPTIONS_SETTINGS_CHANGED, EVENT_CLOSED_CAPTIONS_TEXT_ALIGN,
                EVENT_CLOSED_CAPTIONS_TEXT_ALIGN_VERTICAL,
            },
        },
    },
    platform_state::PlatformState,
};
use dab::core::{
    message::{DabEvent, DabRequestPayload, DabResponsePayload, DabSubscribeMessage},
    model::device::{DeviceEvent, DeviceRequest, EVENT_VOICE_GUIDANCE_ENABLED_CHANGED},
    model::{
        distributor::{DistributorRequest, DistributorSession},
        persistent_store::{StorageData, StorageRequest},
    },
};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    RpcModule,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{error, instrument};

pub const VOICE_GUIDANCE_SETTINGS_CHANGED_EVENT: &'static str =
    "accessibility.onVoiceGuidanceSettingsChanged";
pub const VOICE_GUIDANCE_ENABLED_CHANGED_EVENT: &'static str = "voiceguidance.onEnabledChanged";
pub const VOICE_GUIDANCE_SPEED_CHANGED_EVENT: &'static str = "voiceguidance.onSpeedChanged";

#[derive(Deserialize, Debug)]
pub struct SetBoolProperty {
    pub value: bool,
}

#[derive(Deserialize, Debug)]
pub struct SetStringProperty {
    pub value: String,
}

#[derive(Deserialize, Debug)]
pub struct SetU32Property {
    pub value: u32,
}

#[derive(Deserialize, Debug)]
pub struct SetF32Property {
    pub value: f32,
}

#[derive(Deserialize, Debug)]
pub struct OpacityProperty {
    #[serde(with = "opacity_serde")]
    pub value: u32,
}

#[derive(Serialize, Deserialize)]
pub struct ClosedCaptionsSettings {
    enabled: bool,
    styles: ClosedCaptionStyle,
}

pub const FONT_FAMILY_LIST: [&str; 5] = ["sans-serif", "serif", "monospace", "cursive", "fantasy"];
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClosedCaptionStyle {
    pub font_family: String,
    pub font_size: f32,
    pub font_color: String,
    pub font_edge: String,
    pub font_edge_color: String,
    pub font_opacity: u32,
    pub background_color: String,
    pub background_opacity: u32,
    pub text_align: String,
    pub text_align_vertical: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceGuidanceSettings {
    enabled: bool,
    speed: f32,
}

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct VoiceGuidanceEnabledChangedEventData {
    pub state: bool,
}

#[derive(Clone)]
struct VoiceGuidanceEnabledChangedEventHandler {}

#[async_trait]
impl DabEventHandler for VoiceGuidanceEnabledChangedEventHandler {
    async fn handle_dab_event(
        &self,
        ps: &PlatformState,
        _cur_value: &mut Option<Value>,
        dab_event: &DabEvent,
    ) {
        match dab_event {
            DabEvent::Device(DeviceEvent::VoiceGuidanceEnabledChangedEvent(enabled)) => {
                AppEvents::emit(
                    &ps,
                    &VOICE_GUIDANCE_ENABLED_CHANGED_EVENT.to_string(),
                    &serde_json::to_value(enabled).unwrap(),
                )
                .await;

                AppEvents::emit(
                    &ps,
                    &VOICE_GUIDANCE_SETTINGS_CHANGED_EVENT.to_string(),
                    &serde_json::to_value(enabled).unwrap(),
                )
                .await;
            }

            _ => {
                error!("Invalid dab_event received");
            }
        }
    }

    fn generate_dab_event_subscribe_request(
        &self,
        ctx: CallContext,
        listen: bool,
    ) -> DabRequestPayload {
        let subscribe_message = DabSubscribeMessage {
            subscribe: listen,
            context: Some(serde_json::to_string(&ctx).unwrap()),
        };

        DabRequestPayload::Device(DeviceRequest::OnVoiceGuidanceEnabledChanged(
            subscribe_message,
        ))
    }

    fn get_mapped_dab_event_name(&self) -> &str {
        EVENT_VOICE_GUIDANCE_ENABLED_CHANGED
    }

    fn dab_event_handler_clone(&self) -> Box<dyn DabEventHandler + Send + Sync> {
        Box::new(self.clone())
    }
}

#[rpc(server)]
pub trait Accessibility {
    #[method(name = "accessibility.closedCaptions")]
    async fn closed_captions(&self, ctx: CallContext) -> RpcResult<ClosedCaptionsSettings>;
    #[method(name = "accessibility.closedCaptionsSettings")]
    async fn closed_captions_settings(&self, ctx: CallContext)
        -> RpcResult<ClosedCaptionsSettings>;
    #[method(name = "accessibility.onClosedCaptionsSettingsChanged")]
    async fn on_closed_captions_settings_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "accessibility.voiceGuidanceSettings", aliases = ["accessibility.voiceGuidance"])]
    async fn voice_guidance_settings(&self, ctx: CallContext) -> RpcResult<VoiceGuidanceSettings>;
    #[method(name = "accessibility.onVoiceGuidanceSettingsChanged")]
    async fn on_voice_guidance_settings_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.enabled")]
    async fn closed_captions_settings_enabled_rpc(&self, ctx: CallContext) -> RpcResult<bool>;
    #[method(name = "closedcaptions.setEnabled")]
    async fn closed_captions_settings_enabled_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onEnabledChanged")]
    async fn closed_captions_settings_enabled_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.fontFamily")]
    async fn closed_captions_settings_font_family(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "closedcaptions.setFontFamily")]
    async fn closed_captions_settings_font_family_set(
        &self,
        ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onFontFamilyChanged")]
    async fn closed_captions_settings_font_family_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.fontSize")]
    async fn closed_captions_settings_font_size(&self, ctx: CallContext) -> RpcResult<f32>;
    #[method(name = "closedcaptions.setFontSize")]
    async fn closed_captions_settings_font_size_set(
        &self,
        ctx: CallContext,
        set_request: SetF32Property,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onFontSizeChanged")]
    async fn closed_captions_settings_font_size_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.fontColor")]
    async fn closed_captions_settings_font_color(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "closedcaptions.setFontColor")]
    async fn closed_captions_settings_font_color_set(
        &self,
        ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onFontColorChanged")]
    async fn closed_captions_settings_font_color_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.fontEdge")]
    async fn closed_captions_settings_font_edge(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "closedcaptions.setFontEdge")]
    async fn closed_captions_settings_font_edge_set(
        &self,
        ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onFontEdgeChanged")]
    async fn closed_captions_settings_font_edge_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.fontEdgeColor")]
    async fn closed_captions_settings_font_edge_color(&self, ctx: CallContext)
        -> RpcResult<String>;
    #[method(name = "closedcaptions.setFontEdgeColor")]
    async fn closed_captions_settings_font_edge_color_set(
        &self,
        ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onFontEdgeColorChanged")]
    async fn closed_captions_settings_font_edge_color_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.fontOpacity")]
    async fn closed_captions_settings_font_opacity(&self, ctx: CallContext) -> RpcResult<u32>;
    #[method(name = "closedcaptions.setFontOpacity")]
    async fn closed_captions_settings_font_opacity_set(
        &self,
        ctx: CallContext,
        set_request: OpacityProperty,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onFontOpacityChanged")]
    async fn closed_captions_settings_font_opacity_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.backgroundColor")]
    async fn closed_captions_settings_background_color(
        &self,
        ctx: CallContext,
    ) -> RpcResult<String>;
    #[method(name = "closedcaptions.setBackgroundColor")]
    async fn closed_captions_settings_background_color_set(
        &self,
        ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onBackgroundColorChanged")]
    async fn closed_captions_settings_background_color_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.backgroundOpacity")]
    async fn closed_captions_settings_background_opacity(&self, ctx: CallContext)
        -> RpcResult<u32>;
    #[method(name = "closedcaptions.setBackgroundOpacity")]
    async fn closed_captions_settings_background_opacity_set(
        &self,
        ctx: CallContext,
        set_request: OpacityProperty,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onBackgroundOpacityChanged")]
    async fn closed_captions_settings_background_opacity_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.textAlign")]
    async fn closed_captions_settings_text_align(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "closedcaptions.setTextAlign")]
    async fn closed_captions_settings_text_align_set(
        &self,
        ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onTextAlignChanged")]
    async fn closed_captions_settings_text_align_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "closedcaptions.textAlignVertical")]
    async fn closed_captions_settings_text_align_vertical(
        &self,
        ctx: CallContext,
    ) -> RpcResult<String>;
    #[method(name = "closedcaptions.setTextAlignVertical")]
    async fn closed_captions_settings_text_align_vertical_set(
        &self,
        ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()>;
    #[method(name = "closedcaptions.onTextAlignVerticalChanged")]
    async fn closed_captions_settings_text_align_vertical_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "voiceguidance.enabled")]
    async fn voice_guidance_settings_enabled_rpc(&self, ctx: CallContext) -> RpcResult<bool>;
    #[method(name = "voiceguidance.setEnabled")]
    async fn voice_guidance_settings_enabled_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()>;
    #[method(name = "voiceguidance.onEnabledChanged")]
    async fn voice_guidance_settings_enabled_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "voiceguidance.speed")]
    async fn voice_guidance_settings_speed_rpc(&self, ctx: CallContext) -> RpcResult<f32>;
    #[method(name = "voiceguidance.setSpeed")]
    async fn voice_guidance_settings_speed_set(
        &self,
        ctx: CallContext,
        set_request: SetF32Property,
    ) -> RpcResult<()>;
    #[method(name = "voiceguidance.onSpeedChanged")]
    async fn voice_guidance_settings_speed_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
}

#[derive(Debug)]
pub struct AccessibilityImpl {
    pub platform_state: PlatformState,
}

impl AccessibilityImpl {
    fn is_font_family_supported(font_family: &str) -> bool {
        FONT_FAMILY_LIST.contains(&font_family)
    }

    pub async fn voice_guidance_settings_enabled(state: &PlatformState) -> RpcResult<bool> {
        let resp = state
            .services
            .send_dab(DabRequestPayload::Device(
                DeviceRequest::VoiceGuidanceEnabled,
            ))
            .await;
        match resp {
            Ok(dab_payload) => match dab_payload {
                DabResponsePayload::VoiceGuidanceEnabledResponse(enabled) => Ok(enabled),
                _ => Err(jsonrpsee::core::Error::Custom(String::from(
                    "Voice guidance enabled error response TBD1",
                ))),
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "Voice guidance enabled error response TBD2",
            ))),
        }
    }

    pub async fn voice_guidance_settings_speed(state: &PlatformState) -> RpcResult<f32> {
        let resp = state
            .services
            .send_dab(DabRequestPayload::Device(DeviceRequest::VoiceGuidanceSpeed))
            .await;
        match resp {
            Ok(dab_payload) => match dab_payload {
                DabResponsePayload::VoiceGuidanceSpeedResponse(speed) => Ok(speed),
                _ => Err(jsonrpsee::core::Error::Custom(String::from(
                    "Voice guidance speed error response TBD3",
                ))),
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "Voice guidance speed error response TBD4",
            ))),
        }
    }

    pub async fn closed_captions_settings_enabled(state: &PlatformState) -> RpcResult<bool> {
        StorageManager::get_bool(state, StorageProperty::ClosedCaptionsEnabled).await
    }

    async fn on_setting_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
        event_name: &'static str,
    ) -> RpcResult<ListenerResponse> {
        on_setting_changed(
            &self.platform_state.app_events_state,
            &ctx,
            &request,
            event_name,
        )
    }
}

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
        use crate::managers::storage::storage_manager::StorageManager as SM;
        use StorageProperty::*;
        let enabled_res = AccessibilityImpl::closed_captions_settings_enabled(&ps).await;
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

#[derive(Clone)]
struct VGEventDecorator {}

#[async_trait]
impl AppEventDecorator for VGEventDecorator {
    async fn decorate(
        &self,
        ps: &PlatformState,
        _ctx: &CallContext,
        _event_name: &str,
        _val_in: &Value,
    ) -> Result<Value, AppEventDecorationError> {
        let enabled = AccessibilityImpl::voice_guidance_settings_enabled(&ps).await?;
        let speed = AccessibilityImpl::voice_guidance_settings_speed(&ps).await?;
        Ok(serde_json::to_value(VoiceGuidanceSettings { enabled, speed }).unwrap())
    }

    fn dec_clone(&self) -> Box<dyn AppEventDecorator + Send + Sync> {
        Box::new(self.clone())
    }
}


/*
Free function to allow variability of event_name
*/
async fn voice_guidance_settings_enabled_changed(
    platform_state: &PlatformState,
    ctx: &CallContext,
    request: &ListenRequest,
    event_name: &str,
) -> RpcResult<ListenerResponse> {
    let listen = request.listen;
    AppEvents::add_listener(
        &platform_state.app_events_state,
        String::from(event_name),
        ctx.clone(),
        request.clone(),
    );

    if listen {
        let _ = DabEventAdministrator::get(platform_state.clone())
            .dab_event_subscribe(
                ctx.clone(),
                String::from(event_name),
                None,
                Some(Box::new(VoiceGuidanceEnabledChangedEventHandler {})),
            )
            .await;
    } else {
        let _ = DabEventAdministrator::get(platform_state.clone())
            .dab_event_unsubscribe(
                ctx.clone(),
                String::from(event_name),
                Some(Box::new(VoiceGuidanceEnabledChangedEventHandler {})),
            )
            .await;
    }

    Ok(ListenerResponse {
        listening: listen,
        event: VOICE_GUIDANCE_ENABLED_CHANGED_EVENT,
    })
}
fn on_setting_changed(
    app_events_state: &AppEventsState,
    ctx: &CallContext,
    request: &ListenRequest,
    event_name: &'static str,
) -> RpcResult<ListenerResponse> {
    let listen = request.listen;
    AppEvents::add_listener(
        app_events_state,
        event_name.to_string(),
        ctx.clone(),
        request.clone(),
    );
    Ok(ListenerResponse {
        listening: listen,
        event: event_name,
    })
}

#[async_trait]
impl AccessibilityServer for AccessibilityImpl {
    #[instrument(skip(self))]
    async fn closed_captions(&self, ctx: CallContext) -> RpcResult<ClosedCaptionsSettings> {
        self.closed_captions_settings(ctx).await
    }
    #[instrument(skip(self))]
    async fn closed_captions_settings(
        &self,
        ctx: CallContext,
    ) -> RpcResult<ClosedCaptionsSettings> {
        let styles: ClosedCaptionStyle = ClosedCaptionStyle {
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
            styles: styles,
        })
    }
    #[instrument(skip(self))]
    async fn on_closed_captions_settings_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        AppEvents::add_listener_with_decorator(
            &&self.platform_state.app_events_state,
            EVENT_CLOSED_CAPTIONS_SETTINGS_CHANGED.to_string(),
            ctx,
            request,
            Some(Box::new(CCEventDecorator {})),
        );
        Ok(ListenerResponse {
            listening: listen,
            event: EVENT_CLOSED_CAPTIONS_SETTINGS_CHANGED,
        })
    }

    #[instrument(skip(self))]
    async fn voice_guidance_settings(&self, ctx: CallContext) -> RpcResult<VoiceGuidanceSettings> {
        Ok(VoiceGuidanceSettings {
            enabled: self
                .voice_guidance_settings_enabled_rpc(ctx.clone())
                .await?,
            speed: self.voice_guidance_settings_speed_rpc(ctx.clone()).await?,
        })
    }
    #[instrument(skip(self))]
    async fn on_voice_guidance_settings_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;

        // Register for individual change events (no-op if already registered), handlers emit VOICE_GUIDANCE_SETTINGS_CHANGED_EVENT.
        voice_guidance_settings_enabled_changed(
            &self.platform_state,
            &ctx,
            &request,
            VOICE_GUIDANCE_SETTINGS_CHANGED_EVENT,
        )
        .await
        .ok();
        /*
        Add decorated listener after call to voice_guidance_settings_enabled_changed to make decorated listener current  */
        AppEvents::add_listener_with_decorator(
            &&self.platform_state.app_events_state,
            VOICE_GUIDANCE_SETTINGS_CHANGED_EVENT.to_string(),
            ctx.clone(),
            request.clone(),
            Some(Box::new(VGEventDecorator {})),
        );

        Ok(ListenerResponse {
            listening: listen,
            event: VOICE_GUIDANCE_SETTINGS_CHANGED_EVENT,
        })
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_enabled_rpc(&self, _ctx: CallContext) -> RpcResult<bool> {
        AccessibilityImpl::closed_captions_settings_enabled(&self.platform_state).await
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_enabled_set(
        &self,
        _ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        StorageManager::set_bool(
            &self.platform_state,
            StorageProperty::ClosedCaptionsEnabled,
            set_request.value,
            None,
        )
        .await
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_enabled_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_setting_changed(ctx, request, EVENT_CLOSED_CAPTIONS_ENABLED)
            .await
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_font_family(&self, _ctx: CallContext) -> RpcResult<String> {
        StorageManager::get_string(
            &self.platform_state,
            StorageProperty::ClosedCaptionsFontFamily,
        )
        .await
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_font_family_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        /*
         * Not implemented as a custom serde because this is not a custom datatype.
         */
        if AccessibilityImpl::is_font_family_supported(set_request.value.as_str()) {
            StorageManager::set_string(
                &self.platform_state,
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

    #[instrument(skip(self))]
    async fn closed_captions_settings_font_family_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        self.on_setting_changed(ctx, request, EVENT_CLOSED_CAPTIONS_FONT_FAMILY)
            .await
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_font_size(&self, _ctx: CallContext) -> RpcResult<f32> {
        StorageManager::get_number_as_f32(
            &self.platform_state,
            StorageProperty::ClosedCaptionsFontSize,
        )
        .await
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_font_size_set(
        &self,
        _ctx: CallContext,
        set_request: SetF32Property,
    ) -> RpcResult<()> {
        if set_request.value >= 0.5 && set_request.value <= 2.0 {
            StorageManager::set_number_as_f32(
                &self.platform_state,
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

    #[instrument(skip(self))]
    async fn closed_captions_settings_font_size_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_setting_changed(ctx, request, EVENT_CLOSED_CAPTIONS_FONT_SIZE)
            .await
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_font_color(&self, _ctx: CallContext) -> RpcResult<String> {
        StorageManager::get_string(
            &self.platform_state,
            StorageProperty::ClosedCaptionsFontColor,
        )
        .await
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_font_color_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        StorageManager::set_string(
            &self.platform_state,
            StorageProperty::ClosedCaptionsFontColor,
            set_request.value,
            None,
        )
        .await
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_font_color_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_setting_changed(ctx, request, EVENT_CLOSED_CAPTIONS_FONT_COLOR)
            .await
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_font_edge(&self, _ctx: CallContext) -> RpcResult<String> {
        StorageManager::get_string(
            &self.platform_state,
            StorageProperty::ClosedCaptionsFontEdge,
        )
        .await
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_font_edge_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        StorageManager::set_string(
            &self.platform_state,
            StorageProperty::ClosedCaptionsFontEdge,
            set_request.value,
            None,
        )
        .await
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_font_edge_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_setting_changed(ctx, request, EVENT_CLOSED_CAPTIONS_FONT_EDGE)
            .await
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_font_edge_color(
        &self,
        _ctx: CallContext,
    ) -> RpcResult<String> {
        StorageManager::get_string(
            &self.platform_state,
            StorageProperty::ClosedCaptionsFontEdgeColor,
        )
        .await
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_font_edge_color_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        StorageManager::set_string(
            &self.platform_state,
            StorageProperty::ClosedCaptionsFontEdgeColor,
            set_request.value,
            None,
        )
        .await
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_font_edge_color_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_setting_changed(ctx, request, EVENT_CLOSED_CAPTIONS_FONT_EDGE_COLOR)
            .await
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_font_opacity(&self, _ctx: CallContext) -> RpcResult<u32> {
        StorageManager::get_number_as_u32(
            &self.platform_state,
            StorageProperty::ClosedCaptionsFontOpacity,
        )
        .await
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_font_opacity_set(
        &self,
        _ctx: CallContext,
        set_request: OpacityProperty,
    ) -> RpcResult<()> {
        StorageManager::set_number_as_u32(
            &self.platform_state,
            StorageProperty::ClosedCaptionsFontOpacity,
            set_request.value,
            None,
        )
        .await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_font_opacity_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_setting_changed(ctx, request, EVENT_CLOSED_CAPTIONS_FONT_OPACITY)
            .await
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_background_color(
        &self,
        _ctx: CallContext,
    ) -> RpcResult<String> {
        StorageManager::get_string(
            &self.platform_state,
            StorageProperty::ClosedCaptionsBackgroundColor,
        )
        .await
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_background_color_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        StorageManager::set_string(
            &self.platform_state,
            StorageProperty::ClosedCaptionsBackgroundColor,
            set_request.value,
            None,
        )
        .await
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_background_color_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_setting_changed(ctx, request, EVENT_CLOSED_CAPTIONS_BACKGROUND_COLOR)
            .await
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_background_opacity(
        &self,
        _ctx: CallContext,
    ) -> RpcResult<u32> {
        StorageManager::get_number_as_u32(
            &self.platform_state,
            StorageProperty::ClosedCaptionsBackgroundOpacity,
        )
        .await
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_background_opacity_set(
        &self,
        _ctx: CallContext,
        set_request: OpacityProperty,
    ) -> RpcResult<()> {
        StorageManager::set_number_as_u32(
            &self.platform_state,
            StorageProperty::ClosedCaptionsBackgroundOpacity,
            set_request.value,
            None,
        )
        .await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_background_opacity_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_setting_changed(ctx, request, EVENT_CLOSED_CAPTIONS_BACKGROUND_OPACITY)
            .await
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_text_align(&self, _ctx: CallContext) -> RpcResult<String> {
        StorageManager::get_string(
            &self.platform_state,
            StorageProperty::ClosedCaptionsTextAlign,
        )
        .await
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_text_align_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        StorageManager::set_string(
            &self.platform_state,
            StorageProperty::ClosedCaptionsTextAlign,
            set_request.value,
            None,
        )
        .await
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_text_align_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_setting_changed(ctx, request, EVENT_CLOSED_CAPTIONS_TEXT_ALIGN)
            .await
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_text_align_vertical(
        &self,
        _ctx: CallContext,
    ) -> RpcResult<String> {
        StorageManager::get_string(
            &self.platform_state,
            StorageProperty::ClosedCaptionsTextAlignVertical,
        )
        .await
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_text_align_vertical_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        StorageManager::set_string(
            &self.platform_state,
            StorageProperty::ClosedCaptionsTextAlignVertical,
            set_request.value,
            None,
        )
        .await
    }

    #[instrument(skip(self))]
    async fn closed_captions_settings_text_align_vertical_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_setting_changed(ctx, request, EVENT_CLOSED_CAPTIONS_TEXT_ALIGN_VERTICAL)
            .await
    }

    #[instrument(skip(self))]
    async fn voice_guidance_settings_enabled_rpc(&self, _ctx: CallContext) -> RpcResult<bool> {
        AccessibilityImpl::voice_guidance_settings_enabled(&self.platform_state).await
    }

    #[instrument(skip(self))]
    async fn voice_guidance_settings_enabled_set(
        &self,
        _ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        let resp = self
            .platform_state
            .services
            .send_dab(DabRequestPayload::Device(
                DeviceRequest::VoiceGuidanceSetEnabled(set_request.value),
            ))
            .await;
        match resp {
            Ok(dab_payload) => match dab_payload {
                DabResponsePayload::VoiceGuidanceEnabledResponse(_enabled) => Ok(()),
                _ => Err(jsonrpsee::core::Error::Custom(String::from(
                    "Voice guidance enabled error response TBD",
                ))),
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "Voice guidance enabled error response TBD",
            ))),
        }
    }

    #[instrument(skip(self))]
    async fn voice_guidance_settings_enabled_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        voice_guidance_settings_enabled_changed(
            &self.platform_state,
            &ctx,
            &request,
            VOICE_GUIDANCE_ENABLED_CHANGED_EVENT,
        )
        .await
    }

    #[instrument(skip(self))]
    async fn voice_guidance_settings_speed_rpc(&self, _ctx: CallContext) -> RpcResult<f32> {
        AccessibilityImpl::voice_guidance_settings_speed(&self.platform_state).await
    }

    #[instrument(skip(self))]
    async fn voice_guidance_settings_speed_set(
        &self,
        _ctx: CallContext,
        set_request: SetF32Property,
    ) -> RpcResult<()> {
        if set_request.value >= 0.1 && set_request.value <= 10.0 {
            let resp = self
                .platform_state
                .services
                .send_dab(DabRequestPayload::Device(
                    DeviceRequest::VoiceGuidanceSetSpeed(set_request.value),
                ))
                .await;
            match resp {
                Ok(dab_payload) => match dab_payload {
                    DabResponsePayload::VoiceGuidanceSpeedResponse(speed) => {
                        // TODO: Thunder doesn't currently support a speed change listener so we emit an event here.
                        AppEvents::emit(
                            &self.platform_state,
                            &VOICE_GUIDANCE_SPEED_CHANGED_EVENT.to_string(),
                            &serde_json::to_value(speed).unwrap(),
                        )
                        .await;
                        Ok(())
                    }
                    _ => Err(jsonrpsee::core::Error::Custom(String::from(
                        "Voice guidance speed error response TBD",
                    ))),
                },
                Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                    "Voice guidance speed error response TBD",
                ))),
            }
        } else {
            Err(jsonrpsee::core::error::Error::Custom(
                "Invalid Value for set speed".to_owned(),
            ))
        }
    }

    #[instrument(skip(self))]
    async fn voice_guidance_settings_speed_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_setting_changed(ctx, request, VOICE_GUIDANCE_SPEED_CHANGED_EVENT)
            .await
    }
}

pub struct AccessibilityRippleProvider;
pub struct AccessibilityCapHandler;

impl IGetLoadedCaps for AccessibilityCapHandler {
    fn get_loaded_caps(&self) -> RippleHandlerCaps {
        RippleHandlerCaps {
            caps: Some(vec![CapClassifiedRequest::Supported(vec![
                FireboltCap::short("accessibility:closedcaptions"),
                FireboltCap::short("accessibility:voiceguidance"),
            ])]),
        }
    }
}

impl RPCProvider<AccessibilityImpl, AccessibilityCapHandler> for AccessibilityRippleProvider {
    fn provide(
        self,
        _rhf: Box<RippleHelperFactory>,
        platform_state: PlatformState,
    ) -> (RpcModule<AccessibilityImpl>, AccessibilityCapHandler) {
        let a = AccessibilityImpl { platform_state };
        (a.into_rpc(), AccessibilityCapHandler)
    }

    fn get_helper_variant(self) -> Vec<RippleHelperType> {
        vec![RippleHelperType::Dab, RippleHelperType::Dpab]
    }
}

