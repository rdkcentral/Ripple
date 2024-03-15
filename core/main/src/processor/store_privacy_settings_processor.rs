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
use crate::state::platform_state::PlatformState;
use ripple_sdk::{
    api::{
        distributor::distributor_privacy::{PrivacySettingsData, PrivacySettingsStoreRequest},
        storage_property::StorageProperty::{
            self, AllowAcrCollection, AllowAppContentAdTargeting, AllowBusinessAnalytics,
            AllowCameraAnalytics, AllowPersonalization, AllowPrimaryBrowseAdTargeting,
            AllowPrimaryContentAdTargeting, AllowProductAnalytics, AllowRemoteDiagnostics,
            AllowResumePoints, AllowUnentitledPersonalization, AllowUnentitledResumePoints,
            AllowWatchHistory,
        },
    },
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::ExtnMessage,
        extn_client_message::ExtnResponse,
    },
    log::{debug, error},
    tokio::sync::mpsc::{Receiver as MReceiver, Sender as MSender},
    utils::error::RippleError,
};

#[derive(Debug)]
pub struct StorePrivacySettingsProcessor {
    state: PlatformState,
    streamer: DefaultExtnStreamer,
}

impl StorePrivacySettingsProcessor {
    pub fn new(state: PlatformState) -> StorePrivacySettingsProcessor {
        StorePrivacySettingsProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for StorePrivacySettingsProcessor {
    type STATE = PlatformState;
    type VALUE = PrivacySettingsStoreRequest;
    fn get_state(&self) -> Self::STATE {
        self.state.clone()
    }

    fn sender(&self) -> MSender<ExtnMessage> {
        self.streamer.sender()
    }

    fn receiver(&mut self) -> MReceiver<ExtnMessage> {
        self.streamer.receiver()
    }
}
impl StorePrivacySettingsProcessor {
    async fn process_set_request(
        state: &PlatformState,
        msg: ExtnMessage,
        storage_property: StorageProperty,
        value: bool,
    ) -> bool {
        let result = StorageManager::set_bool(state, storage_property, value, None).await;
        if result.is_ok() {
            Self::respond(
                state.get_client().get_extn_client(),
                msg,
                ExtnResponse::None(()),
            )
            .await
            .is_ok()
        } else {
            Self::handle_error(
                state.get_client().get_extn_client(),
                msg,
                RippleError::ProcessorError,
            )
            .await
        }
    }
    async fn process_get_request(
        state: &PlatformState,
        msg: ExtnMessage,
        storage_property: StorageProperty,
    ) -> bool {
        let result = StorageManager::get_bool(state, storage_property).await;
        match result {
            Ok(val) => Self::respond(
                state.get_client().get_extn_client(),
                msg,
                ExtnResponse::Boolean(val),
            )
            .await
            .is_ok(),
            Err(_err) => {
                Self::handle_error(
                    state.get_client().get_extn_client(),
                    msg,
                    RippleError::ProcessorError,
                )
                .await
            }
        }
    }
    async fn process_set_all_request(
        state: &PlatformState,
        msg: ExtnMessage,
        privacy_settings_data: PrivacySettingsData,
    ) -> bool {
        let mut err = false;
        macro_rules! set_property {
            ($property:ident, $value:expr) => {
                if let Some(value) = $value {
                    let res = StorageManager::set_bool(state, $property, value, None).await;
                    if let Err(e) = res {
                        error!("Unable to set property {:?} error: {:?}", $property, e);
                        err = true;
                    }
                }
            };
        }

        set_property!(
            AllowAcrCollection,
            privacy_settings_data.allow_acr_collection
        );
        set_property!(AllowResumePoints, privacy_settings_data.allow_resume_points);
        set_property!(
            AllowAppContentAdTargeting,
            privacy_settings_data.allow_app_content_ad_targeting
        );

        // business analytics is a special case, if it is not set, we set it to true
        if privacy_settings_data.allow_business_analytics.is_none() {
            set_property!(AllowBusinessAnalytics, Some(true));
        } else {
            set_property!(
                AllowBusinessAnalytics,
                privacy_settings_data.allow_business_analytics
            );
        }
        set_property!(
            AllowCameraAnalytics,
            privacy_settings_data.allow_camera_analytics
        );
        set_property!(
            AllowPersonalization,
            privacy_settings_data.allow_personalization
        );
        set_property!(
            AllowPrimaryBrowseAdTargeting,
            privacy_settings_data.allow_primary_browse_ad_targeting
        );
        set_property!(
            AllowPrimaryContentAdTargeting,
            privacy_settings_data.allow_primary_content_ad_targeting
        );
        set_property!(
            AllowProductAnalytics,
            privacy_settings_data.allow_product_analytics
        );
        set_property!(
            AllowRemoteDiagnostics,
            privacy_settings_data.allow_remote_diagnostics
        );
        set_property!(
            AllowUnentitledPersonalization,
            privacy_settings_data.allow_unentitled_personalization
        );
        set_property!(
            AllowUnentitledResumePoints,
            privacy_settings_data.allow_unentitled_resume_points
        );
        set_property!(AllowWatchHistory, privacy_settings_data.allow_watch_history);

        if err {
            return Self::handle_error(
                state.get_client().get_extn_client(),
                msg,
                RippleError::ProcessorError,
            )
            .await;
        }
        Self::respond(
            state.get_client().get_extn_client(),
            msg,
            ExtnResponse::None(()),
        )
        .await
        .is_ok()
    }
}

#[async_trait]
impl ExtnRequestProcessor for StorePrivacySettingsProcessor {
    fn get_client(&self) -> ripple_sdk::extn::client::extn_client::ExtnClient {
        self.state.get_client().get_extn_client()
    }

    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        debug!(
            "processor received extracted message: {:?}",
            extracted_message
        );
        match extracted_message {
            PrivacySettingsStoreRequest::GetPrivacySettings(storage_property) => {
                Self::process_get_request(&state, msg, storage_property).await
            }
            PrivacySettingsStoreRequest::SetAllPrivacySettings(privacy_settings_data) => {
                Self::process_set_all_request(&state, msg, privacy_settings_data).await
            }
            PrivacySettingsStoreRequest::SetPrivacySettings(storage_property, value) => {
                Self::process_set_request(&state, msg, storage_property, value).await
            }
        }
    }
}
