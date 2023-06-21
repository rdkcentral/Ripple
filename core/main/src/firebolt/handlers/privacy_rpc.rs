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

use crate::processor::storage::storage_manager::StorageManager;
use crate::{
    firebolt::rpc::RippleRPCProvider, processor::storage::storage_manager::StorageManagerError,
    service::apps::app_events::AppEvents, state::platform_state::PlatformState,
};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    types::error::CallError,
    RpcModule,
};
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

use super::capabilities_rpc::is_granted;

pub const US_PRIVACY_KEY: &'static str = "us_privacy";
pub const LMT_KEY: &'static str = "lmt";

#[derive(Debug, Clone)]
struct AllowAppContentAdTargetingSettings {
    lmt: String,
    us_privacy: String,
}

impl AllowAppContentAdTargetingSettings {
    pub fn new(limit_ad_targeting: bool) -> Self {
        let (lmt, us_privacy) = match limit_ad_targeting {
            true => ("1", "1-Y-"),
            false => ("0", "1-N-"),
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
) -> HashMap<String, String> {
    let data = StorageProperty::AllowAppContentAdTargeting.as_data();

    match StorageManager::get_bool_from_namespace(
        platform_state,
        data.namespace.to_string(),
        data.key,
    )
    .await
    {
        Ok(resp) => AllowAppContentAdTargetingSettings::new(resp.as_value())
            .get_allow_app_content_ad_targeting_settings(),
        Err(StorageManagerError::NotFound) => AllowAppContentAdTargetingSettings::default()
            .get_allow_app_content_ad_targeting_settings(),
        _ => AllowAppContentAdTargetingSettings::new(true)
            .get_allow_app_content_ad_targeting_settings(),
    }
}

#[derive(Debug)]
pub struct PrivacyImpl {
    pub state: PlatformState,
}

impl PrivacyImpl {
    async fn on_content_policy_changed(
        &self,
        ctx: CallContext,
        event_name: &'static str,
        request: ContentListenRequest,
    ) -> RpcResult<ListenerResponse> {
        //TODO: Check config for storage type? Are we supporting listeners for cloud settings?
        let event_context = request.app_id.map(|x| {
            json!({
                "appId": x,
            })
        });

        AppEvents::add_listener_with_context(
            &&self.state,
            event_name.to_owned(),
            ctx,
            ListenRequest {
                listen: request.listen,
            },
            event_context,
        );

        Ok(ListenerResponse {
            listening: request.listen,
            event: event_name.into(),
        })
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
            capability: "xrn:firebolt:capability:discovery:watched".to_string(),
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
        let mut parts: Vec<&str> = method.clone().split(".").collect();
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
        if property_opt.is_none() {
            return Err(jsonrpsee::core::Error::Call(CallError::Custom {
                code: CAPABILITY_NOT_AVAILABLE,
                message: format!("{} is not available", method),
                data: None,
            }));
        } else {
            let property = property_opt.unwrap();
            Self::get_bool(platform_state, property).await
        }
    }

    pub async fn handle_allow_set_requests(
        method: &str,
        platform_state: &PlatformState,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        let property_opt = Self::to_storage_property(method);
        if property_opt.is_none() {
            return Err(jsonrpsee::core::Error::Call(CallError::Custom {
                code: CAPABILITY_NOT_AVAILABLE,
                message: format!("{} is not available", method),
                data: None,
            }));
        } else {
            let property = property_opt.unwrap();
            debug!("Resolved property: {:?}", property);
            Self::set_bool(platform_state, property, set_request.value).await
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
            PrivacySettingsStorageType::Local => {
                StorageManager::get_bool(platform_state, property).await
            }
            PrivacySettingsStorageType::Cloud => {
                let dist_session = platform_state.session_state.get_account_session().unwrap();
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
            }
            PrivacySettingsStorageType::Sync => Err(jsonrpsee::core::Error::Custom(String::from(
                "PrivacySettingsStorageType::Sync: Unimplemented",
            ))),
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
                StorageManager::set_bool(platform_state, property, value, None).await
            }
            PrivacySettingsStorageType::Cloud => {
                let dist_session = platform_state.session_state.get_account_session().unwrap();
                let request = PrivacyCloudRequest::SetProperty(SetPropertyParams {
                    setting: property.as_privacy_setting().unwrap(),
                    value,
                    dist_session,
                });
                if let Ok(_) = platform_state.get_client().send_extn_request(request).await {
                    return Ok(());
                }
                Err(jsonrpsee::core::Error::Custom(String::from(
                    "PrivacySettingsStorageType::Cloud: Not Available",
                )))
            }
            PrivacySettingsStorageType::Sync => Err(jsonrpsee::core::Error::Custom(String::from(
                "PrivacySettingsStorageType::Sync: Unimplemented",
            ))),
        }
    }

    async fn get_settings_local(&self) -> RpcResult<PrivacySettings> {
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
            PrivacySettingsStorageType::Local => self.get_settings_local().await,
            PrivacySettingsStorageType::Cloud => {
                let dist_session = self.state.session_state.get_account_session().unwrap();
                let request = PrivacyCloudRequest::GetProperties(dist_session);
                if let Ok(resp) = self.state.get_client().send_extn_request(request).await {
                    if let Some(b) = resp.payload.extract() {
                        return Ok(b);
                    }
                }
                Err(jsonrpsee::core::Error::Custom(String::from(
                    "PrivacySettingsStorageType::Cloud: Not Available",
                )))
            }
            PrivacySettingsStorageType::Sync => Err(jsonrpsee::core::Error::Custom(String::from(
                "PrivacySettingsStorageType::Sync: Unimplemented",
            ))),
        }
    }
}

pub struct PrivacyProvider;
impl RippleRPCProvider<PrivacyImpl> for PrivacyProvider {
    fn provide(state: PlatformState) -> RpcModule<PrivacyImpl> {
        (PrivacyImpl { state }).into_rpc()
    }
}
