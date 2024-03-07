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

use crate::processor::storage::storage_manager::StorageManager;
use crate::service::apps::app_events::AppEventDecorator;
use crate::{
    firebolt::rpc::RippleRPCProvider, service::apps::app_events::AppEvents,
    state::platform_state::PlatformState,
};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    types::error::CallError,
    RpcModule,
};
use ripple_sdk::api::distributor::distributor_privacy::PrivacySettingsStoreRequest;
use ripple_sdk::api::firebolt::fb_capabilities::FireboltCap;
use ripple_sdk::extn::extn_client_message::ExtnPayload;
use ripple_sdk::{
    api::{
        device::device_peristence::SetBoolProperty,
        distributor::distributor_privacy::{
            ContentListenRequest, GetPropertyParams, PrivacyCloudRequest, PrivacySettings,
            SetPropertyParams,
        },
        firebolt::{
            fb_capabilities::{CapabilityRole, RoleInfo, CAPABILITY_NOT_AVAILABLE},
            fb_general::{ListenRequest, ListenerResponse},
        },
        gateway::rpc_gateway_api::CallContext,
        storage_property::{
            StorageProperty, EVENT_ALLOW_ACR_COLLECTION_CHANGED,
            EVENT_ALLOW_APP_CONTENT_AD_TARGETING_CHANGED, EVENT_ALLOW_CAMERA_ANALYTICS_CHANGED,
            EVENT_ALLOW_PERSONALIZATION_CHANGED, EVENT_ALLOW_PRIMARY_BROWSE_AD_TARGETING_CHANGED,
            EVENT_ALLOW_PRIMARY_CONTENT_AD_TARGETING_CHANGED,
            EVENT_ALLOW_PRODUCT_ANALYTICS_CHANGED, EVENT_ALLOW_REMOTE_DIAGNOSTICS_CHANGED,
            EVENT_ALLOW_RESUME_POINTS_CHANGED, EVENT_ALLOW_UNENTITLED_PERSONALIZATION_CHANGED,
            EVENT_ALLOW_UNENTITLED_RESUME_POINTS_CHANGED, EVENT_ALLOW_WATCH_HISTORY_CHANGED,
        },
    },
    extn::extn_client_message::ExtnResponse,
    log::debug,
    serde_json::json,
};

use std::collections::HashMap;

use super::advertising_rpc::ScopeOption;
use super::capabilities_rpc::is_granted;

pub const US_PRIVACY_KEY: &str = "us_privacy";
pub const LMT_KEY: &str = "lmt";

#[derive(Debug, Clone)]
struct AllowAppContentAdTargetingSettings {
    lmt: String,
    us_privacy: String,
}

impl AllowAppContentAdTargetingSettings {
    pub fn new(allow_app_content_ad_targeting: bool) -> Self {
        let (lmt, us_privacy) = match allow_app_content_ad_targeting {
            true => ("0", "1-N-"),
            false => ("1", "1-Y-"),
        };
        AllowAppContentAdTargetingSettings {
            lmt: lmt.to_owned(),
            us_privacy: us_privacy.to_owned(),
        }
    }

    pub fn get_allow_app_content_ad_targeting_settings(&self) -> HashMap<String, String> {
        HashMap::from([
            (US_PRIVACY_KEY.to_owned(), self.us_privacy.to_owned()),
            (LMT_KEY.to_owned(), self.lmt.to_owned()),
        ])
    }
}
impl Default for AllowAppContentAdTargetingSettings {
    fn default() -> Self {
        Self {
            lmt: "0".to_owned(),
            us_privacy: "1-N-".to_owned(),
        }
    }
}
#[rpc(server)]
pub trait Privacy {
    #[method(name = "privacy.allowACRCollection")]
    async fn privacy_allow_acr_collection(&self, ctx: CallContext) -> RpcResult<bool>;
    #[method(name = "privacy.setAllowACRCollection")]
    async fn privacy_allow_acr_collection_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()>;
    #[method(name = "privacy.onAllowACRCollectionChanged")]
    async fn privacy_allow_acr_collection_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "privacy.allowAppContentAdTargeting")]
    async fn privacy_allow_app_content_ad_targeting(&self, ctx: CallContext) -> RpcResult<bool>;
    #[method(name = "privacy.setAllowAppContentAdTargeting")]
    async fn privacy_allow_app_content_ad_targeting_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()>;
    #[method(name = "privacy.onAllowAppContentAdTargetingChanged")]
    async fn privacy_allow_app_content_ad_targeting_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "privacy.allowCameraAnalytics")]
    async fn privacy_allow_camera_analytics(&self, ctx: CallContext) -> RpcResult<bool>;
    #[method(name = "privacy.setAllowCameraAnalytics")]
    async fn privacy_allow_camera_analytics_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()>;
    #[method(name = "privacy.onAllowCameraAnalyticsChanged")]
    async fn privacy_allow_camera_analytics_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "privacy.allowPersonalization")]
    async fn privacy_allow_personalization(&self, ctx: CallContext) -> RpcResult<bool>;
    #[method(name = "privacy.setAllowPersonalization")]
    async fn privacy_allow_personalization_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()>;
    #[method(name = "privacy.onAllowPersonalizationChanged")]
    async fn privacy_allow_personalization_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "privacy.allowPrimaryBrowseAdTargeting")]
    async fn privacy_allow_primary_browse_ad_targeting(&self, ctx: CallContext) -> RpcResult<bool>;
    #[method(name = "privacy.setAllowPrimaryBrowseAdTargeting")]
    async fn privacy_allow_primary_browse_ad_targeting_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()>;
    #[method(name = "privacy.onAllowPrimaryBrowseAdTargetingChanged")]
    async fn privacy_allow_primary_browse_ad_targeting_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "privacy.allowPrimaryContentAdTargeting")]
    async fn privacy_allow_primary_content_ad_targeting(&self, ctx: CallContext)
        -> RpcResult<bool>;
    #[method(name = "privacy.setAllowPrimaryContentAdTargeting")]
    async fn privacy_allow_primary_content_ad_targeting_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()>;
    #[method(name = "privacy.onAllowPrimaryContentAdTargetingChanged")]
    async fn privacy_allow_primary_content_ad_targeting_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "privacy.allowProductAnalytics")]
    async fn privacy_allow_product_analytics(&self, ctx: CallContext) -> RpcResult<bool>;
    #[method(name = "privacy.setAllowProductAnalytics")]
    async fn privacy_allow_product_analytics_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()>;
    #[method(name = "privacy.onAllowProductAnalyticsChanged")]
    async fn privacy_allow_product_analytics_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "privacy.allowRemoteDiagnostics")]
    async fn privacy_allow_remote_diagnostics(&self, ctx: CallContext) -> RpcResult<bool>;
    #[method(name = "privacy.setAllowRemoteDiagnostics")]
    async fn privacy_allow_remote_diagnostics_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()>;
    #[method(name = "privacy.onAllowRemoteDiagnosticsChanged")]
    async fn privacy_allow_remote_diagnostics_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "privacy.allowResumePoints")]
    async fn privacy_allow_resume_points(&self, ctx: CallContext) -> RpcResult<bool>;
    #[method(name = "privacy.setAllowResumePoints")]
    async fn privacy_allow_resume_points_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()>;
    #[method(name = "privacy.onAllowResumePointsChanged")]
    async fn privacy_allow_resume_points_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "privacy.allowUnentitledPersonalization")]
    async fn privacy_allow_unentitled_personalization(&self, ctx: CallContext) -> RpcResult<bool>;
    #[method(name = "privacy.setAllowUnentitledPersonalization")]
    async fn privacy_allow_unentitled_personalization_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()>;
    #[method(name = "privacy.onAllowUnentitledPersonalizationChanged")]
    async fn privacy_allow_unentitled_personalization_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "privacy.allowUnentitledResumePoints")]
    async fn privacy_allow_unentitled_resume_points(&self, ctx: CallContext) -> RpcResult<bool>;
    #[method(name = "privacy.setAllowUnentitledResumePoints")]
    async fn privacy_allow_unentitled_resume_points_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()>;
    #[method(name = "privacy.onAllowUnentitledResumePointsChanged")]
    async fn privacy_allow_unentitled_resume_points_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "privacy.allowWatchHistory")]
    async fn privacy_allow_watch_history(&self, ctx: CallContext) -> RpcResult<bool>;
    #[method(name = "privacy.setAllowWatchHistory")]
    async fn privacy_allow_watch_history_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()>;
    #[method(name = "privacy.onAllowWatchHistoryChanged")]
    async fn privacy_allow_watch_history_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "privacy.settings")]
    async fn get_settings(&self, ctx: CallContext) -> RpcResult<PrivacySettings>;
}

pub async fn get_allow_app_content_ad_targeting_settings(
    platform_state: &PlatformState,
    scope_option: Option<&ScopeOption>,
    caller_app: &String,
) -> HashMap<String, String> {
    let mut data = StorageProperty::AllowAppContentAdTargeting;
    if let Some(scope_opt) = scope_option {
        if let Some(scope) = &scope_opt.scope {
            let primary_app = platform_state
                .get_device_manifest()
                .applications
                .defaults
                .main;
            if primary_app == *caller_app.to_string() {
                if scope._type.as_string() == "browse" {
                    data = StorageProperty::AllowPrimaryBrowseAdTargeting;
                } else if scope._type.as_string() == "content" {
                    data = StorageProperty::AllowPrimaryContentAdTargeting;
                }
            }
        }
    }

    AllowAppContentAdTargetingSettings::new(
        StorageManager::get_bool(platform_state, data)
            .await
            .unwrap_or(true),
    )
    .get_allow_app_content_ad_targeting_settings()
}

#[derive(Debug)]
pub struct PrivacyImpl {
    pub state: PlatformState,
}

impl PrivacyImpl {
    pub fn listen_content_policy_changed(
        state: &PlatformState,
        listen: bool,
        ctx: &CallContext,
        event_name: &'static str,
        request: Option<ContentListenRequest>,
        dec: Option<Box<dyn AppEventDecorator + Send + Sync>>,
    ) -> RpcResult<ListenerResponse> {
        let event_context = if let Some(content_request) = request {
            //TODO: Check config for storage type? Are we supporting listeners for cloud settings?
            content_request.app_id.map(|x| {
                json!({
                    "appId": x,
                })
            })
        } else {
            Some(json!({
                "appId": ctx.app_id.to_owned(),
            }))
        };

        AppEvents::add_listener_with_context_and_decorator(
            state,
            event_name.to_owned(),
            ctx.clone(),
            ListenRequest { listen },
            event_context,
            dec,
        );

        Ok(ListenerResponse {
            listening: listen,
            event: event_name.into(),
        })
    }

    async fn on_content_policy_changed(
        &self,
        ctx: CallContext,
        event_name: &'static str,
        request: ContentListenRequest,
    ) -> RpcResult<ListenerResponse> {
        Self::listen_content_policy_changed(
            &self.state,
            request.listen,
            &ctx,
            event_name,
            Some(request),
            None,
        )
    }

    pub async fn get_allow_app_content_ad_targeting(state: &PlatformState) -> bool {
        StorageManager::get_bool(state, StorageProperty::AllowAppContentAdTargeting)
            .await
            .unwrap_or(false)
    }

    pub async fn get_allow_personalization(state: &PlatformState, _app_id: &str) -> bool {
        StorageManager::get_bool(state, StorageProperty::AllowPersonalization)
            .await
            .unwrap_or(false)
    }

    pub async fn get_share_watch_history(
        ctx: &CallContext,
        state: &PlatformState,
        _app_id: &str,
    ) -> bool {
        let cap = RoleInfo {
            capability: FireboltCap::Short("discovery:watched".to_string()),
            role: Some(CapabilityRole::Use),
        };
        if let Ok(watch_granted) = is_granted(state.clone(), ctx.clone(), cap).await {
            debug!("--> watch_granted={}", watch_granted);
            return watch_granted;
        }
        false
    }

    pub async fn get_allow_watch_history(state: &PlatformState, _app_id: &str) -> bool {
        StorageManager::get_bool(state, StorageProperty::AllowWatchHistory)
            .await
            .unwrap_or(false)
    }

    pub fn to_storage_property(method: &str) -> Option<StorageProperty> {
        let mut parts: Vec<&str> = method.split('.').collect();
        if parts.len() < 2 {
            return None;
        }
        let method_name = parts.remove(1);
        match method_name {
            "setAllowACRCollection" | "allowACRCollection" => {
                Some(StorageProperty::AllowAcrCollection)
            }
            "setAllowAppContentAdTargeting" | "allowAppContentAdTargeting" => {
                Some(StorageProperty::AllowAppContentAdTargeting)
            }
            // Do not include entry for AllowBusinessAnalytics here.
            // No set/get APIs for AllowBusinessAnalytics
            "setAllowCameraAnalytics" | "allowCameraAnalytics" => {
                Some(StorageProperty::AllowCameraAnalytics)
            }
            "setAllowPersonalization" | "allowPersonalization" => {
                Some(StorageProperty::AllowPersonalization)
            }
            "setAllowPrimaryBrowseAdTargeting" | "allowPrimaryBrowseAdTargeting" => {
                Some(StorageProperty::AllowPrimaryBrowseAdTargeting)
            }
            "setAllowPrimaryContentAdTargeting" | "allowPrimaryContentAdTargeting" => {
                Some(StorageProperty::AllowPrimaryContentAdTargeting)
            }
            "setAllowProductAnalytics" | "allowProductAnalytics" => {
                Some(StorageProperty::AllowProductAnalytics)
            }
            "setAllowRemoteDiagnostics" | "allowRemoteDiagnostics" => {
                Some(StorageProperty::AllowRemoteDiagnostics)
            }
            "setAllowResumePoints" | "allowResumePoints" => {
                Some(StorageProperty::AllowResumePoints)
            }
            "setAllowUnentitledPersonalization" | "allowUnentitledPersonalization" => {
                Some(StorageProperty::AllowUnentitledPersonalization)
            }
            "setAllowUnentitledResumePoints" | "allowUnentitledResumePoints" => {
                Some(StorageProperty::AllowUnentitledResumePoints)
            }
            "setAllowWatchHistory" | "allowWatchHistory" => {
                Some(StorageProperty::AllowWatchHistory)
            }
            _ => None,
        }
    }

    /// Handles get request for privacy settings
    /// # Arguments
    ///
    /// * `method` - A string slice that holds the method name (eg., privacy.allowWatchHistory)
    /// * `platform_state` - A reference to PlatformState
    /// * `fill_default` - get configured default value if not found in the storage
    ///
    pub async fn handle_allow_get_requests(
        method: &str,
        platform_state: &PlatformState,
    ) -> RpcResult<bool> {
        let property_opt = Self::to_storage_property(method);
        if let Some(prop) = property_opt {
            Self::get_bool(platform_state, prop).await
        } else {
            Err(jsonrpsee::core::Error::Call(CallError::Custom {
                code: CAPABILITY_NOT_AVAILABLE,
                message: format!("{} is not available", method),
                data: None,
            }))
        }
    }

    pub async fn handle_allow_set_requests(
        method: &str,
        platform_state: &PlatformState,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        let property_opt = Self::to_storage_property(method);
        if let Some(prop) = property_opt {
            debug!("Resolved property: {:?}", prop);
            Self::set_bool(platform_state, prop, set_request.value).await
        } else {
            Err(jsonrpsee::core::Error::Call(CallError::Custom {
                code: CAPABILITY_NOT_AVAILABLE,
                message: format!("{} is not available", method),
                data: None,
            }))
        }
    }

    pub async fn get_bool_storage_property(&self, property: StorageProperty) -> RpcResult<bool> {
        Self::get_bool(&self.state, property).await
    }

    pub async fn get_bool(
        platform_state: &PlatformState,
        property: StorageProperty,
    ) -> RpcResult<bool> {
        use ripple_sdk::api::manifest::device_manifest::PrivacySettingsStorageType;
        let privacy_settings_storage_type: PrivacySettingsStorageType = platform_state
            .get_device_manifest()
            .configuration
            .features
            .privacy_settings_storage_type;

        match privacy_settings_storage_type {
            PrivacySettingsStorageType::Local | PrivacySettingsStorageType::Sync => {
                let payload = PrivacySettingsStoreRequest::GetPrivacySettings(property);
                let response = platform_state.get_client().send_extn_request(payload).await;
                if let Ok(extn_msg) = response {
                    match extn_msg.payload {
                        ExtnPayload::Response(res) => match res {
                            ExtnResponse::Boolean(val) => RpcResult::Ok(val),
                            _ => RpcResult::Err(jsonrpsee::core::Error::Custom(
                                "Unable to fetch".to_owned(),
                            )),
                        },
                        _ => RpcResult::Err(jsonrpsee::core::Error::Custom(
                            "Unexpected response received from Extn".to_owned(),
                        )),
                    }
                } else {
                    RpcResult::Err(jsonrpsee::core::Error::Custom(
                        "Error in getting response from Extn".to_owned(),
                    ))
                }
            }
            PrivacySettingsStorageType::Cloud => {
                if let Some(dist_session) = platform_state.session_state.get_account_session() {
                    let request = PrivacyCloudRequest::GetProperty(GetPropertyParams {
                        setting: property.as_privacy_setting().unwrap(),
                        dist_session,
                    });
                    if let Ok(resp) = platform_state.get_client().send_extn_request(request).await {
                        if let Some(ExtnResponse::Boolean(b)) = resp.payload.extract() {
                            return Ok(b);
                        }
                    }
                    Err(jsonrpsee::core::Error::Custom(String::from(
                        "PrivacySettingsStorageType::Cloud: Not Available",
                    )))
                } else {
                    Err(jsonrpsee::core::Error::Custom(String::from(
                        "Account session is not available",
                    )))
                }
            }
        }
    }

    pub async fn set_bool(
        platform_state: &PlatformState,
        property: StorageProperty,
        value: bool,
    ) -> RpcResult<()> {
        use ripple_sdk::api::manifest::device_manifest::PrivacySettingsStorageType;
        let privacy_settings_storage_type: PrivacySettingsStorageType = platform_state
            .get_device_manifest()
            .configuration
            .features
            .privacy_settings_storage_type;

        match privacy_settings_storage_type {
            PrivacySettingsStorageType::Local => {
                let payload = PrivacySettingsStoreRequest::SetPrivacySettings(property, value);
                let response = platform_state.get_client().send_extn_request(payload).await;
                if let Ok(extn_msg) = response {
                    match extn_msg.payload {
                        ExtnPayload::Response(res) => match res {
                            ExtnResponse::None(_) => RpcResult::Ok(()),
                            _ => RpcResult::Err(jsonrpsee::core::Error::Custom(
                                "Unable to fetch".to_owned(),
                            )),
                        },
                        _ => RpcResult::Err(jsonrpsee::core::Error::Custom(
                            "Unexpected response received from Extn".to_owned(),
                        )),
                    }
                } else {
                    RpcResult::Err(jsonrpsee::core::Error::Custom(
                        "Error in getting response from Extn".to_owned(),
                    ))
                }
            }
            PrivacySettingsStorageType::Cloud | PrivacySettingsStorageType::Sync => {
                if let Some(dist_session) = platform_state.session_state.get_account_session() {
                    if let Some(privacy_setting) = property.as_privacy_setting() {
                        let request = PrivacyCloudRequest::SetProperty(SetPropertyParams {
                            setting: privacy_setting,
                            value,
                            dist_session,
                        });
                        let result = platform_state.get_client().send_extn_request(request).await;
                        if PrivacySettingsStorageType::Sync == privacy_settings_storage_type
                            && result.is_ok()
                        {
                            let _ = StorageManager::set_bool(platform_state, property, value, None)
                                .await;
                        }
                        if result.is_ok() {
                            return Ok(());
                        }
                    }
                }
                Err(jsonrpsee::core::Error::Custom(String::from(&format!(
                    "{:?}: Not Available",
                    privacy_settings_storage_type
                ))))
            }
        }
    }

    pub async fn get_settings_local(&self) -> RpcResult<PrivacySettings> {
        let settings = PrivacySettings {
            allow_acr_collection: self
                .get_bool_storage_property(StorageProperty::AllowAcrCollection)
                .await
                .unwrap_or(false),
            allow_resume_points: self
                .get_bool_storage_property(StorageProperty::AllowResumePoints)
                .await
                .unwrap_or(false),
            allow_app_content_ad_targeting: self
                .get_bool_storage_property(StorageProperty::AllowAppContentAdTargeting)
                .await
                .unwrap_or(false),
            allow_business_analytics: self
                .get_bool_storage_property(StorageProperty::AllowBusinessAnalytics)
                .await
                .unwrap_or(true),
            allow_camera_analytics: self
                .get_bool_storage_property(StorageProperty::AllowCameraAnalytics)
                .await
                .unwrap_or(false),
            allow_personalization: self
                .get_bool_storage_property(StorageProperty::AllowPersonalization)
                .await
                .unwrap_or(false),
            allow_primary_browse_ad_targeting: self
                .get_bool_storage_property(StorageProperty::AllowPrimaryBrowseAdTargeting)
                .await
                .unwrap_or(false),
            allow_primary_content_ad_targeting: self
                .get_bool_storage_property(StorageProperty::AllowPrimaryContentAdTargeting)
                .await
                .unwrap_or(false),
            allow_product_analytics: self
                .get_bool_storage_property(StorageProperty::AllowProductAnalytics)
                .await
                .unwrap_or(false),
            allow_remote_diagnostics: self
                .get_bool_storage_property(StorageProperty::AllowRemoteDiagnostics)
                .await
                .unwrap_or(false),
            allow_unentitled_personalization: self
                .get_bool_storage_property(StorageProperty::AllowUnentitledPersonalization)
                .await
                .unwrap_or(false),
            allow_unentitled_resume_points: self
                .get_bool_storage_property(StorageProperty::AllowUnentitledResumePoints)
                .await
                .unwrap_or(false),
            allow_watch_history: self
                .get_bool_storage_property(StorageProperty::AllowWatchHistory)
                .await
                .unwrap_or(false),
        };
        Ok(settings)
    }
}

#[async_trait]
impl PrivacyServer for PrivacyImpl {
    async fn privacy_allow_acr_collection(&self, ctx: CallContext) -> RpcResult<bool> {
        Self::handle_allow_get_requests(&ctx.method, &self.state).await
    }

    async fn privacy_allow_acr_collection_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        Self::handle_allow_set_requests(&ctx.method, &self.state, set_request).await
    }

    async fn privacy_allow_acr_collection_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_content_policy_changed(
            ctx,
            EVENT_ALLOW_ACR_COLLECTION_CHANGED,
            ContentListenRequest {
                app_id: None,
                listen: request.listen,
            },
        )
        .await
    }

    async fn privacy_allow_app_content_ad_targeting(&self, ctx: CallContext) -> RpcResult<bool> {
        Self::handle_allow_get_requests(&ctx.method, &self.state).await
    }

    async fn privacy_allow_app_content_ad_targeting_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        Self::handle_allow_set_requests(&ctx.method, &self.state, set_request).await
    }

    async fn privacy_allow_app_content_ad_targeting_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_content_policy_changed(
            ctx,
            EVENT_ALLOW_APP_CONTENT_AD_TARGETING_CHANGED,
            ContentListenRequest {
                app_id: None,
                listen: request.listen,
            },
        )
        .await
    }

    async fn privacy_allow_camera_analytics(&self, ctx: CallContext) -> RpcResult<bool> {
        Self::handle_allow_get_requests(&ctx.method, &self.state).await
    }

    async fn privacy_allow_camera_analytics_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        Self::handle_allow_set_requests(&ctx.method, &self.state, set_request).await
    }

    async fn privacy_allow_camera_analytics_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_content_policy_changed(
            ctx,
            EVENT_ALLOW_CAMERA_ANALYTICS_CHANGED,
            ContentListenRequest {
                app_id: None,
                listen: request.listen,
            },
        )
        .await
    }

    async fn privacy_allow_personalization(&self, ctx: CallContext) -> RpcResult<bool> {
        Self::handle_allow_get_requests(&ctx.method, &self.state).await
    }

    async fn privacy_allow_personalization_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        Self::handle_allow_set_requests(&ctx.method, &self.state, set_request).await
    }

    async fn privacy_allow_personalization_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_content_policy_changed(
            ctx,
            EVENT_ALLOW_PERSONALIZATION_CHANGED,
            ContentListenRequest {
                app_id: None,
                listen: request.listen,
            },
        )
        .await
    }

    async fn privacy_allow_primary_browse_ad_targeting(&self, ctx: CallContext) -> RpcResult<bool> {
        Self::handle_allow_get_requests(&ctx.method, &self.state).await
    }

    async fn privacy_allow_primary_browse_ad_targeting_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        Self::handle_allow_set_requests(&ctx.method, &self.state, set_request).await
    }

    async fn privacy_allow_primary_browse_ad_targeting_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_content_policy_changed(
            ctx,
            EVENT_ALLOW_PRIMARY_BROWSE_AD_TARGETING_CHANGED,
            ContentListenRequest {
                app_id: None,
                listen: request.listen,
            },
        )
        .await
    }

    async fn privacy_allow_primary_content_ad_targeting(
        &self,
        ctx: CallContext,
    ) -> RpcResult<bool> {
        Self::handle_allow_get_requests(&ctx.method, &self.state).await
    }

    async fn privacy_allow_primary_content_ad_targeting_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        Self::handle_allow_set_requests(&ctx.method, &self.state, set_request).await
    }

    async fn privacy_allow_primary_content_ad_targeting_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_content_policy_changed(
            ctx,
            EVENT_ALLOW_PRIMARY_CONTENT_AD_TARGETING_CHANGED,
            ContentListenRequest {
                app_id: None,
                listen: request.listen,
            },
        )
        .await
    }

    async fn privacy_allow_product_analytics(&self, ctx: CallContext) -> RpcResult<bool> {
        Self::handle_allow_get_requests(&ctx.method, &self.state).await
    }

    async fn privacy_allow_product_analytics_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        Self::handle_allow_set_requests(&ctx.method, &self.state, set_request).await
    }

    async fn privacy_allow_product_analytics_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_content_policy_changed(
            ctx,
            EVENT_ALLOW_PRODUCT_ANALYTICS_CHANGED,
            ContentListenRequest {
                app_id: None,
                listen: request.listen,
            },
        )
        .await
    }

    async fn privacy_allow_remote_diagnostics(&self, ctx: CallContext) -> RpcResult<bool> {
        Self::handle_allow_get_requests(&ctx.method, &self.state).await
    }

    async fn privacy_allow_remote_diagnostics_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        Self::handle_allow_set_requests(&ctx.method, &self.state, set_request).await
    }

    async fn privacy_allow_remote_diagnostics_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_content_policy_changed(
            ctx,
            EVENT_ALLOW_REMOTE_DIAGNOSTICS_CHANGED,
            ContentListenRequest {
                app_id: None,
                listen: request.listen,
            },
        )
        .await
    }

    async fn privacy_allow_resume_points(&self, ctx: CallContext) -> RpcResult<bool> {
        Self::handle_allow_get_requests(&ctx.method, &self.state).await
    }

    async fn privacy_allow_resume_points_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        Self::handle_allow_set_requests(&ctx.method, &self.state, set_request).await
    }

    async fn privacy_allow_resume_points_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_content_policy_changed(
            ctx,
            EVENT_ALLOW_RESUME_POINTS_CHANGED,
            ContentListenRequest {
                app_id: None,
                listen: request.listen,
            },
        )
        .await
    }

    async fn privacy_allow_unentitled_personalization(&self, ctx: CallContext) -> RpcResult<bool> {
        Self::handle_allow_get_requests(&ctx.method, &self.state).await
    }

    async fn privacy_allow_unentitled_personalization_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        Self::handle_allow_set_requests(&ctx.method, &self.state, set_request).await
    }

    async fn privacy_allow_unentitled_personalization_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_content_policy_changed(
            ctx,
            EVENT_ALLOW_UNENTITLED_PERSONALIZATION_CHANGED,
            ContentListenRequest {
                app_id: None,
                listen: request.listen,
            },
        )
        .await
    }

    async fn privacy_allow_unentitled_resume_points(&self, ctx: CallContext) -> RpcResult<bool> {
        Self::handle_allow_get_requests(&ctx.method, &self.state).await
    }

    async fn privacy_allow_unentitled_resume_points_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        Self::handle_allow_set_requests(&ctx.method, &self.state, set_request).await
    }

    async fn privacy_allow_unentitled_resume_points_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_content_policy_changed(
            ctx,
            EVENT_ALLOW_UNENTITLED_RESUME_POINTS_CHANGED,
            ContentListenRequest {
                app_id: None,
                listen: request.listen,
            },
        )
        .await
    }

    async fn privacy_allow_watch_history(&self, ctx: CallContext) -> RpcResult<bool> {
        Self::handle_allow_get_requests(&ctx.method, &self.state).await
    }

    async fn privacy_allow_watch_history_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        Self::handle_allow_set_requests(&ctx.method, &self.state, set_request).await
    }

    async fn privacy_allow_watch_history_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_content_policy_changed(
            ctx,
            EVENT_ALLOW_WATCH_HISTORY_CHANGED,
            ContentListenRequest {
                app_id: None,
                listen: request.listen,
            },
        )
        .await
    }

    async fn get_settings(&self, _ctx: CallContext) -> RpcResult<PrivacySettings> {
        use ripple_sdk::api::manifest::device_manifest::PrivacySettingsStorageType;
        let privacy_settings_storage_type: PrivacySettingsStorageType = self
            .state
            .get_device_manifest()
            .configuration
            .features
            .privacy_settings_storage_type;

        match privacy_settings_storage_type {
            PrivacySettingsStorageType::Local | PrivacySettingsStorageType::Sync => {
                self.get_settings_local().await
            }
            PrivacySettingsStorageType::Cloud => {
                if let Some(dist_session) = self.state.session_state.get_account_session() {
                    let request = PrivacyCloudRequest::GetProperties(dist_session);
                    if let Ok(resp) = self.state.get_client().send_extn_request(request).await {
                        if let Some(b) = resp.payload.extract() {
                            return Ok(b);
                        }
                    }
                    Err(jsonrpsee::core::Error::Custom(String::from(
                        "PrivacySettingsStorageType::Cloud: Not Available",
                    )))
                } else {
                    Err(jsonrpsee::core::Error::Custom(String::from(
                        "Account session is not available",
                    )))
                }
            }
        }
    }
}

pub struct PrivacyProvider;
impl RippleRPCProvider<PrivacyImpl> for PrivacyProvider {
    fn provide(state: PlatformState) -> RpcModule<PrivacyImpl> {
        (PrivacyImpl { state }).into_rpc()
    }
}
