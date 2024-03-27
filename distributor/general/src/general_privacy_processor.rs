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

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use ripple_sdk::{
    api::distributor::distributor_privacy::{
        ExclusionPolicy, GetPropertyParams, PrivacyCloudRequest, PrivacySetting, PrivacySettings,
        SetPropertyParams,
    },
    async_trait::async_trait,
    extn::{
        client::{
            extn_client::ExtnClient,
            extn_processor::{
                DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
            },
        },
        extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnResponse},
    },
    framework::file_store::FileStore,
};
use serde::{Deserialize, Serialize};

pub struct DistributorPrivacyProcessor {
    state: PrivacyState,
    streamer: DefaultExtnStreamer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyData {
    settings: PrivacySettings,
    data_collection: HashMap<String, bool>,
    entitlement: HashMap<String, bool>,
}

#[derive(Debug, Clone)]
pub struct PrivacyState {
    client: ExtnClient,
    privacy_data: Arc<RwLock<FileStore<PrivacyData>>>,
}

impl PrivacyState {
    fn new(client: ExtnClient, path: String) -> Self {
        let path = get_privacy_path(path);
        let store = if let Ok(v) = FileStore::load(path.clone()) {
            v
        } else {
            FileStore::new(path, PrivacyData::new())
        };

        Self {
            client,
            privacy_data: Arc::new(RwLock::new(store)),
        }
    }

    fn get_property(&self, params: GetPropertyParams) -> bool {
        let data = self.privacy_data.read().unwrap();
        match params.setting {
            PrivacySetting::AppDataCollection(a) => data.value.get_data_collections(a),
            PrivacySetting::AppEntitlementCollection(e) => data.value.get_ent_collections(e),
            PrivacySetting::ContinueWatching => data.value.settings.allow_resume_points,
            PrivacySetting::UnentitledContinueWatching => {
                data.value.settings.allow_unentitled_resume_points
            }
            PrivacySetting::WatchHistory => data.value.settings.allow_watch_history,
            PrivacySetting::ProductAnalytics => data.value.settings.allow_product_analytics,
            PrivacySetting::Personalization => data.value.settings.allow_personalization,
            PrivacySetting::UnentitledPersonalization => {
                data.value.settings.allow_unentitled_personalization
            }
            PrivacySetting::RemoteDiagnostics => data.value.settings.allow_remote_diagnostics,
            PrivacySetting::PrimaryContentAdTargeting => {
                data.value.settings.allow_primary_content_ad_targeting
            }
            PrivacySetting::PrimaryBrowseAdTargeting => {
                data.value.settings.allow_primary_browse_ad_targeting
            }
            PrivacySetting::AppContentAdTargeting => {
                data.value.settings.allow_app_content_ad_targeting
            }
            PrivacySetting::Acr => data.value.settings.allow_acr_collection,
            PrivacySetting::CameraAnalytics => data.value.settings.allow_camera_analytics,
            PrivacySetting::BusinessAnalytics => data.value.settings.allow_business_analytics,
        }
    }

    fn set_property(&self, params: SetPropertyParams) -> bool {
        let mut data = self.privacy_data.write().unwrap();
        match params.setting.clone() {
            PrivacySetting::AppDataCollection(a) => {
                data.value.set_data_collections(a, params.value)
            }
            PrivacySetting::AppEntitlementCollection(e) => {
                data.value.set_ent_collections(e, params.value)
            }
            _ => data.value.set_setting(params.setting, params.value),
        }
        false
    }

    fn get_settings(&self) -> ExtnResponse {
        let data = self.privacy_data.read().unwrap();
        let value = data.value.settings.clone();
        if let ExtnPayload::Response(r) = value.get_extn_payload() {
            r
        } else {
            ExtnResponse::Value(serde_json::Value::Null)
        }
    }
}

impl PrivacyData {
    fn new() -> Self {
        Self {
            settings: PrivacySettings::new(),
            data_collection: HashMap::new(),
            entitlement: HashMap::new(),
        }
    }
    fn get_data_collections(&self, id: String) -> bool {
        self.data_collection.get(&id).cloned().unwrap_or(false)
    }

    fn get_ent_collections(&self, id: String) -> bool {
        self.entitlement.get(&id).cloned().unwrap_or(false)
    }

    fn set_data_collections(&mut self, id: String, data: bool) {
        self.data_collection.insert(id, data);
    }

    fn set_ent_collections(&mut self, id: String, data: bool) {
        self.entitlement.insert(id, data);
    }

    fn set_setting(&mut self, setting: PrivacySetting, data: bool) {
        match setting {
            PrivacySetting::ContinueWatching => self.settings.allow_resume_points = data,
            PrivacySetting::UnentitledContinueWatching => {
                self.settings.allow_unentitled_resume_points = data
            }
            PrivacySetting::WatchHistory => self.settings.allow_watch_history = data,
            PrivacySetting::ProductAnalytics => self.settings.allow_product_analytics = data,
            PrivacySetting::Personalization => self.settings.allow_personalization = data,
            PrivacySetting::UnentitledPersonalization => {
                self.settings.allow_unentitled_personalization = data
            }
            PrivacySetting::RemoteDiagnostics => self.settings.allow_remote_diagnostics = data,
            PrivacySetting::PrimaryContentAdTargeting => {
                self.settings.allow_primary_content_ad_targeting = data
            }
            PrivacySetting::PrimaryBrowseAdTargeting => {
                self.settings.allow_primary_browse_ad_targeting = data
            }
            PrivacySetting::AppContentAdTargeting => {
                self.settings.allow_app_content_ad_targeting = data
            }
            PrivacySetting::Acr => self.settings.allow_acr_collection = data,
            PrivacySetting::CameraAnalytics => self.settings.allow_camera_analytics = data,
            _ => {}
        }
    }
}

fn get_privacy_path(saved_dir: String) -> String {
    format!("{}/{}", saved_dir, "privacy_settings")
}

impl DistributorPrivacyProcessor {
    pub fn new(client: ExtnClient, path: String) -> DistributorPrivacyProcessor {
        DistributorPrivacyProcessor {
            state: PrivacyState::new(client, path),
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for DistributorPrivacyProcessor {
    type STATE = PrivacyState;
    type VALUE = PrivacyCloudRequest;

    fn get_state(&self) -> Self::STATE {
        self.state.clone()
    }

    fn receiver(
        &mut self,
    ) -> ripple_sdk::tokio::sync::mpsc::Receiver<ripple_sdk::extn::extn_client_message::ExtnMessage>
    {
        self.streamer.receiver()
    }

    fn sender(
        &self,
    ) -> ripple_sdk::tokio::sync::mpsc::Sender<ripple_sdk::extn::extn_client_message::ExtnMessage>
    {
        self.streamer.sender()
    }
}

#[async_trait]
impl ExtnRequestProcessor for DistributorPrivacyProcessor {
    fn get_client(&self) -> ExtnClient {
        self.state.client.clone()
    }
    async fn process_request(
        state: Self::STATE,
        msg: ripple_sdk::extn::extn_client_message::ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        match extracted_message {
            PrivacyCloudRequest::GetProperty(p) => {
                let resp = state.get_property(p);
                Self::respond(state.client.clone(), msg, ExtnResponse::Boolean(resp))
                    .await
                    .is_ok()
            }
            PrivacyCloudRequest::SetProperty(p) => {
                state.set_property(p);
                Self::ack(state.client.clone(), msg).await.is_ok()
            }
            PrivacyCloudRequest::GetProperties(_) => {
                Self::respond(state.client.clone(), msg, state.get_settings())
                    .await
                    .is_ok()
            }
            PrivacyCloudRequest::GetPartnerExclusions(_) => Self::respond(
                state.client.clone(),
                msg,
                if let ExtnPayload::Response(r) = ExclusionPolicy::default().get_extn_payload() {
                    r
                } else {
                    ExtnResponse::Value(serde_json::Value::Null)
                },
            )
            .await
            .is_ok(),
        }
    }
}
