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

use std::collections::HashMap;

use crate::{
    processor::storage::{
        storage_manager::{StorageManager, StorageManagerError},
        storage_property::StorageProperty,
    },
    state::platform_state::PlatformState,
};

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
pub struct PrivacyImpl<IRippleHelper> {
    pub helper: Box<IRippleHelper>,
    pub platform_state: PlatformState,
}

impl PrivacyImpl<RippleHelper> {
    async fn send_policy_changed_event(&self, ctx: &CallContext, app_id: String) {
        let event_data =
            DiscoveryImpl::get_content_policy(&ctx, &self.platform_state, &app_id.clone())
                .await
                .unwrap();
        AppEvents::emit_with_context(
            &self.platform_state,
            "discovery.onPolicyChanged",
            &serde_json::to_value(event_data).unwrap(),
            Some(serde_json::Value::String(app_id)),
        )
        .await;
    }

    async fn on_content_policy_changed(
        &self,
        ctx: CallContext,
        event_name: &'static str,
        request: ContentListenRequest,
    ) -> RpcResult<ListenerResponse> {
        // TODO: Check config for storage type? Are we supporting listeners for cloud settings?
        let event_context = request.app_id.map(|x| {
            json!({
                "appId": x,
            })
        });

        AppEvents::add_listener_with_context(
            &&self.platform_state.app_events_state,
            event_name.to_owned(),
            ctx,
            ListenRequest {
                listen: request.listen,
            },
            event_context,
        );

        Ok(ListenerResponse {
            listening: request.listen,
            event: event_name,
        })
    }

    pub async fn get_allow_app_content_ad_targeting(state: &PlatformState) -> bool {
        StorageManager::get_bool(state, StorageProperty::AllowAppContentAdTargeting, true)
            .await
            .unwrap_or(false)
    }

    pub async fn get_allow_personalization(state: &PlatformState, app_id: &str) -> bool {
        StorageManager::get_bool(state, StorageProperty::AllowPersonalization, true)
            .await
            .unwrap_or(false)
    }

    pub async fn get_share_watch_history(
        ctx: &CallContext,
        state: &PlatformState,
        app_id: &str,
    ) -> bool {
        let helper = Box::new(state.services.clone());
        let watch_granted = is_granted(
            &helper,
            ctx,
            "xrn:firebolt:capability:discovery:watched",
            None,
        )
        .await;
        debug!("watch_granted={}", watch_granted);
        watch_granted
    }

    pub async fn get_allow_watch_history(state: &PlatformState, app_id: &str) -> bool {
        StorageManager::get_bool(state, StorageProperty::AllowWatchHistory, true)
            .await
            .unwrap_or(false)
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
        Self::get_bool(&self.platform_state, property, true).await
    }

    pub async fn get_bool(
        platform_state: &PlatformState,
        property: StorageProperty,
        fill_default: bool,
    ) -> RpcResult<bool> {
        match platform_state
            .services
            .get_config()
            .get_features()
            .privacy_settings_storage_type
        {
            PrivacySettingsStorageType::Local => {
                StorageManager::get_bool(platform_state, property, fill_default).await
            }
            PrivacySettingsStorageType::Cloud => {
                PrivacyCloud::get_bool(platform_state, property).await
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
        match platform_state
            .services
            .get_config()
            .get_features()
            .privacy_settings_storage_type
        {
            PrivacySettingsStorageType::Local => {
                StorageManager::set_bool(platform_state, property, value, None).await
            }
            PrivacySettingsStorageType::Cloud => {
                PrivacyCloud::set_bool(platform_state, property, value).await
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
impl PrivacyServer for PrivacyImpl<RippleHelper> {
    async fn privacy_allow_acr_collection(&self, ctx: CallContext) -> RpcResult<bool> {
        Self::handle_allow_get_requests(&ctx.method, &self.platform_state, true).await
    }

    #[instrument(skip(self))]
    async fn privacy_allow_acr_collection_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        Self::handle_allow_set_requests(&ctx.method, &self.platform_state, set_request).await
    }

    #[instrument(skip(self))]
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
        Self::handle_allow_get_requests(&ctx.method, &self.platform_state, true).await
    }

    #[instrument(skip(self))]
    async fn privacy_allow_app_content_ad_targeting_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        Self::handle_allow_set_requests(&ctx.method, &self.platform_state, set_request).await
    }

    #[instrument(skip(self))]
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

    #[instrument(skip(self))]
    async fn privacy_allow_camera_analytics(&self, ctx: CallContext) -> RpcResult<bool> {
        Self::handle_allow_get_requests(&ctx.method, &self.platform_state, true).await
    }

    #[instrument(skip(self))]
    async fn privacy_allow_camera_analytics_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        Self::handle_allow_set_requests(&ctx.method, &self.platform_state, set_request).await
    }

    #[instrument(skip(self))]
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

    #[instrument(skip(self))]
    async fn privacy_allow_personalization(&self, ctx: CallContext) -> RpcResult<bool> {
        Self::handle_allow_get_requests(&ctx.method, &self.platform_state, true).await
    }

    #[instrument(skip(self))]
    async fn privacy_allow_personalization_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        Self::handle_allow_set_requests(&ctx.method, &self.platform_state, set_request).await
    }

    #[instrument(skip(self))]
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

    #[instrument(skip(self))]
    async fn privacy_allow_primary_browse_ad_targeting(&self, ctx: CallContext) -> RpcResult<bool> {
        Self::handle_allow_get_requests(&ctx.method, &self.platform_state, true).await
    }

    #[instrument(skip(self))]
    async fn privacy_allow_primary_browse_ad_targeting_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        Self::handle_allow_set_requests(&ctx.method, &self.platform_state, set_request).await
    }

    #[instrument(skip(self))]
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

    #[instrument(skip(self))]
    async fn privacy_allow_primary_content_ad_targeting(
        &self,
        ctx: CallContext,
    ) -> RpcResult<bool> {
        Self::handle_allow_get_requests(&ctx.method, &self.platform_state, true).await
    }

    #[instrument(skip(self))]
    async fn privacy_allow_primary_content_ad_targeting_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        Self::handle_allow_set_requests(&ctx.method, &self.platform_state, set_request).await
    }

    #[instrument(skip(self))]
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

    #[instrument(skip(self))]
    async fn privacy_allow_product_analytics(&self, ctx: CallContext) -> RpcResult<bool> {
        Self::handle_allow_get_requests(&ctx.method, &self.platform_state, true).await
    }

    #[instrument(skip(self))]
    async fn privacy_allow_product_analytics_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        Self::handle_allow_set_requests(&ctx.method, &self.platform_state, set_request).await
    }

    #[instrument(skip(self))]
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

    #[instrument(skip(self))]
    async fn privacy_allow_remote_diagnostics(&self, ctx: CallContext) -> RpcResult<bool> {
        Self::handle_allow_get_requests(&ctx.method, &self.platform_state, true).await
    }

    #[instrument(skip(self))]
    async fn privacy_allow_remote_diagnostics_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        Self::handle_allow_set_requests(&ctx.method, &self.platform_state, set_request).await
    }

    #[instrument(skip(self))]
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

    #[instrument(skip(self))]
    async fn privacy_allow_resume_points(&self, ctx: CallContext) -> RpcResult<bool> {
        Self::handle_allow_get_requests(&ctx.method, &self.platform_state, true).await
    }

    #[instrument(skip(self))]
    async fn privacy_allow_resume_points_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        Self::handle_allow_set_requests(&ctx.method, &self.platform_state, set_request).await
    }

    #[instrument(skip(self))]
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

    #[instrument(skip(self))]
    async fn privacy_allow_unentitled_personalization(&self, ctx: CallContext) -> RpcResult<bool> {
        Self::handle_allow_get_requests(&ctx.method, &self.platform_state, true).await
    }

    #[instrument(skip(self))]
    async fn privacy_allow_unentitled_personalization_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        Self::handle_allow_set_requests(&ctx.method, &self.platform_state, set_request).await
    }

    #[instrument(skip(self))]
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

    #[instrument(skip(self))]
    async fn privacy_allow_unentitled_resume_points(&self, ctx: CallContext) -> RpcResult<bool> {
        Self::handle_allow_get_requests(&ctx.method, &self.platform_state, true).await
    }

    #[instrument(skip(self))]
    async fn privacy_allow_unentitled_resume_points_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        Self::handle_allow_set_requests(&ctx.method, &self.platform_state, set_request).await
    }

    #[instrument(skip(self))]
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

    #[instrument(skip(self))]
    async fn privacy_allow_watch_history(&self, ctx: CallContext) -> RpcResult<bool> {
        Self::handle_allow_get_requests(&ctx.method, &self.platform_state, true).await
    }

    #[instrument(skip(self))]
    async fn privacy_allow_watch_history_set(
        &self,
        ctx: CallContext,
        set_request: SetBoolProperty,
    ) -> RpcResult<()> {
        Self::handle_allow_set_requests(&ctx.method, &self.platform_state, set_request).await
    }

    #[instrument(skip(self))]
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

    #[instrument(skip(self))]
    async fn get_settings(&self, _ctx: CallContext) -> RpcResult<PrivacySettings> {
        match self
            .helper
            .get_config()
            .get_features()
            .privacy_settings_storage_type
        {
            PrivacySettingsStorageType::Local => self.get_settings_local().await,
            PrivacySettingsStorageType::Cloud => {
                let resp = PrivacyCloud::get_settings(&self.platform_state).await?;
                Ok(PrivacySettings::new(resp))
            }
            PrivacySettingsStorageType::Sync => Err(jsonrpsee::core::Error::Custom(String::from(
                "PrivacySettingsStorageType::Sync: Unimplemented",
            ))),
        }
    }
}

pub struct PrivacyRippleProvider;
pub struct PrivacyCapHandler;

impl IGetLoadedCaps for PrivacyCapHandler {
    fn get_loaded_caps(&self) -> RippleHandlerCaps {
        RippleHandlerCaps {
            caps: Some(vec![CapClassifiedRequest::Supported(vec![
                FireboltCap::Short("privacy:allowPersonalization".into()),
                FireboltCap::Short("privacy:allowAppContentAdTargeting".into()),
                FireboltCap::Short("privacy:allowWatchHistory".into()),
                FireboltCap::Short("privacy:shareWatchHistory".into()),
            ])]),
        }
    }
}

impl RPCProvider<PrivacyImpl<RippleHelper>, PrivacyCapHandler> for PrivacyRippleProvider {
    fn provide(
        self,
        rhf: Box<RippleHelperFactory>,
        platform_state: PlatformState,
    ) -> (RpcModule<PrivacyImpl<RippleHelper>>, PrivacyCapHandler) {
        let a = PrivacyImpl {
            helper: rhf.clone().get(self.get_helper_variant()),
            platform_state,
        };
        (a.into_rpc(), PrivacyCapHandler)
    }

    fn get_helper_variant(self) -> Vec<RippleHelperType> {
        vec![
            RippleHelperType::Cap,
            RippleHelperType::Dab,
            RippleHelperType::Dpab,
        ]
    }
}

fn get_namespace(app_id: &str) -> String {
    if app_id.is_empty() {
        String::from("Privacy")
    } else {
        format!("Privacy.{}", app_id)
    }
}
