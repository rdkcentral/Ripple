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

use ripple_sdk::{
    api::{device::device_user_settings::UserSettingsRequest, storage_property::StorageProperty},
    extn::extn_client_message::ExtnResponse,
    log::error,
};

use crate::state::platform_state::PlatformState;

use super::storage_manager::StorageManager;

static STORAGE_PROPERTIES: &[StorageProperty] = &[
    StorageProperty::AudioDescriptionEnabled,
    // StorageProperty::PreferredAudioLanguages,
    // StorageProperty::CCPreferredLanguages,
    // StorageProperty::ClosedCaptionsEnabled,
];

struct MigrationStatus {
    storage_property: StorageProperty,
    migrated: bool,
}

pub struct StorageMigrator {
    platform_state: PlatformState,
    migration_statuses: Vec<MigrationStatus>,
}

impl StorageMigrator {
    pub fn new(platform_state: &PlatformState) -> Self {
        let mut migration_statuses = vec![];
        for storage_property in STORAGE_PROPERTIES.iter() {
            migration_statuses.push(MigrationStatus {
                storage_property: storage_property.clone(),
                migrated: false,
            });
        }
        Self {
            platform_state: platform_state.clone(),
            migration_statuses,
        }
    }

    pub async fn migrate(&mut self) {
        println!("*** _DEBUG: StorageMigrator::migrate: entry");
        for migration_status in &mut self.migration_statuses {
            if !migration_status.migrated {
                let success = match migration_status.storage_property {
                    StorageProperty::AudioDescriptionEnabled => {
                        migrate_audio_description_enabled(&self.platform_state).await
                    }
                    _ => {
                        error!(
                            "migrate: Unsupported property: {:?}",
                            migration_status.storage_property
                        );
                        false
                    }
                };

                println!("migrate: success={}", success);

                if success {
                    migration_status.migrated = true;
                }
            }
        }
    }
}

async fn migrate_audio_description_enabled(platform_state: &PlatformState) -> bool {
    if let Ok(enabled) =
        StorageManager::get_bool(platform_state, StorageProperty::AudioDescriptionEnabled).await
    {
        let result = platform_state
            .get_client()
            .send_extn_request(UserSettingsRequest::SetAudioDescription(enabled))
            .await;

        println!(
            "*** _DEBUG: migrate_audio_description_enabled: result={:?}",
            result
        );

        match result {
            //Ok(msg) => msg.payload.extract().is_some(),
            Ok(msg) => {
                if let Some(ExtnResponse::Error(_)) = msg.payload.as_response() {
                    false
                } else {
                    true
                }
            }
            Err(_) => false,
        }
    } else {
        error!(
            "migrate_audio_description_enabled: Failed to retrieve value from persistent storage"
        );
        false
    }
}
