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
    processor::storage::storage_manager::{StorageManagerError, StorageManagerResponse},
    state::platform_state::PlatformState,
};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    types::error::CallError,
    RpcModule,
};
use ripple_sdk::{
    api::{
        firebolt::{
            fb_capabilities::CAPABILITY_NOT_AVAILABLE,
            fb_general::{ListenRequest, ListenerResponse},
        },
        gateway::rpc_gateway_api::CallContext,
    },
    log::debug,
    serde_json::json,
};

use crate::processor::storage::{
    storage_manager::StorageManager,
    storage_property::{
        StorageProperty, EVENT_ALLOW_ACR_COLLECTION_CHANGED,
        EVENT_ALLOW_APP_CONTENT_AD_TARGETING_CHANGED, EVENT_ALLOW_CAMERA_ANALYTICS_CHANGED,
        EVENT_ALLOW_PERSONALIZATION_CHANGED, EVENT_ALLOW_PRIMARY_BROWSE_AD_TARGETING_CHANGED,
        EVENT_ALLOW_PRIMARY_CONTENT_AD_TARGETING_CHANGED, EVENT_ALLOW_PRODUCT_ANALYTICS_CHANGED,
        EVENT_ALLOW_REMOTE_DIAGNOSTICS_CHANGED, EVENT_ALLOW_RESUME_POINTS_CHANGED,
        EVENT_ALLOW_UNENTITLED_PERSONALIZATION_CHANGED,
        EVENT_ALLOW_UNENTITLED_RESUME_POINTS_CHANGED, EVENT_ALLOW_WATCH_HISTORY_CHANGED,
        EVENT_ENABLE_RECOMMENDATIONS, EVENT_LIMIT_AD_TRACKING, EVENT_REMEMBER_WATCHED_PROGRAMS,
        EVENT_SHARE_WATCH_HISTORY, KEY_ENABLE_RECOMMENDATIONS, KEY_REMEMBER_WATCHED_PROGRAMS,
        KEY_SHARE_WATCH_HISTORY,
    },
};

use std::collections::HashMap;

// use crate::{
//     api::rpc::rpc_gateway::{CallContext, RPCProvider},
//     apps::app_events::{AppEvents, ListenRequest, ListenerResponse},
//     device_manifest::PrivacySettingsStorageType,
//     helpers::{
//         error_util::CAPABILITY_NOT_AVAILABLE,
//         ripple_helper::{IRippleHelper, RippleHelper, RippleHelperFactory, RippleHelperType},
//     },
//     managers::{
//         capability_manager::{
//             CapClassifiedRequest, FireboltCap, IGetLoadedCaps, RippleHandlerCaps,
//         },
//         storage::{
//             storage_manager::{StorageManager, StorageManagerError, StorageManagerResponse},
//             storage_property::{
//                 StorageProperty, EVENT_ALLOW_ACR_COLLECTION_CHANGED,
//                 EVENT_ALLOW_APP_CONTENT_AD_TARGETING_CHANGED, EVENT_ALLOW_CAMERA_ANALYTICS_CHANGED,
//                 EVENT_ALLOW_PERSONALIZATION_CHANGED,
//                 EVENT_ALLOW_PRIMARY_BROWSE_AD_TARGETING_CHANGED,
//                 EVENT_ALLOW_PRIMARY_CONTENT_AD_TARGETING_CHANGED,
//                 EVENT_ALLOW_PRODUCT_ANALYTICS_CHANGED, EVENT_ALLOW_REMOTE_DIAGNOSTICS_CHANGED,
//                 EVENT_ALLOW_RESUME_POINTS_CHANGED, EVENT_ALLOW_UNENTITLED_PERSONALIZATION_CHANGED,
//                 EVENT_ALLOW_UNENTITLED_RESUME_POINTS_CHANGED, EVENT_ALLOW_WATCH_HISTORY_CHANGED,
//                 EVENT_ENABLE_RECOMMENDATIONS, EVENT_LIMIT_AD_TRACKING,
//                 EVENT_REMEMBER_WATCHED_PROGRAMS, EVENT_SHARE_WATCH_HISTORY,
//                 KEY_ENABLE_RECOMMENDATIONS, KEY_REMEMBER_WATCHED_PROGRAMS, KEY_SHARE_WATCH_HISTORY,
//             },
//         },
//     },
//     platform_state::PlatformState,
// };

use serde::{Deserialize, Serialize};
// use tracing::{debug, instrument};

#[derive(Debug, Deserialize, Clone)]
pub struct SetBoolProperty {
    pub value: bool,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GetAppContentPolicy {
    pub app_id: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetAppContentPolicy {
    pub app_id: String,
    pub value: bool,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContentListenRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    pub listen: bool,
}

pub const US_PRIVACY_KEY: &'static str = "us_privacy";
pub const LMT_KEY: &'static str = "lmt";

#[derive(Debug, Clone)]
struct LimitAdTrackingSettings {
    lmt: String,
    us_privacy: String,
}

impl LimitAdTrackingSettings {
    pub fn new(limit_ad_tracking: bool) -> Self {
        let (lmt, us_privacy) = match limit_ad_tracking {
            true => ("1", "1-Y-"),
            false => ("0", "1-N-"),
        };
        LimitAdTrackingSettings {
            lmt: lmt.to_owned(),
            us_privacy: us_privacy.to_owned(),
        }
    }

    pub fn get_limit_ad_tracking_settings(&self) -> HashMap<String, String> {
        HashMap::from([
            (US_PRIVACY_KEY.to_owned(), self.us_privacy.to_owned()),
            (LMT_KEY.to_owned(), self.lmt.to_owned()),
        ])
    }
}
impl Default for LimitAdTrackingSettings {
    fn default() -> Self {
        Self {
            /*
            As per X1 privacy settings documentation, default privacy setting is lmt = 0 us_privacy = 1-N-
            (i.e. , Customer has not opted-out)
            https://developer.comcast.com/documentation/limit-ad-tracking-and-ccpa-technical-requirements-1
            */
            lmt: "0".to_owned(),
            us_privacy: "1-N-".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacySettings {
    #[serde(rename = "allowACRCollection")]
    pub allow_acr_collection: bool,
    pub allow_resume_points: bool,
    pub allow_app_content_ad_targeting: bool,
    pub allow_camera_analytics: bool,
    pub allow_personalization: bool,
    pub allow_primary_browse_ad_targeting: bool,
    pub allow_primary_content_ad_targeting: bool,
    pub allow_product_analytics: bool,
    pub allow_remote_diagnostics: bool,
    pub allow_unentitled_personalization: bool,
    pub allow_unentitled_resume_points: bool,
    pub allow_watch_history: bool,
}

impl PrivacySettings {
    pub fn new() -> Self {
        PrivacySettings {
            allow_acr_collection: false,
            allow_resume_points: false,
            allow_app_content_ad_targeting: false,
            allow_camera_analytics: false,
            allow_personalization: false,
            allow_primary_browse_ad_targeting: false,
            allow_primary_content_ad_targeting: false,
            allow_product_analytics: false,
            allow_remote_diagnostics: false,
            allow_unentitled_personalization: false,
            allow_unentitled_resume_points: false,
            allow_watch_history: false,
        }
    }
}

#[rpc(server)]
pub trait Privacy {
    #[method(name = "privacy.enableRecommendations")]
    async fn privacy_enable_recommendations(
        &self,
        ctx: CallContext,
        get_request: GetAppContentPolicy,
    ) -> RpcResult<bool>;
    #[method(name = "privacy.setEnableRecommendations")]
    async fn privacy_enable_recommendations_set(
        &self,
        ctx: CallContext,
        set_request: SetAppContentPolicy,
    ) -> RpcResult<()>;
    #[method(name = "privacy.onEnableRecommendationsChanged")]
    async fn privacy_enable_recommendations_changed(
        &self,
        ctx: CallContext,
        request: ContentListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "privacy.limitAdTracking", aliases = ["badger.limitAdTracking"])]
    async fn privacy_limit_ad_tracking(&self, ctx: CallContext) -> RpcResult<bool>;
    #[method(name = "privacy.setLimitAdTracking")]
    async fn privacy_limit_ad_tracking_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()>;
    #[method(name = "privacy.onLimitAdTrackingChanged")]
    async fn privacy_limit_ad_tracking_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "privacy.rememberWatchedPrograms")]
    async fn privacy_remember_watched_programs(
        &self,
        ctx: CallContext,
        get_request: GetAppContentPolicy,
    ) -> RpcResult<bool>;
    #[method(name = "privacy.setRememberWatchedPrograms")]
    async fn privacy_remember_watched_programs_set(
        &self,
        ctx: CallContext,
        set_request: SetAppContentPolicy,
    ) -> RpcResult<()>;
    #[method(name = "privacy.onRememberWatchedProgramsChanged")]
    async fn privacy_remember_watched_programs_changed(
        &self,
        ctx: CallContext,
        request: ContentListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "privacy.shareWatchHistory")]
    async fn privacy_share_watch_history(
        &self,
        ctx: CallContext,
        get_request: GetAppContentPolicy,
    ) -> RpcResult<bool>;
    #[method(name = "privacy.setShareWatchHistory")]
    async fn privacy_share_watch_history_set(
        &self,
        ctx: CallContext,
        set_request: SetAppContentPolicy,
    ) -> RpcResult<()>;
    #[method(name = "privacy.onShareWatchHistoryChanged")]
    async fn privacy_share_watch_history_changed(
        &self,
        ctx: CallContext,
        request: ContentListenRequest,
    ) -> RpcResult<ListenerResponse>;
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

pub async fn get_limit_ad_tracking_settings(
    platform_state: &PlatformState,
) -> HashMap<String, String> {
    let data = StorageProperty::LimitAdTracking.as_data();

    match StorageManager::get_bool_from_namespace(
        platform_state,
        data.namespace.to_string(),
        data.key,
    )
    .await
    {
        Ok(resp) => LimitAdTrackingSettings::new(resp.as_value()).get_limit_ad_tracking_settings(),
        Err(StorageManagerError::NotFound) => {
            LimitAdTrackingSettings::default().get_limit_ad_tracking_settings()
        }
        _ => LimitAdTrackingSettings::new(true).get_limit_ad_tracking_settings(),
    }
}

#[derive(Debug)]
pub struct PrivacyImpl {
    pub state: PlatformState,
}

impl PrivacyImpl {
    async fn send_policy_changed_event(&self, _ctx: &CallContext, _app_id: String) {
        // let event_data = DiscoveryImpl::get_content_policy(&ctx, &self.state, &app_id.clone())
        //     .await
        //     .unwrap();
        // AppEvents::emit_with_context(
        //     &self.state,
        //     "discovery.onPolicyChanged",
        //     &serde_json::to_value(event_data).unwrap(),
        //     Some(serde_json::Value::String(app_id)),
        // )
        // .await;
        todo!()
    }

    async fn on_content_policy_changed(
        &self,
        _ctx: CallContext,
        _event_name: &'static str,
        _request: ContentListenRequest,
    ) -> RpcResult<ListenerResponse> {
        // TODO: Check config for storage type? Are we supporting listeners for cloud settings?
        // let event_context = request.app_id.map(|x| {
        //     json!({
        //         "appId": x,
        //     })
        // });

        // AppEvents::add_listener_with_context(
        //     &&self.state.app_events_state,
        //     event_name.to_owned(),
        //     ctx,
        //     ListenRequest {
        //         listen: request.listen,
        //     },
        //     event_context,
        // );

        // Ok(ListenerResponse {
        //     listening: request.listen,
        //     event: event_name,
        // })
        todo!()
    }

    pub async fn get_limit_ad_tracking(state: &PlatformState) -> bool {
        match StorageManager::get_bool(state, StorageProperty::LimitAdTracking).await {
            Ok(resp) => resp,
            Err(_) => true,
        }
    }

    pub async fn get_enable_recommendations(state: &PlatformState, app_id: &str) -> bool {
        match StorageManager::get_bool_from_namespace(
            state,
            get_namespace(app_id),
            KEY_ENABLE_RECOMMENDATIONS,
        )
        .await
        {
            Ok(resp) => resp.as_value(),
            Err(_) => false,
        }
    }

    pub async fn get_share_watch_history(
        _ctx: &CallContext,
        _state: &PlatformState,
        _app_id: &str,
    ) -> bool {
        // let helper = Box::new(state.services.clone());
        // let watch_granted = is_granted(
        //     &helper,
        //     ctx,
        //     "xrn:firebolt:capability:discovery:watched",
        //     None,
        // )
        // .await;
        // debug!("--> watch_granted={}", watch_granted);
        // watch_granted
        todo!()
    }

    pub async fn get_remember_watched_programs(state: &PlatformState, app_id: &str) -> bool {
        match StorageManager::get_bool_from_namespace(
            state,
            get_namespace(app_id),
            KEY_REMEMBER_WATCHED_PROGRAMS,
        )
        .await
        {
            Ok(resp) => resp.as_value(),
            Err(_) => false,
        }
    }

    pub fn to_storage_property(method: &str) -> Option<StorageProperty> {
        match method {
            "privacy.setAllowACRCollection" | "privacy.allowACRCollection" => {
                Some(StorageProperty::AllowAcrCollection)
            }
            "privacy.setAllowAppContentAdTargeting" | "privacy.allowAppContentAdTargeting" => {
                Some(StorageProperty::AllowAppContentAdTargeting)
            }
            "privacy.setAllowCameraAnalytics" | "privacy.allowCameraAnalytics" => {
                Some(StorageProperty::AllowCameraAnalytics)
            }
            "privacy.setAllowPersonalization" | "privacy.allowPersonalization" => {
                Some(StorageProperty::AllowPersonalization)
            }
            "privacy.setAllowPrimaryBrowseAdTargeting"
            | "privacy.allowPrimaryBrowseAdTargeting" => {
                Some(StorageProperty::AllowPrimaryBrowseAdTargeting)
            }
            "privacy.setAllowPrimaryContentAdTargeting"
            | "privacy.allowPrimaryContentAdTargeting" => {
                Some(StorageProperty::AllowPrimaryContentAdTargeting)
            }
            "privacy.setAllowProductAnalytics" | "privacy.allowProductAnalytics" => {
                Some(StorageProperty::AllowProductAnalytics)
            }
            "privacy.setAllowRemoteDiagnostics" | "privacy.allowRemoteDiagnostics" => {
                Some(StorageProperty::AllowRemoteDiagnostics)
            }
            "privacy.setAllowResumePoints" | "privacy.allowResumePoints" => {
                Some(StorageProperty::AllowResumePoints)
            }
            "privacy.setAllowUnentitledPersonalization"
            | "privacy.allowUnentitledPersonalization" => {
                Some(StorageProperty::AllowUnentitledPersonalization)
            }
            "privacy.setAllowUnentitledResumePoints" | "privacy.allowUnentitledResumePoints" => {
                Some(StorageProperty::AllowUnentitledResumePoints)
            }
            "privacy.setAllowWatchHistory" | "privacy.allowWatchHistory" => {
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
        fill_default: bool,
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
            Self::get_bool(platform_state, property, fill_default).await
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
        Self::get_bool(&self.state, property, true).await
    }

    pub async fn get_bool(
        platform_state: &PlatformState,
        property: StorageProperty,
        _fill_default: bool,
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
            PrivacySettingsStorageType::Cloud => Err(jsonrpsee::core::Error::Custom(String::from(
                "PrivacySettingsStorageType::Cloud: Unimplemented",
            ))),
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
            PrivacySettingsStorageType::Cloud => Err(jsonrpsee::core::Error::Custom(String::from(
                "PrivacySettingsStorageType::Cloud: Unimplemented",
            ))),
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
    async fn privacy_enable_recommendations(
        &self,
        _ctx: CallContext,
        get_request: GetAppContentPolicy,
    ) -> RpcResult<bool> {
        match StorageManager::get_bool_from_namespace(
            &self.state,
            get_namespace(&get_request.app_id),
            KEY_ENABLE_RECOMMENDATIONS,
        )
        .await
        {
            Ok(resp) => Ok(resp.as_value()),
            Err(_) => Ok(false),
        }
    }

    async fn privacy_enable_recommendations_set(
        &self,
        ctx: CallContext,
        set_request: SetAppContentPolicy,
    ) -> RpcResult<()> {
        let context = Some(json!({
            "appId": set_request.app_id,
        }));
        match StorageManager::set_in_namespace(
            &self.state,
            get_namespace(&set_request.app_id),
            KEY_ENABLE_RECOMMENDATIONS.to_string(),
            json!(set_request.value),
            Some(&[EVENT_ENABLE_RECOMMENDATIONS]),
            context,
        )
        .await
        {
            Ok(resp) => {
                if let StorageManagerResponse::Ok(_) = resp {
                    self.send_policy_changed_event(&ctx, set_request.app_id)
                        .await;
                }
                Ok(())
            }
            Err(_e) => Err(StorageManager::get_firebolt_error(
                &StorageProperty::EnableRecommendations,
            )),
        }
    }

    async fn privacy_enable_recommendations_changed(
        &self,
        ctx: CallContext,
        request: ContentListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_content_policy_changed(ctx, EVENT_ENABLE_RECOMMENDATIONS, request)
            .await
    }

    async fn privacy_limit_ad_tracking(&self, _ctx: CallContext) -> RpcResult<bool> {
        Ok(PrivacyImpl::get_limit_ad_tracking(&self.state).await)
    }

    async fn privacy_limit_ad_tracking_set(
        &self,
        _ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        StorageManager::set_bool(
            &self.state,
            StorageProperty::LimitAdTracking,
            set_request.value,
            None,
        )
        .await
    }

    async fn privacy_limit_ad_tracking_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_content_policy_changed(
            ctx,
            EVENT_LIMIT_AD_TRACKING,
            ContentListenRequest {
                app_id: None,
                listen: request.listen,
            },
        )
        .await
    }

    async fn privacy_remember_watched_programs(
        &self,
        _ctx: CallContext,
        get_request: GetAppContentPolicy,
    ) -> RpcResult<bool> {
        match StorageManager::get_bool_from_namespace(
            &self.state,
            get_namespace(&get_request.app_id),
            KEY_REMEMBER_WATCHED_PROGRAMS,
        )
        .await
        {
            Ok(resp) => Ok(resp.as_value()),
            Err(_) => Ok(false),
        }
    }

    async fn privacy_remember_watched_programs_set(
        &self,
        ctx: CallContext,
        set_request: SetAppContentPolicy,
    ) -> RpcResult<()> {
        let context = Some(json!({
            "appId": set_request.app_id,
        }));
        match StorageManager::set_in_namespace(
            &self.state,
            get_namespace(&set_request.app_id),
            KEY_REMEMBER_WATCHED_PROGRAMS.to_string(),
            json!(set_request.value),
            Some(&[EVENT_REMEMBER_WATCHED_PROGRAMS]),
            context,
        )
        .await
        {
            Ok(resp) => {
                if let StorageManagerResponse::Ok(_) = resp {
                    self.send_policy_changed_event(&ctx, set_request.app_id)
                        .await;
                }
                Ok(())
            }
            Err(_e) => Err(StorageManager::get_firebolt_error(
                &StorageProperty::RemeberWatchedPrograms,
            )),
        }
    }

    async fn privacy_remember_watched_programs_changed(
        &self,
        ctx: CallContext,
        request: ContentListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_content_policy_changed(ctx, EVENT_REMEMBER_WATCHED_PROGRAMS, request)
            .await
    }

    async fn privacy_share_watch_history(
        &self,
        _ctx: CallContext,
        get_request: GetAppContentPolicy,
    ) -> RpcResult<bool> {
        match StorageManager::get_bool_from_namespace(
            &self.state,
            get_namespace(&get_request.app_id),
            KEY_SHARE_WATCH_HISTORY,
        )
        .await
        {
            Ok(resp) => Ok(resp.as_value()),
            Err(_) => Ok(false),
        }
    }

    async fn privacy_share_watch_history_set(
        &self,
        ctx: CallContext,
        set_request: SetAppContentPolicy,
    ) -> RpcResult<()> {
        let context = Some(json!({
            "appId": set_request.app_id,
        }));
        match StorageManager::set_in_namespace(
            &self.state,
            get_namespace(&set_request.app_id),
            KEY_SHARE_WATCH_HISTORY.to_string(),
            json!(set_request.value),
            Some(&[EVENT_SHARE_WATCH_HISTORY]),
            context,
        )
        .await
        {
            Ok(resp) => {
                if let StorageManagerResponse::Ok(_) = resp {
                    self.send_policy_changed_event(&ctx, set_request.app_id)
                        .await;
                }
                Ok(())
            }
            Err(_e) => Err(StorageManager::get_firebolt_error(
                &StorageProperty::ShareWatchHistory,
            )),
        }
    }

    async fn privacy_share_watch_history_changed(
        &self,
        ctx: CallContext,
        request: ContentListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_content_policy_changed(ctx, EVENT_SHARE_WATCH_HISTORY, request)
            .await
    }

    async fn privacy_allow_acr_collection(&self, ctx: CallContext) -> RpcResult<bool> {
        Self::handle_allow_get_requests(&ctx.method, &self.state, true).await
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
        Self::handle_allow_get_requests(&ctx.method, &self.state, true).await
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
        Self::handle_allow_get_requests(&ctx.method, &self.state, true).await
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
        Self::handle_allow_get_requests(&ctx.method, &self.state, true).await
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
        Self::handle_allow_get_requests(&ctx.method, &self.state, true).await
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
        Self::handle_allow_get_requests(&ctx.method, &self.state, true).await
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
        Self::handle_allow_get_requests(&ctx.method, &self.state, true).await
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
        Self::handle_allow_get_requests(&ctx.method, &self.state, true).await
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
        Self::handle_allow_get_requests(&ctx.method, &self.state, true).await
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
        Self::handle_allow_get_requests(&ctx.method, &self.state, true).await
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
        Self::handle_allow_get_requests(&ctx.method, &self.state, true).await
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
        Self::handle_allow_get_requests(&ctx.method, &self.state, true).await
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
            PrivacySettingsStorageType::Cloud => Err(jsonrpsee::core::Error::Custom(String::from(
                "PrivacySettingsStorageType::Cloud: Unimplemented",
            ))),
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

fn get_namespace(app_id: &str) -> String {
    if app_id.is_empty() {
        String::from("Privacy")
    } else {
        format!("Privacy.{}", app_id)
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::{
//         api::{
//             handlers::{
//                 advertising::{AdvertisingImpl, AdvertisingServer},
//                 discovery::{DiscoveryImpl, DiscoveryServer},
//                 entertainment_data::ContentPolicy,
//                 privacy::ContentListenRequest,
//             },
//             rpc::{
//                 api_messages::{ApiMessage, ApiProtocol},
//                 firebolt_gateway::tests::TestGateway,
//                 rpc_gateway::CallContext,
//             },
//         },
//         apps::{
//             app_events::{AppEventsState, ListenRequest},
//             test::helpers::get_ripple_helper_factory,
//         },
//         helpers::ripple_helper::{
//             mock_ripple_helper::{CapResponse, MockRippleBuilder},
//             IRippleHelper, RippleHelper,
//         },
//         managers::capability_manager::{CapClassifiedRequest, CapRequest, DenyReason},
//         platform_state::PlatformState,
//     };
//     use dab::core::{
//         message::{DabError, DabRequest, DabRequestPayload, DabResponsePayload},
//         model::persistent_store::{StorageData, StorageRequest},
//     };
//     use serde_json::{json, Value};
//     use std::collections::{HashMap, HashSet};
//     use std::sync::{Arc, Mutex, MutexGuard};
//     use tokio::{
//         sync::{mpsc, oneshot},
//         time::{timeout, Duration},
//     };
//     use tracing::{debug, info};

//     static mut WATCH_HISTORY: bool = false;

//     #[derive(PartialEq, Eq, Hash, Clone)]
//     enum Events {
//         EnableRecommendationsChangedEvent(Option<String>),
//         LimitAdTrackingChangedEvent,
//         RememberWatchedProgramsChangedEvent(Option<String>),
//         ShareWatchHistoryChangedEvent(Option<String>),
//         PolicyChangedEvent,
//         AdvertisingPolicyChangedEvent,
//         AllowAcrCollectionChangedEvent,
//         AllowAppContentAdTargetingChangedEvent,
//         AllowCameraAnalyticsChangedEvent,
//         AllowPersonalizationChangedEvent,
//         AllowPrimaryBrowseAdTargetingChangedEvent,
//         AllowPrimaryContentAdTargetingChangedEvent,
//         AllowProductAnalyticsChangedEvent,
//         AllowRemoteDiagnosticsChangedEvent,
//         AllowResumePointsChangedEvent,
//         AllowUnentitledPersonalizationChangedEvent,
//         AllowUnentitledResumePointsChangedEvent,
//         AllowWatchHistoryChangedEvent,
//     }

//     struct ContentPolicyApp {
//         helper: Box<RippleHelper>,
//         supported_events: HashSet<Events>,
//     }

//     impl ContentPolicyApp {
//         fn new(helper: Box<RippleHelper>, supported_events: HashSet<Events>) -> Self {
//             ContentPolicyApp {
//                 helper,
//                 supported_events,
//             }
//         }

//         async fn start(
//             &self,
//             mut session_rx: mpsc::Receiver<ApiMessage>,
//             platform_state: PlatformState,
//             tx: Option<mpsc::Sender<String>>,
//         ) {
//             let (provider_rdy_tx, provider_rdy_rx) = oneshot::channel();
//             let discovery_handler: DiscoveryImpl<RippleHelper> = DiscoveryImpl {
//                 helper: self.helper.clone(),
//                 platform_state: platform_state.clone(),
//             };
//             let privacy_handler = PrivacyImpl {
//                 helper: self.helper.clone(),
//                 platform_state: platform_state.clone(),
//             };
//             let advertising_handler = AdvertisingImpl {
//                 helper: self.helper.clone(),
//                 platform_state: platform_state,
//             };

//             let ctx_provider = TestGateway::provider_call();
//             let supported_events = self.supported_events.clone();

//             tokio::spawn(async move {
//                 //Register for events
//                 for events in &supported_events {
//                     match events {
//                         Events::EnableRecommendationsChangedEvent(app_id) => {
//                             let _listen_response = privacy_handler
//                                 .privacy_enable_recommendations_changed(
//                                     ctx_provider.clone(),
//                                     ContentListenRequest {
//                                         app_id: app_id.clone(),
//                                         listen: true,
//                                     },
//                                 )
//                                 .await
//                                 .unwrap();
//                         }
//                         Events::LimitAdTrackingChangedEvent => {
//                             let _listen_response = privacy_handler
//                                 .privacy_limit_ad_tracking_changed(
//                                     ctx_provider.clone(),
//                                     ListenRequest { listen: true },
//                                 )
//                                 .await
//                                 .unwrap();
//                         }
//                         Events::AdvertisingPolicyChangedEvent => {
//                             let _listen_response = advertising_handler
//                                 .advertising_on_policy_changed(
//                                     ctx_provider.clone(),
//                                     ListenRequest { listen: true },
//                                 )
//                                 .await
//                                 .unwrap();
//                         }
//                         Events::RememberWatchedProgramsChangedEvent(app_id) => {
//                             let _listen_response = privacy_handler
//                                 .privacy_remember_watched_programs_changed(
//                                     ctx_provider.clone(),
//                                     ContentListenRequest {
//                                         app_id: app_id.clone(),
//                                         listen: true,
//                                     },
//                                 )
//                                 .await
//                                 .unwrap();
//                         }
//                         Events::ShareWatchHistoryChangedEvent(app_id) => {
//                             let _listen_response = privacy_handler
//                                 .privacy_share_watch_history_changed(
//                                     ctx_provider.clone(),
//                                     ContentListenRequest {
//                                         app_id: app_id.clone(),
//                                         listen: true,
//                                     },
//                                 )
//                                 .await
//                                 .unwrap();
//                         }
//                         Events::PolicyChangedEvent => {
//                             let _listen_response = discovery_handler
//                                 .on_policy_changed(
//                                     ctx_provider.clone(),
//                                     ListenRequest { listen: true },
//                                 )
//                                 .await
//                                 .unwrap();
//                         }
//                         Events::AllowAcrCollectionChangedEvent => {
//                             let _listen_response = privacy_handler
//                                 .privacy_allow_acr_collection_changed(
//                                     ctx_provider.clone(),
//                                     ListenRequest { listen: true },
//                                 )
//                                 .await
//                                 .unwrap();
//                         }
//                         Events::AllowAppContentAdTargetingChangedEvent => {
//                             let _listen_response = privacy_handler
//                                 .privacy_allow_app_content_ad_targeting_changed(
//                                     ctx_provider.clone(),
//                                     ListenRequest { listen: true },
//                                 )
//                                 .await
//                                 .unwrap();
//                         }
//                         Events::AllowCameraAnalyticsChangedEvent => {
//                             let _listen_response = privacy_handler
//                                 .privacy_allow_camera_analytics_changed(
//                                     ctx_provider.clone(),
//                                     ListenRequest { listen: true },
//                                 )
//                                 .await
//                                 .unwrap();
//                         }
//                         Events::AllowPersonalizationChangedEvent => {
//                             let _listen_response = privacy_handler
//                                 .privacy_allow_personalization_changed(
//                                     ctx_provider.clone(),
//                                     ListenRequest { listen: true },
//                                 )
//                                 .await
//                                 .unwrap();
//                         }
//                         Events::AllowPrimaryBrowseAdTargetingChangedEvent => {
//                             let _listen_response = privacy_handler
//                                 .privacy_allow_primary_browse_ad_targeting_changed(
//                                     ctx_provider.clone(),
//                                     ListenRequest { listen: true },
//                                 )
//                                 .await
//                                 .unwrap();
//                         }
//                         Events::AllowPrimaryContentAdTargetingChangedEvent => {
//                             let _listen_response = privacy_handler
//                                 .privacy_allow_primary_content_ad_targeting_changed(
//                                     ctx_provider.clone(),
//                                     ListenRequest { listen: true },
//                                 )
//                                 .await
//                                 .unwrap();
//                         }
//                         Events::AllowProductAnalyticsChangedEvent => {
//                             let _listen_response = privacy_handler
//                                 .privacy_allow_product_analytics_changed(
//                                     ctx_provider.clone(),
//                                     ListenRequest { listen: true },
//                                 )
//                                 .await
//                                 .unwrap();
//                         }
//                         Events::AllowRemoteDiagnosticsChangedEvent => {
//                             let _listen_response = privacy_handler
//                                 .privacy_allow_remote_diagnostics_changed(
//                                     ctx_provider.clone(),
//                                     ListenRequest { listen: true },
//                                 )
//                                 .await
//                                 .unwrap();
//                         }
//                         Events::AllowResumePointsChangedEvent => {
//                             let _listen_response = privacy_handler
//                                 .privacy_allow_resume_points_changed(
//                                     ctx_provider.clone(),
//                                     ListenRequest { listen: true },
//                                 )
//                                 .await
//                                 .unwrap();
//                         }
//                         Events::AllowUnentitledPersonalizationChangedEvent => {
//                             let _listen_response = privacy_handler
//                                 .privacy_allow_unentitled_personalization_changed(
//                                     ctx_provider.clone(),
//                                     ListenRequest { listen: true },
//                                 )
//                                 .await
//                                 .unwrap();
//                         }
//                         Events::AllowUnentitledResumePointsChangedEvent => {
//                             let _listen_response = privacy_handler
//                                 .privacy_allow_unentitled_resume_points_changed(
//                                     ctx_provider.clone(),
//                                     ListenRequest { listen: true },
//                                 )
//                                 .await
//                                 .unwrap();
//                         }
//                         Events::AllowWatchHistoryChangedEvent => {
//                             let _listen_response = privacy_handler
//                                 .privacy_allow_watch_history_changed(
//                                     ctx_provider.clone(),
//                                     ListenRequest { listen: true },
//                                 )
//                                 .await
//                                 .unwrap();
//                         }
//                     }
//                 }
//                 let _ = provider_rdy_tx.send(());

//                 while let Some(message) = session_rx.recv().await {
//                     debug!("Got message {}", message.jsonrpc_msg);
//                     if let Some(ref tx) = tx {
//                         let _ = tx.send(message.jsonrpc_msg).await.unwrap();
//                     }
//                 }
//             });
//             provider_rdy_rx.await.unwrap();
//         }
//     }

//     fn privacy_call_ctx() -> CallContext {
//         CallContext {
//             session_id: String::from("4e01c079-83c4-479a-ad6a-a9d055bd266d"),
//             request_id: String::from("abc"),
//             app_id: String::from("test-app"),
//             call_id: rand::random::<u64>(),
//             protocol: ApiProtocol::JsonRpc,
//             method: "privacy.testmethods".to_string(),
//         }
//     }

//     fn start_local_storage_service(mut rx: tokio::sync::mpsc::Receiver<DabRequest>) {
//         tokio::spawn(async move {
//             let mut local_storage: HashMap<String, StorageData> = HashMap::new();
//             while let Some(req) = rx.recv().await {
//                 match &req.payload {
//                     dab::core::message::DabRequestPayload::Storage(sreq) => match &sreq {
//                         dab::core::model::persistent_store::StorageRequest::Get(data) => {
//                             let key = (data.namespace.to_owned() + data.key.as_str()).to_owned();
//                             if let Some(data) = local_storage.get(key.as_str()) {
//                                 req.respond_and_log(Ok(DabResponsePayload::StorageData(
//                                     data.clone(),
//                                 )));
//                             } else {
//                                 req.respond_and_log(Err(DabError::OsError));
//                             }
//                         }

//                         dab::core::model::persistent_store::StorageRequest::Set(ssp) => {
//                             local_storage.insert(
//                                 (ssp.namespace.to_owned() + &ssp.key).to_owned(),
//                                 ssp.data.clone(),
//                             );
//                             req.respond_and_log(Ok(DabResponsePayload::JsonValue(json!(true))));
//                         }
//                     },
//                     _ => panic!("Not intended to handle request other than Storage!!"),
//                 }
//             }
//         });
//     }

//     #[tokio::test]
//     async fn test_policy_changed_event() {
//         let (tx, mut rx) = mpsc::channel::<String>(32);
//         let (mock_dab_tx, mock_dab_rx) = mpsc::channel::<DabRequest>(32);
//         let mut state = PlatformState::default();
//         state.services.sender_hub.dab_tx = Some(mock_dab_tx.clone());
//         let mut test_gateway = TestGateway::start(state.clone()).await;
//         let mut result_set = HashSet::new();
//         result_set.insert(Events::PolicyChangedEvent);
//         test_gateway.pin_helper.sender_hub.dab_tx = Some(mock_dab_tx.clone());
//         start_local_storage_service(mock_dab_rx);
//         let test_app = ContentPolicyApp::new(test_gateway.pin_helper.clone(), result_set.clone());
//         test_app
//             .start(test_gateway.prov_session_rx, state.clone(), Some(tx))
//             .await;

//         let handler = PrivacyImpl {
//             helper: test_gateway.pin_helper.clone(),
//             platform_state: state.clone(),
//         };
//         let ctx_cons = TestGateway::consumer_call();
//         let get_request = GetAppContentPolicy {
//             app_id: "app_provider".to_owned(),
//         };
//         let mut set_request = SetAppContentPolicy {
//             app_id: "app_provider".to_owned(),
//             value: true,
//         };

//         //set value to true
//         let resp = handler
//             .privacy_enable_recommendations_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_enable_recommendations_set response {:?}", resp);

//         let resp = handler
//             .privacy_enable_recommendations(ctx_cons.clone(), get_request.clone())
//             .await
//             .unwrap();
//         debug!("privacy_enable_recommendations response {:?}", resp);
//         assert_eq!(resp, true);

//         match timeout(Duration::from_millis(500), rx.recv()).await {
//             Ok(msg) => {
//                 assert!(msg.is_some());
//                 let obj: Value = serde_json::from_str(msg.unwrap().as_str()).unwrap();
//                 assert!(obj.get("result").is_some());
//                 let res_obj = obj.get("result").unwrap().clone();
//                 let result: ContentPolicy =
//                     serde_json::from_str(res_obj.to_string().as_str()).unwrap();
//                 assert_eq!(result.enable_recommendations, true);
//             }
//             Err(_) => {
//                 assert!(false, "Didn't receive policy change event");
//             }
//         }
//         //set value to false
//         set_request.value = false;
//         let resp = handler
//             .privacy_enable_recommendations_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_enable_recommendations_set response {:?}", resp);

//         let resp = handler
//             .privacy_enable_recommendations(ctx_cons.clone(), get_request.clone())
//             .await
//             .unwrap();
//         debug!("privacy_enable_recommendations response {:?}", resp);
//         match timeout(Duration::from_millis(500), rx.recv()).await {
//             Ok(msg) => {
//                 assert!(msg.is_some());
//                 let obj: Value = serde_json::from_str(msg.unwrap().as_str()).unwrap();
//                 assert!(obj.get("result").is_some());
//                 let res_obj = obj.get("result").unwrap().clone();
//                 let result: ContentPolicy =
//                     serde_json::from_str(res_obj.to_string().as_str()).unwrap();
//                 assert_eq!(result.enable_recommendations, false);
//             }
//             Err(_) => {
//                 assert!(false, "Didn't receive policy change event");
//             }
//         }
//     }

//     #[tokio::test]
//     async fn test_policy_changed_event_for_other_app() {
//         let (tx, mut rx) = mpsc::channel::<String>(32);
//         let (mock_dab_tx, mock_dab_rx) = mpsc::channel::<DabRequest>(32);
//         let mut state = PlatformState::default();
//         state.services.sender_hub.dab_tx = Some(mock_dab_tx.clone());
//         let mut test_gateway = TestGateway::start(state.clone()).await;
//         let mut result_set = HashSet::new();
//         result_set.insert(Events::PolicyChangedEvent);
//         test_gateway.pin_helper.sender_hub.dab_tx = Some(mock_dab_tx.clone());
//         start_local_storage_service(mock_dab_rx);
//         let test_app = ContentPolicyApp::new(test_gateway.pin_helper.clone(), result_set.clone());
//         test_app
//             .start(test_gateway.prov_session_rx, state.clone(), Some(tx))
//             .await;

//         let handler = PrivacyImpl {
//             helper: test_gateway.pin_helper.clone(),
//             platform_state: state,
//         };
//         let ctx_cons = TestGateway::consumer_call();
//         let get_request = GetAppContentPolicy {
//             app_id: "some-app".to_owned(),
//         };
//         let set_request = SetAppContentPolicy {
//             app_id: "some-app".to_owned(),
//             value: true,
//         };

//         //set value to true
//         let resp = handler
//             .privacy_enable_recommendations_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_enable_recommendations_set response {:?}", resp);

//         let resp = handler
//             .privacy_enable_recommendations(ctx_cons.clone(), get_request.clone())
//             .await
//             .unwrap();
//         debug!("privacy_enable_recommendations response {:?}", resp);
//         assert_eq!(resp, true);

//         match timeout(Duration::from_millis(500), rx.recv()).await {
//             Ok(_) => {
//                 assert!(false, "Received Policy change event happed for some-app");
//             }
//             Err(_) => {
//                 info!("Didn't receive policy change event as expected");
//             }
//         }
//     }

//     #[tokio::test]
//     async fn test_different_app_subscription() {
//         let (tx, mut rx) = mpsc::channel::<String>(32);
//         let (mock_dab_tx, mock_dab_rx) = mpsc::channel::<DabRequest>(32);
//         let mut state = PlatformState::default();
//         state.services.sender_hub.dab_tx = Some(mock_dab_tx.clone());
//         let mut test_gateway = TestGateway::start(state.clone()).await;
//         let mut result_set = HashSet::new();
//         result_set.insert(Events::EnableRecommendationsChangedEvent(Some(
//             "some-other-app".to_owned(),
//         )));
//         test_gateway.pin_helper.sender_hub.dab_tx = Some(mock_dab_tx.clone());
//         start_local_storage_service(mock_dab_rx);
//         let test_app = ContentPolicyApp::new(test_gateway.pin_helper.clone(), result_set.clone());
//         test_app
//             .start(test_gateway.prov_session_rx, state.clone(), Some(tx))
//             .await;

//         let handler = PrivacyImpl {
//             helper: test_gateway.pin_helper.clone(),
//             platform_state: state,
//         };
//         let ctx_cons = privacy_call_ctx();
//         let get_request = GetAppContentPolicy {
//             app_id: "some-app".to_owned(),
//         };
//         let mut set_request = SetAppContentPolicy {
//             app_id: "some-app".to_owned(),
//             value: true,
//         };

//         //set value to true
//         let resp = handler
//             .privacy_enable_recommendations_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_enable_recommendations_set response {:?}", resp);

//         let resp = handler
//             .privacy_enable_recommendations(ctx_cons.clone(), get_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_enable_recommendations response {:?}", resp);
//         assert_eq!(resp, true);

//         //set value to false
//         set_request.value = false;
//         let resp = handler
//             .privacy_enable_recommendations_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_enable_recommendations_set response {:?}", resp);

//         let resp = handler
//             .privacy_enable_recommendations(ctx_cons.clone(), get_request.clone())
//             .await
//             .unwrap();
//         assert_eq!(resp, false);
//         match timeout(Duration::from_millis(500), rx.recv()).await {
//             Ok(_) => {
//                 assert!(false, "Event delivered for wrong app")
//             }
//             Err(_) => {
//                 info!("Event not received as expected");
//             }
//         }
//     }

//     #[tokio::test]
//     async fn test_same_app_subscription() {
//         let (tx, mut rx) = mpsc::channel::<String>(32);
//         let (mock_dab_tx, mock_dab_rx) = mpsc::channel::<DabRequest>(32);
//         let mut state = PlatformState::default();
//         state.services.sender_hub.dab_tx = Some(mock_dab_tx.clone());
//         let mut test_gateway = TestGateway::start(state.clone()).await;
//         let mut result_set = HashSet::new();
//         result_set.insert(Events::EnableRecommendationsChangedEvent(Some(
//             "some-app".to_owned(),
//         )));
//         test_gateway.pin_helper.sender_hub.dab_tx = Some(mock_dab_tx.clone());
//         start_local_storage_service(mock_dab_rx);
//         let test_app = ContentPolicyApp::new(test_gateway.pin_helper.clone(), result_set.clone());
//         test_app
//             .start(test_gateway.prov_session_rx, state.clone(), Some(tx))
//             .await;

//         let handler = PrivacyImpl {
//             helper: test_gateway.pin_helper.clone(),
//             platform_state: state,
//         };
//         let ctx_cons = privacy_call_ctx();
//         let get_request = GetAppContentPolicy {
//             app_id: "some-app".to_owned(),
//         };
//         let mut set_request = SetAppContentPolicy {
//             app_id: "some-app".to_owned(),
//             value: true,
//         };

//         //set value to true
//         let resp = handler
//             .privacy_enable_recommendations_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_enable_recommendations_set response {:?}", resp);

//         let resp = handler
//             .privacy_enable_recommendations(ctx_cons.clone(), get_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_enable_recommendations response {:?}", resp);
//         assert_eq!(resp, true);

//         //set value to false
//         set_request.value = false;
//         let resp = handler
//             .privacy_enable_recommendations_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_enable_recommendations_set response {:?}", resp);

//         let resp = handler
//             .privacy_enable_recommendations(ctx_cons.clone(), get_request.clone())
//             .await
//             .unwrap();
//         assert_eq!(resp, false);
//         match timeout(Duration::from_millis(500), rx.recv()).await {
//             Ok(msg) => {
//                 info!("Event received: {:?} which is expected", msg);
//             }
//             Err(_) => {
//                 assert!(false, "App specific Event not delivered")
//             }
//         }
//     }

//     #[tokio::test]
//     async fn test_app_subscription_global_settings_change() {
//         let (tx, mut rx) = mpsc::channel::<String>(32);
//         let (mock_dab_tx, mock_dab_rx) = mpsc::channel::<DabRequest>(32);
//         let mut state = PlatformState::default();
//         state.services.sender_hub.dab_tx = Some(mock_dab_tx.clone());
//         let mut test_gateway = TestGateway::start(state.clone()).await;
//         let mut result_set = HashSet::new();
//         result_set.insert(Events::EnableRecommendationsChangedEvent(Some(
//             "some-app".to_owned(),
//         )));

//         test_gateway.pin_helper.sender_hub.dab_tx = Some(mock_dab_tx.clone());
//         start_local_storage_service(mock_dab_rx);
//         let test_app = ContentPolicyApp::new(test_gateway.pin_helper.clone(), result_set.clone());
//         test_app
//             .start(test_gateway.prov_session_rx, state.clone(), Some(tx))
//             .await;

//         let handler = PrivacyImpl {
//             helper: test_gateway.pin_helper.clone(),
//             platform_state: state,
//         };
//         let ctx_cons = privacy_call_ctx();
//         let get_request = GetAppContentPolicy {
//             app_id: "".to_owned(),
//         };
//         let mut set_request = SetAppContentPolicy {
//             app_id: "".to_owned(),
//             value: true,
//         };

//         //set value to true
//         let resp = handler
//             .privacy_enable_recommendations_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_enable_recommendations_set response {:?}", resp);

//         let resp = handler
//             .privacy_enable_recommendations(ctx_cons.clone(), get_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_enable_recommendations response {:?}", resp);
//         assert_eq!(resp, true);

//         //set value to false
//         set_request.value = false;
//         let resp = handler
//             .privacy_enable_recommendations_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_enable_recommendations_set response {:?}", resp);

//         let resp = handler
//             .privacy_enable_recommendations(ctx_cons.clone(), get_request.clone())
//             .await
//             .unwrap();
//         assert_eq!(resp, false);
//         match timeout(Duration::from_millis(500), rx.recv()).await {
//             Ok(msg) => {
//                 info!("Event received: {:?} which is expected", msg);
//                 assert!(
//                     false,
//                     "Global value change event delivered when subscribed for app specific"
//                 )
//             }
//             Err(_) => {
//                 info!("Event NOT received: which is expected");
//             }
//         }
//     }

//     #[tokio::test]
//     async fn test_global_subscription_app_settings_change() {
//         let (tx, mut rx) = mpsc::channel::<String>(32);
//         let (mock_dab_tx, mock_dab_rx) = mpsc::channel::<DabRequest>(32);
//         let mut state = PlatformState::default();
//         state.services.sender_hub.dab_tx = Some(mock_dab_tx.clone());
//         let mut test_gateway = TestGateway::start(state.clone()).await;
//         let mut result_set = HashSet::new();
//         result_set.insert(Events::EnableRecommendationsChangedEvent(None));
//         test_gateway.pin_helper.sender_hub.dab_tx = Some(mock_dab_tx.clone());
//         start_local_storage_service(mock_dab_rx);
//         let test_app = ContentPolicyApp::new(test_gateway.pin_helper.clone(), result_set.clone());
//         test_app
//             .start(test_gateway.prov_session_rx, state.clone(), Some(tx))
//             .await;

//         let handler = PrivacyImpl {
//             helper: test_gateway.pin_helper.clone(),
//             platform_state: state,
//         };
//         let ctx_cons = privacy_call_ctx();
//         let get_request = GetAppContentPolicy {
//             app_id: "".to_owned(),
//         };
//         let mut set_request = SetAppContentPolicy {
//             app_id: "".to_owned(),
//             value: true,
//         };
//         //set value to true
//         let resp = handler
//             .privacy_enable_recommendations_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_enable_recommendations_set response {:?}", resp);

//         let resp = handler
//             .privacy_enable_recommendations(ctx_cons.clone(), get_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_enable_recommendations response {:?}", resp);
//         assert_eq!(resp, true);

//         //set value to false
//         set_request.value = false;
//         let resp = handler
//             .privacy_enable_recommendations_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_enable_recommendations_set response {:?}", resp);

//         let resp = handler
//             .privacy_enable_recommendations(ctx_cons.clone(), get_request.clone())
//             .await
//             .unwrap();
//         debug!("privacy_enable_recommendations response {:?}", resp);
//         assert_eq!(resp, false);
//         match timeout(Duration::from_millis(500), rx.recv()).await {
//             Ok(msg) => {
//                 info!("Event received: {:?} which is expected", msg);
//             }
//             Err(_) => {
//                 assert!(false, "App specific value change event NOT delivered when subscribed for global change event")
//             }
//         }
//     }

//     #[tokio::test]
//     async fn test_privacy_enable_recommendations() {
//         let (tx, mut rx) = mpsc::channel::<String>(32);
//         let (mock_dab_tx, mock_dab_rx) = mpsc::channel::<DabRequest>(32);
//         let mut state = PlatformState::default();
//         state.services.sender_hub.dab_tx = Some(mock_dab_tx.clone());
//         let mut test_gateway = TestGateway::start(state.clone()).await;
//         let mut result_set = HashSet::new();
//         result_set.insert(Events::EnableRecommendationsChangedEvent(Some(
//             "some-app".to_owned(),
//         )));
//         test_gateway.pin_helper.sender_hub.dab_tx = Some(mock_dab_tx.clone());
//         start_local_storage_service(mock_dab_rx);
//         let test_app = ContentPolicyApp::new(test_gateway.pin_helper.clone(), result_set.clone());
//         test_app
//             .start(test_gateway.prov_session_rx, state.clone(), Some(tx))
//             .await;

//         let handler = PrivacyImpl {
//             helper: test_gateway.pin_helper.clone(),
//             platform_state: state,
//         };
//         let ctx_cons = privacy_call_ctx();
//         let get_request = GetAppContentPolicy {
//             app_id: "some-app".to_owned(),
//         };
//         let mut set_request = SetAppContentPolicy {
//             app_id: "some-app".to_owned(),
//             value: true,
//         };

//         //set value to true
//         let resp = handler
//             .privacy_enable_recommendations_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_enable_recommendations_set response {:?}", resp);

//         let resp = handler
//             .privacy_enable_recommendations(ctx_cons.clone(), get_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_enable_recommendations response {:?}", resp);
//         assert_eq!(resp, true);

//         //set value to false
//         set_request.value = false;
//         let resp = handler
//             .privacy_enable_recommendations_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_enable_recommendations_set response {:?}", resp);

//         let resp = handler
//             .privacy_enable_recommendations(ctx_cons.clone(), get_request.clone())
//             .await
//             .unwrap();
//         assert_eq!(resp, false);
//         if let Some(msg) = rx.recv().await {
//             info!("Received message: {}", msg);
//         }
//     }

//     #[tokio::test]
//     async fn test_privacy_remember_watched_programs() {
//         let (mock_dab_tx, mock_dab_rx) = mpsc::channel::<DabRequest>(32);
//         let mut state = PlatformState::default();
//         state.services.sender_hub.dab_tx = Some(mock_dab_tx.clone());
//         let mut test_gateway = TestGateway::start(state.clone()).await;
//         let mut result_set = HashSet::new();
//         result_set.insert(Events::RememberWatchedProgramsChangedEvent(Some(
//             "some-app".to_owned(),
//         )));
//         test_gateway.pin_helper.sender_hub.dab_tx = Some(mock_dab_tx.clone());
//         start_local_storage_service(mock_dab_rx);
//         let test_app = ContentPolicyApp::new(test_gateway.pin_helper.clone(), result_set.clone());
//         test_app
//             .start(test_gateway.prov_session_rx, state.clone(), None)
//             .await;

//         let handler = PrivacyImpl {
//             helper: test_gateway.pin_helper.clone(),
//             platform_state: state,
//         };
//         let ctx_cons = privacy_call_ctx();
//         let get_request = GetAppContentPolicy {
//             app_id: "some-app".to_owned(),
//         };
//         let mut set_request = SetAppContentPolicy {
//             app_id: "some-app".to_owned(),
//             value: true,
//         };

//         //set value to true
//         let resp = handler
//             .privacy_remember_watched_programs_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_remember_watched_programs_set response {:?}", resp);

//         let resp = handler
//             .privacy_remember_watched_programs(ctx_cons.clone(), get_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_remember_watched_programs response {:?}", resp);
//         assert_eq!(resp, true);

//         //set value to false
//         set_request.value = false;
//         let resp = handler
//             .privacy_remember_watched_programs_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_remember_watched_programs_set response {:?}", resp);

//         let resp = handler
//             .privacy_remember_watched_programs(ctx_cons.clone(), get_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_remember_watched_programs response {:?}", resp);
//         assert_eq!(resp, false);
//     }

//     #[tokio::test]
//     async fn test_privacy_share_watch_history() {
//         let helper = MockRippleBuilder::new()
//             .with_dab(vec![(
//                 &|req| match req {
//                     DabRequestPayload::Storage(sreq) => match sreq {
//                         StorageRequest::Set(ssp) => {
//                             let mut value: bool = ssp.data.value.as_bool().unwrap_or(true);
//                             if (ssp.key == "shareWatchHistory") {
//                                 unsafe {
//                                     WATCH_HISTORY = value;
//                                 }
//                             }
//                             Ok(DabResponsePayload::StorageData(StorageData::new(
//                                 Value::Bool(true),
//                             )))
//                         }
//                         StorageRequest::Get(data) => {
//                             let mut value: bool = true;
//                             if (data.key == "shareWatchHistory") {
//                                 unsafe {
//                                     value = WATCH_HISTORY;
//                                 }
//                             }
//                             Ok(DabResponsePayload::StorageData(StorageData::new(
//                                 Value::Bool(value),
//                             )))
//                         }
//                         _ => {
//                             debug!("sreq={:?}", sreq);
//                             Ok(DabResponsePayload::StorageData(StorageData::new(
//                                 Value::Bool(false),
//                             )))
//                         }
//                     },
//                     _ => panic!(),
//                 },
//                 12,
//             )])
//             .with_caps(vec![(&|_| CapResponse::CapCallbackResponse(Ok(())), 4)])
//             .build();
//         let platform_state = PlatformState::new(helper.clone());

//         let handler = PrivacyImpl {
//             helper: Box::new(helper),
//             platform_state,
//         };
//         let ctx_cons = privacy_call_ctx();
//         let get_request = GetAppContentPolicy {
//             app_id: "some-app".to_owned(),
//         };
//         let mut set_request = SetAppContentPolicy {
//             app_id: "some-app".to_owned(),
//             value: true,
//         };

//         //set value to true
//         let resp = handler
//             .privacy_share_watch_history_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_share_watch_history_set response {:?}", resp);
//         let resp = handler
//             .privacy_share_watch_history(ctx_cons.clone(), get_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_share_watch_history response {:?}", resp);
//         assert_eq!(resp, true);

//         //set value to false
//         set_request.value = false;
//         let resp = handler
//             .privacy_share_watch_history_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_share_watch_history_set response {:?}", resp);

//         let resp = handler
//             .privacy_share_watch_history(ctx_cons.clone(), get_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_share_watch_history response {:?}", resp);
//         assert_eq!(resp, false);
//     }

//     #[tokio::test]
//     async fn test_privacy_limit_ad_tracking() {
//         let (mock_dab_tx, mock_dab_rx) = mpsc::channel::<DabRequest>(32);
//         let mut state = PlatformState::default();
//         state.services.sender_hub.dab_tx = Some(mock_dab_tx.clone());
//         let mut test_gateway = TestGateway::start(state.clone()).await;
//         let mut result_set = HashSet::new();
//         result_set.insert(Events::LimitAdTrackingChangedEvent);
//         result_set.insert(Events::AdvertisingPolicyChangedEvent);
//         test_gateway.pin_helper.sender_hub.dab_tx = Some(mock_dab_tx.clone());
//         start_local_storage_service(mock_dab_rx);
//         let test_app = ContentPolicyApp::new(test_gateway.pin_helper.clone(), result_set.clone());
//         test_app
//             .start(test_gateway.prov_session_rx, state.clone(), None)
//             .await;

//         let handler = PrivacyImpl {
//             helper: test_gateway.pin_helper.clone(),
//             platform_state: state,
//         };
//         let ctx_cons = privacy_call_ctx();

//         let mut set_request = SetBoolProperty { value: true };

//         //set value to true
//         let resp = handler
//             .privacy_limit_ad_tracking_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_limit_ad_tracking_set response {:?}", resp);

//         let resp = handler
//             .privacy_limit_ad_tracking(ctx_cons.clone())
//             .await
//             .unwrap();
//         info!("privacy_limit_ad_tracking response {:?}", resp);
//         assert_eq!(resp, true);

//         //set value to false
//         set_request.value = false;
//         let resp = handler
//             .privacy_limit_ad_tracking_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_limit_ad_tracking_set response {:?}", resp);

//         let resp = handler
//             .privacy_limit_ad_tracking(ctx_cons.clone())
//             .await
//             .unwrap();
//         info!("limit_ad_tracking response {:?}", resp);
//         assert_eq!(resp, false);
//     }

//     #[tokio::test]
//     async fn test_default_limit_ad_tracking() {
//         let helper = MockRippleBuilder::new()
//             .with_dab(vec![(&|_| Err(DabError::IoError), 1)])
//             .build();
//         let platform_state = PlatformState::new(helper.clone());

//         let handler = PrivacyImpl {
//             helper: Box::new(helper),
//             platform_state,
//         };

//         let resp = handler.privacy_limit_ad_tracking(privacy_call_ctx()).await;
//         assert!(matches!(resp, Ok(true)));
//     }

//     #[tokio::test]
//     async fn test_default_enable_recommendations() {
//         let helper = MockRippleBuilder::new()
//             .with_dab(vec![(&|_| Err(DabError::IoError), 1)])
//             .build();
//         let platform_state = PlatformState::new(helper.clone());

//         let handler = PrivacyImpl {
//             helper: Box::new(helper),
//             platform_state,
//         };

//         let request = GetAppContentPolicy {
//             app_id: String::from("someAppId"),
//         };

//         let resp = handler
//             .privacy_enable_recommendations(privacy_call_ctx(), request)
//             .await;

//         assert!(matches!(resp, Ok(false)));
//     }

//     #[tokio::test]
//     async fn test_default_remember_watched_programs() {
//         let helper = MockRippleBuilder::new()
//             .with_dab(vec![(&|_| Err(DabError::IoError), 1)])
//             .build();
//         let platform_state = PlatformState::new(helper.clone());

//         let handler = PrivacyImpl {
//             helper: Box::new(helper),
//             platform_state,
//         };

//         let request = GetAppContentPolicy {
//             app_id: String::from("someAppId"),
//         };

//         let resp = handler
//             .privacy_remember_watched_programs(privacy_call_ctx(), request)
//             .await;
//         assert!(matches!(resp, Ok(false)));
//     }

//     #[tokio::test]
//     async fn test_default_share_watch_history() {
//         let helper = MockRippleBuilder::new()
//             .with_dab(vec![(&|_| Err(DabError::IoError), 1)])
//             .build();
//         let platform_state = PlatformState::new(helper.clone());

//         let handler = PrivacyImpl {
//             helper: Box::new(helper),
//             platform_state,
//         };

//         let request = GetAppContentPolicy {
//             app_id: String::from("someAppId"),
//         };

//         let resp = handler
//             .privacy_share_watch_history(privacy_call_ctx(), request)
//             .await;
//         assert!(matches!(resp, Ok(false)));
//     }

//     async fn start_test_app(events: Vec<Events>) -> PrivacyImpl<RippleHelper> {
//         let mut state = PlatformState::default();
//         let (mock_dab_tx, mock_dab_rx) = mpsc::channel::<DabRequest>(32);
//         state.services.sender_hub.dab_tx = Some(mock_dab_tx.clone());
//         let mut test_gateway = TestGateway::start(state.clone()).await;
//         let mut result_set = HashSet::new();

//         for event in events {
//             result_set.insert(event);
//         }

//         test_gateway.pin_helper.sender_hub.dab_tx = Some(mock_dab_tx.clone());
//         start_local_storage_service(mock_dab_rx);
//         let test_app = ContentPolicyApp::new(test_gateway.pin_helper.clone(), result_set.clone());
//         test_app
//             .start(test_gateway.prov_session_rx, state.clone(), None)
//             .await;

//         PrivacyImpl {
//             helper: test_app.helper,
//             platform_state: state,
//         }
//     }

//     #[tokio::test]
//     async fn test_privacy_allow_acr_collection() {
//         let handler = start_test_app(vec![Events::AllowAcrCollectionChangedEvent]).await;
//         let mut ctx_cons = privacy_call_ctx();
//         let mut set_request = SetBoolProperty { value: true };

//         //set value to true
//         ctx_cons.method = "privacy.setAllowACRCollection".to_owned();
//         let resp = handler
//             .privacy_allow_acr_collection_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_acr_collection_set response {:?}", resp);

//         ctx_cons.method = "privacy.allowACRCollection".to_owned();
//         let resp = handler
//             .privacy_allow_acr_collection(ctx_cons.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_acr_collection response {:?}", resp);
//         assert_eq!(resp, true);

//         //set value to false
//         ctx_cons.method = "privacy.setAllowACRCollection".to_owned();
//         set_request.value = false;
//         let resp = handler
//             .privacy_allow_acr_collection_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_acr_collection_set response {:?}", resp);

//         ctx_cons.method = "privacy.allowACRCollection".to_owned();
//         let resp = handler
//             .privacy_allow_acr_collection(ctx_cons.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_acr_collection response {:?}", resp);
//         assert_eq!(resp, false);
//     }

//     #[tokio::test]
//     async fn test_privacy_allow_app_content_ad_targeting() {
//         let handler = start_test_app(vec![Events::AllowAppContentAdTargetingChangedEvent]).await;
//         let mut ctx_cons = privacy_call_ctx();
//         let mut set_request = SetBoolProperty { value: true };

//         ctx_cons.method = "privacy.setAllowAppContentAdTargeting".to_owned();
//         //set value to true
//         let resp = handler
//             .privacy_allow_app_content_ad_targeting_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!(
//             "privacy_allow_app_content_ad_targeting_set response {:?}",
//             resp
//         );

//         ctx_cons.method = "privacy.allowAppContentAdTargeting".to_owned();
//         let resp = handler
//             .privacy_allow_app_content_ad_targeting(ctx_cons.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_app_content_ad_targeting response {:?}", resp);
//         assert_eq!(resp, true);

//         ctx_cons.method = "privacy.setAllowAppContentAdTargeting".to_owned();
//         //set value to false
//         set_request.value = false;
//         let resp = handler
//             .privacy_allow_app_content_ad_targeting_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!(
//             "privacy_allow_app_content_ad_targeting_set response {:?}",
//             resp
//         );

//         ctx_cons.method = "privacy.allowAppContentAdTargeting".to_owned();
//         let resp = handler
//             .privacy_allow_app_content_ad_targeting(ctx_cons.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_app_content_ad_targeting response {:?}", resp);
//         assert_eq!(resp, false);
//     }

//     #[tokio::test]
//     async fn test_privacy_allow_camera_analytics() {
//         let handler = start_test_app(vec![Events::AllowCameraAnalyticsChangedEvent]).await;
//         let mut ctx_cons = privacy_call_ctx();
//         let mut set_request = SetBoolProperty { value: true };

//         //set value to true
//         ctx_cons.method = "privacy.setAllowCameraAnalytics".to_owned();
//         let resp = handler
//             .privacy_allow_camera_analytics_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_camera_analytics_set response {:?}", resp);

//         ctx_cons.method = "privacy.allowCameraAnalytics".to_owned();
//         let resp = handler
//             .privacy_allow_camera_analytics(ctx_cons.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_camera_analytics response {:?}", resp);
//         assert_eq!(resp, true);

//         //set value to false
//         ctx_cons.method = "privacy.setAllowCameraAnalytics".to_owned();
//         set_request.value = false;
//         let resp = handler
//             .privacy_allow_camera_analytics_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_camera_analytics_set response {:?}", resp);

//         ctx_cons.method = "privacy.allowCameraAnalytics".to_owned();
//         let resp = handler
//             .privacy_allow_camera_analytics(ctx_cons.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_camera_analytics response {:?}", resp);
//         assert_eq!(resp, false);
//     }

//     #[tokio::test]
//     async fn test_privacy_allow_personalization() {
//         let handler = start_test_app(vec![Events::AllowPersonalizationChangedEvent]).await;
//         let mut ctx_cons = privacy_call_ctx();
//         let mut set_request = SetBoolProperty { value: true };

//         //set value to true
//         ctx_cons.method = "privacy.setAllowPersonalization".to_owned();
//         let resp = handler
//             .privacy_allow_personalization_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_personalization_set response {:?}", resp);

//         ctx_cons.method = "privacy.allowPersonalization".to_owned();
//         let resp = handler
//             .privacy_allow_personalization(ctx_cons.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_personalization response {:?}", resp);
//         assert_eq!(resp, true);

//         //set value to false
//         ctx_cons.method = "privacy.setAllowPersonalization".to_owned();
//         set_request.value = false;
//         let resp = handler
//             .privacy_allow_personalization_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_personalization_set response {:?}", resp);

//         ctx_cons.method = "privacy.allowPersonalization".to_owned();
//         let resp = handler
//             .privacy_allow_personalization(ctx_cons.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_personalization response {:?}", resp);
//         assert_eq!(resp, false);
//     }

//     #[tokio::test]
//     async fn test_privacy_allow_primary_browse_ad_targeting() {
//         let handler = start_test_app(vec![Events::AllowPrimaryBrowseAdTargetingChangedEvent]).await;
//         let mut ctx_cons = privacy_call_ctx();
//         let mut set_request = SetBoolProperty { value: true };

//         //set value to true
//         ctx_cons.method = "privacy.setAllowPrimaryBrowseAdTargeting".to_owned();
//         let resp = handler
//             .privacy_allow_primary_browse_ad_targeting_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!(
//             "privacy_allow_primary_browse_ad_targeting_set response {:?}",
//             resp
//         );

//         ctx_cons.method = "privacy.allowPrimaryBrowseAdTargeting".to_owned();
//         let resp = handler
//             .privacy_allow_primary_browse_ad_targeting(ctx_cons.clone())
//             .await
//             .unwrap();
//         info!(
//             "privacy_allow_primary_browse_ad_targeting response {:?}",
//             resp
//         );
//         assert_eq!(resp, true);

//         //set value to false
//         ctx_cons.method = "privacy.setAllowPrimaryBrowseAdTargeting".to_owned();
//         set_request.value = false;
//         let resp = handler
//             .privacy_allow_primary_browse_ad_targeting_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!(
//             "privacy_allow_primary_browse_ad_targeting_set response {:?}",
//             resp
//         );

//         ctx_cons.method = "privacy.allowPrimaryBrowseAdTargeting".to_owned();
//         let resp = handler
//             .privacy_allow_primary_browse_ad_targeting(ctx_cons.clone())
//             .await
//             .unwrap();
//         info!(
//             "privacy_allow_primary_browse_ad_targeting response {:?}",
//             resp
//         );
//         assert_eq!(resp, false);
//     }

//     #[tokio::test]
//     async fn test_privacy_allow_primary_content_ad_targeting() {
//         let handler =
//             start_test_app(vec![Events::AllowPrimaryContentAdTargetingChangedEvent]).await;
//         let mut ctx_cons = privacy_call_ctx();
//         let mut set_request = SetBoolProperty { value: true };

//         //set value to true
//         ctx_cons.method = "privacy.setAllowPrimaryContentAdTargeting".to_owned();
//         let resp = handler
//             .privacy_allow_primary_content_ad_targeting_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!(
//             "privacy_allow_primary_content_ad_targeting_set response {:?}",
//             resp
//         );

//         ctx_cons.method = "privacy.allowPrimaryContentAdTargeting".to_owned();
//         let resp = handler
//             .privacy_allow_primary_content_ad_targeting(ctx_cons.clone())
//             .await
//             .unwrap();
//         info!(
//             "privacy_allow_primary_content_ad_targeting response {:?}",
//             resp
//         );
//         assert_eq!(resp, true);

//         //set value to false
//         ctx_cons.method = "privacy.setAllowPrimaryContentAdTargeting".to_owned();
//         set_request.value = false;
//         let resp = handler
//             .privacy_allow_primary_content_ad_targeting_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!(
//             "privacy_allow_primary_content_ad_targeting_set response {:?}",
//             resp
//         );

//         ctx_cons.method = "privacy.allowPrimaryContentAdTargeting".to_owned();
//         let resp = handler
//             .privacy_allow_primary_content_ad_targeting(ctx_cons.clone())
//             .await
//             .unwrap();
//         info!(
//             "privacy_allow_primary_content_ad_targeting response {:?}",
//             resp
//         );
//         assert_eq!(resp, false);
//     }

//     #[tokio::test]
//     async fn test_privacy_allow_product_analytics() {
//         let handler = start_test_app(vec![Events::AllowProductAnalyticsChangedEvent]).await;
//         let mut ctx_cons = privacy_call_ctx();
//         let mut set_request = SetBoolProperty { value: true };

//         //set value to true
//         ctx_cons.method = "privacy.setAllowProductAnalytics".to_owned();
//         let resp = handler
//             .privacy_allow_product_analytics_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_product_analytics_set response {:?}", resp);

//         ctx_cons.method = "privacy.allowProductAnalytics".to_owned();
//         let resp = handler
//             .privacy_allow_product_analytics(ctx_cons.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_product_analytics response {:?}", resp);
//         assert_eq!(resp, true);

//         //set value to false
//         ctx_cons.method = "privacy.setAllowProductAnalytics".to_owned();
//         set_request.value = false;
//         let resp = handler
//             .privacy_allow_product_analytics_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_product_analytics_set response {:?}", resp);

//         ctx_cons.method = "privacy.allowProductAnalytics".to_owned();
//         let resp = handler
//             .privacy_allow_product_analytics(ctx_cons.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_product_analytics response {:?}", resp);
//         assert_eq!(resp, false);
//     }

//     #[tokio::test]
//     async fn test_privacy_allow_remote_diagnostics() {
//         let handler = start_test_app(vec![Events::AllowRemoteDiagnosticsChangedEvent]).await;
//         let mut ctx_cons = privacy_call_ctx();
//         let mut set_request = SetBoolProperty { value: true };

//         //set value to true
//         ctx_cons.method = "privacy.setAllowRemoteDiagnostics".to_owned();
//         let resp = handler
//             .privacy_allow_remote_diagnostics_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_remote_diagnostics_set response {:?}", resp);

//         ctx_cons.method = "privacy.allowRemoteDiagnostics".to_owned();
//         let resp = handler
//             .privacy_allow_remote_diagnostics(ctx_cons.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_remote_diagnostics response {:?}", resp);
//         assert_eq!(resp, true);

//         //set value to false
//         ctx_cons.method = "privacy.setAllowRemoteDiagnostics".to_owned();
//         set_request.value = false;
//         let resp = handler
//             .privacy_allow_remote_diagnostics_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_remote_diagnostics_set response {:?}", resp);

//         ctx_cons.method = "privacy.allowRemoteDiagnostics".to_owned();
//         let resp = handler
//             .privacy_allow_remote_diagnostics(ctx_cons.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_remote_diagnostics response {:?}", resp);
//         assert_eq!(resp, false);
//     }

//     #[tokio::test]
//     async fn test_privacy_allow_resume_points() {
//         let handler = start_test_app(vec![Events::AllowResumePointsChangedEvent]).await;
//         let mut ctx_cons = privacy_call_ctx();
//         let mut set_request = SetBoolProperty { value: true };

//         //set value to true
//         ctx_cons.method = "privacy.setAllowResumePoints".to_owned();
//         let resp = handler
//             .privacy_allow_resume_points_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_resume_points_set response {:?}", resp);

//         ctx_cons.method = "privacy.allowResumePoints".to_owned();
//         let resp = handler
//             .privacy_allow_resume_points(ctx_cons.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_resume_points response {:?}", resp);
//         assert_eq!(resp, true);

//         //set value to false
//         ctx_cons.method = "privacy.setAllowResumePoints".to_owned();
//         set_request.value = false;
//         let resp = handler
//             .privacy_allow_resume_points_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_resume_points_set response {:?}", resp);

//         ctx_cons.method = "privacy.allowResumePoints".to_owned();
//         let resp = handler
//             .privacy_allow_resume_points(ctx_cons.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_resume_points response {:?}", resp);
//         assert_eq!(resp, false);
//     }

//     #[tokio::test]
//     async fn test_privacy_allow_unentitled_personalization() {
//         let handler =
//             start_test_app(vec![Events::AllowUnentitledPersonalizationChangedEvent]).await;
//         let mut ctx_cons = privacy_call_ctx();
//         let mut set_request = SetBoolProperty { value: true };

//         //set value to true
//         ctx_cons.method = "privacy.setAllowUnentitledPersonalization".to_owned();
//         let resp = handler
//             .privacy_allow_unentitled_personalization_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!(
//             "privacy_allow_unentitled_personalization_set response {:?}",
//             resp
//         );

//         ctx_cons.method = "privacy.allowUnentitledPersonalization".to_owned();
//         let resp = handler
//             .privacy_allow_unentitled_personalization(ctx_cons.clone())
//             .await
//             .unwrap();
//         info!(
//             "privacy_allow_unentitled_personalization response {:?}",
//             resp
//         );
//         assert_eq!(resp, true);

//         //set value to false
//         ctx_cons.method = "privacy.setAllowUnentitledPersonalization".to_owned();
//         set_request.value = false;
//         let resp = handler
//             .privacy_allow_unentitled_personalization_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!(
//             "privacy_allow_unentitled_personalization_set response {:?}",
//             resp
//         );

//         ctx_cons.method = "privacy.allowUnentitledPersonalization".to_owned();
//         let resp = handler
//             .privacy_allow_unentitled_personalization(ctx_cons.clone())
//             .await
//             .unwrap();
//         info!(
//             "privacy_allow_unentitled_personalization response {:?}",
//             resp
//         );
//         assert_eq!(resp, false);
//     }

//     #[tokio::test]
//     async fn test_privacy_allow_unentitled_resume_points() {
//         let handler = start_test_app(vec![Events::AllowUnentitledResumePointsChangedEvent]).await;
//         let mut ctx_cons = privacy_call_ctx();
//         let mut set_request = SetBoolProperty { value: true };

//         //set value to true
//         ctx_cons.method = "privacy.setAllowUnentitledResumePoints".to_owned();
//         let resp = handler
//             .privacy_allow_unentitled_resume_points_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!(
//             "privacy_allow_unentitled_resume_points_set response {:?}",
//             resp
//         );

//         ctx_cons.method = "privacy.allowUnentitledResumePoints".to_owned();
//         let resp = handler
//             .privacy_allow_unentitled_resume_points(ctx_cons.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_unentitled_resume_points response {:?}", resp);
//         assert_eq!(resp, true);

//         //set value to false
//         ctx_cons.method = "privacy.setAllowUnentitledResumePoints".to_owned();
//         set_request.value = false;
//         let resp = handler
//             .privacy_allow_unentitled_resume_points_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!(
//             "privacy_allow_unentitled_resume_points_set response {:?}",
//             resp
//         );

//         ctx_cons.method = "privacy.allowUnentitledResumePoints".to_owned();
//         let resp = handler
//             .privacy_allow_unentitled_resume_points(ctx_cons.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_unentitled_resume_points response {:?}", resp);
//         assert_eq!(resp, false);
//     }

//     #[tokio::test]
//     async fn test_privacy_allow_watch_history() {
//         let handler = start_test_app(vec![Events::AllowWatchHistoryChangedEvent]).await;
//         let mut ctx_cons = privacy_call_ctx();
//         let mut set_request = SetBoolProperty { value: true };

//         //set value to true
//         ctx_cons.method = "privacy.setAllowWatchHistory".to_owned();
//         let resp = handler
//             .privacy_allow_watch_history_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_watch_history_set response {:?}", resp);

//         ctx_cons.method = "privacy.allowWatchHistory".to_owned();
//         let resp = handler
//             .privacy_allow_watch_history(ctx_cons.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_watch_history response {:?}", resp);
//         assert_eq!(resp, true);

//         //set value to false
//         ctx_cons.method = "privacy.setAllowWatchHistory".to_owned();
//         set_request.value = false;
//         let resp = handler
//             .privacy_allow_watch_history_set(ctx_cons.clone(), set_request.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_watch_history_set response {:?}", resp);

//         ctx_cons.method = "privacy.allowWatchHistory".to_owned();
//         let resp = handler
//             .privacy_allow_watch_history(ctx_cons.clone())
//             .await
//             .unwrap();
//         info!("privacy_allow_watch_history response {:?}", resp);
//         assert_eq!(resp, false);
//     }
// }
