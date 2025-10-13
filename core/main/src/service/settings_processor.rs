use jsonrpsee::core::RpcResult;
use std::collections::HashMap;

use ripple_sdk::{
    api::{
        distributor::distributor_privacy::ContentListenRequest,
        firebolt::{
            fb_capabilities::{CapabilityRole, FireboltCap, RoleInfo},
            fb_general::ListenRequest,
        },
        gateway::rpc_gateway_api::CallContext,
        settings::{SettingKey, SettingValue, SettingsRequestParam},
        storage_property::{
            EVENT_ALLOW_PERSONALIZATION_CHANGED, EVENT_ALLOW_WATCH_HISTORY_CHANGED,
            EVENT_SHARE_WATCH_HISTORY,
        },
    },
    async_trait::async_trait,
    log::{debug, warn},
    utils::error::RippleError,
};
use serde_json::{json, Value};

use crate::{
    broker::broker_utils::{self, BrokerUtils},
    firebolt::handlers::{
        capabilities_rpc::is_permitted, closed_captions_rpc::ClosedcaptionsImpl,
        discovery_rpc::DiscoveryImpl, privacy_rpc::PrivacyImpl,
    },
    service::apps::app_events::{AppEventDecorationError, AppEventDecorator, AppEvents},
    state::platform_state::PlatformState,
};

#[derive(Clone, Debug)]
struct SettingsChangeEventDecorator {
    request: SettingsRequestParam,
}

#[async_trait]
impl AppEventDecorator for SettingsChangeEventDecorator {
    async fn decorate(
        &self,
        state: &PlatformState,
        _ctx: &CallContext,
        _event_name: &str,
        _val: &Value,
    ) -> Result<Value, AppEventDecorationError> {
        if let Ok(v) = get_settings_map(state, &self.request).await {
            Ok(serde_json::to_value(v).unwrap())
        } else {
            Ok(json!({}))
        }
    }

    fn dec_clone(&self) -> Box<dyn AppEventDecorator + Send + Sync> {
        Box::new(self.clone())
    }
}

pub async fn get_settings_map(
    state: &PlatformState,
    request: &SettingsRequestParam,
) -> Result<HashMap<String, SettingValue>, RippleError> {
    let ctx = request.context.clone();
    if let Ok(cp) =
        DiscoveryImpl::get_content_policy(&request.context, state, &request.context.app_id).await
    {
        let mut settings = HashMap::default();
        for sk in request.keys.clone() {
            let val = match sk {
                SettingKey::VoiceGuidanceEnabled => {
                    let enabled = voice_guidance_settings_enabled(state)
                        .await
                        .unwrap_or(false);
                    Some(SettingValue::bool(enabled))
                }
                SettingKey::ClosedCaptions => {
                    let enabled = ClosedcaptionsImpl::cc_enabled(state).await.unwrap_or(false);
                    Some(SettingValue::bool(enabled))
                }
                SettingKey::AllowPersonalization => {
                    Some(SettingValue::bool(cp.enable_recommendations))
                }
                SettingKey::AllowWatchHistory => {
                    Some(SettingValue::bool(cp.remember_watched_programs))
                }
                SettingKey::ShareWatchHistory => Some(SettingValue::bool(cp.share_watch_history)),
                SettingKey::DeviceName => {
                    let s = state.clone();
                    Some(SettingValue::string(
                        broker_utils::BrokerUtils::process_internal_main_request(
                            &s,
                            "device.name",
                            None,
                        )
                        .await
                        .unwrap_or_else(|_| "".into())
                        .to_string(),
                    ))
                }
                SettingKey::PowerSaving => Some(SettingValue::bool(true)),
                SettingKey::LegacyMiniGuide => Some(SettingValue::bool(false)),
            };

            if let Some(v) = val {
                let role_info = RoleInfo {
                    role: Some(CapabilityRole::Use),
                    capability: FireboltCap::Short(sk.use_capability().into()),
                };
                if let Ok(result) = is_permitted(state, &ctx, &role_info).await {
                    if result {
                        settings.insert(request.get_alias(&sk), v);
                    }
                }
            }
        }
        return Ok(settings);
    }
    Err(RippleError::InvalidOutput)
}

async fn voice_guidance_settings_enabled(state: &PlatformState) -> RpcResult<bool> {
    match BrokerUtils::process_internal_main_request(&state.clone(), "voiceguidance.enabled", None)
        .await
    {
        Ok(enabled_value) => {
            if let Value::Bool(enabled) = enabled_value {
                Ok(enabled)
            } else {
                Err(jsonrpsee::core::Error::Custom(String::from(
                    "voice_guidance_settings_enabled: Unexpected value type",
                )))
            }
        }
        Err(e) => Err(jsonrpsee::core::Error::Custom(format!(
            "voice_guidance_settings_enabled: e={}",
            e
        ))),
    }
}

fn subscribe_event(
    state: &PlatformState,
    ctx: CallContext,
    event_name: &str,
    request: SettingsRequestParam,
) -> bool {
    AppEvents::add_listener_with_decorator(
        state,
        event_name.to_string(),
        ctx,
        ListenRequest { listen: true },
        Some(Box::new(SettingsChangeEventDecorator { request })),
    );
    true
}

pub async fn subscribe_to_settings(state: &PlatformState, request: SettingsRequestParam) -> bool {
    let mut resp = true;
    let ctx = &request.context;
    debug!("Incoming subscribe request");
    for key in &request.keys {
        debug!("Checking Key {:?}", key);
        match key {
            SettingKey::VoiceGuidanceEnabled => {
                resp = BrokerUtils::process_internal_request(
                    &state.clone(),
                    Some(request.context.clone()),
                    "sts.voiceguidance.onEnabledChanged",
                    serde_json::to_value(json!({"listen": true})).ok(),
                )
                .await
                .is_ok();
            }
            SettingKey::ClosedCaptions => {
                resp = BrokerUtils::process_internal_request(
                    &state.clone(),
                    Some(request.context.clone()),
                    "sts.closedcaptions.onEnabledChanged",
                    serde_json::to_value(json!({"listen": true})).ok(),
                )
                .await
                .is_ok();
            }
            SettingKey::AllowPersonalization => {
                if PrivacyImpl::listen_content_policy_changed(
                    state,
                    true,
                    ctx,
                    EVENT_ALLOW_PERSONALIZATION_CHANGED,
                    Some(ContentListenRequest {
                        listen: true,
                        app_id: None,
                    }),
                    Some(Box::new(SettingsChangeEventDecorator {
                        request: request.clone(),
                    })),
                )
                .is_err()
                {
                    resp = false;
                }
            }
            SettingKey::AllowWatchHistory => {
                if PrivacyImpl::listen_content_policy_changed(
                    state,
                    true,
                    ctx,
                    EVENT_ALLOW_WATCH_HISTORY_CHANGED,
                    Some(ContentListenRequest {
                        listen: true,
                        app_id: None,
                    }),
                    Some(Box::new(SettingsChangeEventDecorator {
                        request: request.clone(),
                    })),
                )
                .is_err()
                {
                    resp = false;
                }
            }
            SettingKey::ShareWatchHistory => {
                if !subscribe_event(
                    state,
                    ctx.clone(),
                    EVENT_SHARE_WATCH_HISTORY,
                    request.clone(),
                ) {
                    resp = false;
                }
            }
            SettingKey::DeviceName => {
                resp = BrokerUtils::process_internal_request(
                    &state.clone(),
                    Some(request.context.clone()),
                    "sts.device.onNameChanged",
                    serde_json::to_value(json!({"listen": true})).ok(),
                )
                .await
                .is_ok();
            }
            SettingKey::PowerSaving | SettingKey::LegacyMiniGuide => {
                warn!("{} Not implemented", key.to_string());
            }
        }
    }
    resp
}
