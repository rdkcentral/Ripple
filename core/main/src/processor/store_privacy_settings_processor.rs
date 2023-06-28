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
        distributor::distributor_privacy::CloudPrivacySettings, storage_property::StorageProperty,
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
    type VALUE = CloudPrivacySettings;
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
        let mut err = false;
        if let Some(allow_acr_collection) = extracted_message.allow_acr_collection {
            let res = StorageManager::set_bool(
                &state,
                StorageProperty::AllowAcrCollection,
                allow_acr_collection,
                None,
            )
            .await;
            if let Err(e) = res {
                error!("Unable to set property allow_resume_points error: {:?}", e);
                err = true;
            }
        }
        if let Some(allow_resume_points) = extracted_message.allow_resume_points {
            let res = StorageManager::set_bool(
                &state,
                StorageProperty::AllowResumePoints,
                allow_resume_points,
                None,
            )
            .await;
            if let Err(e) = res {
                error!("Unable to set property allow_resume_points error: {:?}", e);
                err = true;
            }
        }
        if let Some(allow_app_content_ad_targeting) =
            extracted_message.allow_app_content_ad_targeting
        {
            let res = StorageManager::set_bool(
                &state,
                StorageProperty::AllowAppContentAdTargeting,
                allow_app_content_ad_targeting,
                None,
            )
            .await;
            if let Err(e) = res {
                error!(
                    "Unable to set property allow_primary_browse_ad_targetting: {:?}",
                    e
                );
                err = true;
            }
        }
        if let Some(allow_camera_analytics) = extracted_message.allow_camera_analytics {
            let res = StorageManager::set_bool(
                &state,
                StorageProperty::AllowCameraAnalytics,
                allow_camera_analytics,
                None,
            )
            .await;
            if let Err(e) = res {
                error!(
                    "Unable to set property allow_primary_browse_ad_targetting: {:?}",
                    e
                );
                err = true;
            }
        }
        if let Some(allow_personalization) = extracted_message.allow_personalization {
            let res = StorageManager::set_bool(
                &state,
                StorageProperty::AllowPersonalization,
                allow_personalization,
                None,
            )
            .await;
            if let Err(e) = res {
                error!("Unable to set property allow_resume_points error: {:?}", e);
                err = true;
            }
        }
        if let Some(allow_primary_browse_ad_targeting) =
            extracted_message.allow_primary_browse_ad_targeting
        {
            let res = StorageManager::set_bool(
                &state,
                StorageProperty::AllowPrimaryBrowseAdTargeting,
                allow_primary_browse_ad_targeting,
                None,
            )
            .await;
            if let Err(e) = res {
                error!(
                    "Unable to set property allow_primary_browse_ad_targetting: {:?}",
                    e
                );
                err = true;
            }
        }
        if let Some(allow_primary_content_ad_targeting) =
            extracted_message.allow_primary_content_ad_targeting
        {
            let res = StorageManager::set_bool(
                &state,
                StorageProperty::AllowPrimaryContentAdTargeting,
                allow_primary_content_ad_targeting,
                None,
            )
            .await;
            if let Err(e) = res {
                error!(
                    "Unable to set property allow_primary_browse_ad_targetting: {:?}",
                    e
                );
                err = true;
            }
        }
        if let Some(allow_product_analytics) = extracted_message.allow_product_analytics {
            let res = StorageManager::set_bool(
                &state,
                StorageProperty::AllowProductAnalytics,
                allow_product_analytics,
                None,
            )
            .await;
            if let Err(e) = res {
                error!("Unable to set property allow_resume_points error: {:?}", e);
                err = true;
            }
        }
        if let Some(allow_remote_diagnostics) = extracted_message.allow_remote_diagnostics {
            let res = StorageManager::set_bool(
                &state,
                StorageProperty::AllowRemoteDiagnostics,
                allow_remote_diagnostics,
                None,
            )
            .await;
            if let Err(e) = res {
                error!(
                    "Unable to set property allow_primary_browse_ad_targetting: {:?}",
                    e
                );
                err = true;
            }
        }
        if let Some(allow_unentitled_personalization) =
            extracted_message.allow_unentitled_personalization
        {
            let res = StorageManager::set_bool(
                &state,
                StorageProperty::AllowUnentitledPersonalization,
                allow_unentitled_personalization,
                None,
            )
            .await;
            if let Err(e) = res {
                error!(
                    "Unable to set property allow_primary_browse_ad_targetting: {:?}",
                    e
                );
                err = true;
            }
        }
        if let Some(allow_unentitled_resume_points) =
            extracted_message.allow_unentitled_resume_points
        {
            let res = StorageManager::set_bool(
                &state,
                StorageProperty::AllowUnentitledResumePoints,
                allow_unentitled_resume_points,
                None,
            )
            .await;
            if let Err(e) = res {
                error!("Unable to set property allow_resume_points error: {:?}", e);
                err = true;
            }
        }
        if let Some(allow_watch_history) = extracted_message.allow_watch_history {
            let res = StorageManager::set_bool(
                &state,
                StorageProperty::AllowWatchHistory,
                allow_watch_history,
                None,
            )
            .await;
            if let Err(e) = res {
                error!(
                    "Unable to set property allow_primary_browse_ad_targetting: {:?}",
                    e
                );
                err = true;
            }
        }

        if err {
            return Self::handle_error(
                state.get_client().get_extn_client(),
                msg,
                RippleError::ProcessorError,
            )
            .await;
        }
        return Self::respond(
            state.get_client().get_extn_client(),
            msg,
            ExtnResponse::None(()),
        )
        .await
        .is_ok();
    }
}
