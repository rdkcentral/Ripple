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
    log::{error, info},
};

use crate::state::platform_state::PlatformState;

use super::storage_manager::StorageManager;

use std::time::{SystemTime, UNIX_EPOCH};

fn get_current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .try_into()
        .unwrap()
}

static STORAGE_PROPERTIES: &[StorageProperty] = &[
    StorageProperty::AudioDescriptionEnabled,
    StorageProperty::PreferredAudioLanguages,
    StorageProperty::CCPreferredLanguages,
    StorageProperty::ClosedCaptionsEnabled,
];

#[derive(Debug, PartialEq)]
enum MigrationState {
    NotStarted,
    NotNeeded,
    Succeeded,
    Failed,
}

struct MigrationStatus {
    storage_property: StorageProperty,
    migration_state: MigrationState,
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
                migration_state: MigrationState::NotStarted,
            });
        }
        Self {
            platform_state: platform_state.clone(),
            migration_statuses,
        }
    }

    pub async fn migrate(&mut self) {
        println!("*** _DEBUG: StorageMigrator::migrate: entry");
        let start_time_ms = get_current_time_ms();

        for migration_status in &mut self.migration_statuses {
            if [MigrationState::NotStarted, MigrationState::Failed]
                .contains(&migration_status.migration_state)
            {
                migration_status.migration_state = match migration_status.storage_property {
                    StorageProperty::AudioDescriptionEnabled => {
                        migrate_audio_description_enabled(&self.platform_state).await
                    }
                    StorageProperty::PreferredAudioLanguages => {
                        migrate_preferred_audio_languages(&self.platform_state).await
                    }
                    StorageProperty::CCPreferredLanguages => {
                        migrate_preferred_cc_languages(&self.platform_state).await
                    }
                    StorageProperty::ClosedCaptionsEnabled => {
                        migrate_cc_enabled(&self.platform_state).await
                    }
                    _ => {
                        error!(
                            "migrate: Unsupported property: {:?}",
                            migration_status.storage_property
                        );
                        error!(
                            "*** _DEBUG: migrate: Unsupported property: {:?}",
                            migration_status.storage_property
                        );
                        MigrationState::Failed
                    }
                };
            }
        }

        info!(
            "migrate: Total time taken: {} ms",
            get_current_time_ms() - start_time_ms
        );
        info!(
            "*** _DEBUG: migrate: Total time taken: {} ms",
            get_current_time_ms() - start_time_ms
        );
    }
}

async fn migrate_audio_description_enabled(platform_state: &PlatformState) -> MigrationState {
    println!("*** _DEBUG: migrate_audio_description_enabled: entry");
    let mut migration_state = MigrationState::Failed;
    let start_time_ms = get_current_time_ms();

    if let Ok(enabled) =
        StorageManager::get_bool(platform_state, StorageProperty::AudioDescriptionEnabled).await
    {
        if let Ok(extn_menssage) = platform_state
            .get_client()
            .send_extn_request(UserSettingsRequest::SetAudioDescription(enabled))
            .await
        {
            if let Some(ExtnResponse::None(())) = extn_menssage.payload.as_response() {
                migration_state = MigrationState::Succeeded;
            }
        }
    }

    info!(
        "migrate_audio_description_enabled: migration_state={:?}, Time taken: {} ms",
        migration_state,
        get_current_time_ms() - start_time_ms
    );
    info!(
        "*** _DEBUG: migrate_audio_description_enabled: migration_state={:?}, Time taken: {} ms",
        migration_state,
        get_current_time_ms() - start_time_ms
    );

    migration_state
}

async fn migrate_preferred_audio_languages(platform_state: &PlatformState) -> MigrationState {
    println!("*** _DEBUG: migrate_preferred_audio_languages: entry");
    let mut migration_state = MigrationState::Failed;
    let start_time_ms = get_current_time_ms();

    let preferred_cc_languages =
        StorageManager::get_vec_string(platform_state, StorageProperty::PreferredAudioLanguages)
            .await
            .unwrap_or_default();

    println!(
        "*** _DEBUG: migrate_preferred_audio_languages: preferred_cc_languages={:?}",
        preferred_cc_languages
    );

    if let Ok(extn_menssage) = platform_state
        .get_client()
        .send_extn_request(UserSettingsRequest::SetPreferredAudioLanguages(
            preferred_cc_languages,
        ))
        .await
    {
        if let Some(ExtnResponse::None(())) = extn_menssage.payload.as_response() {
            migration_state = MigrationState::Succeeded;
        }
    }

    info!(
        "migrate_preferred_audio_languages: migration_state={:?}, Time taken: {} ms",
        migration_state,
        get_current_time_ms() - start_time_ms
    );
    info!(
        "*** _DEBUG: migrate_preferred_audio_languages: migration_state={:?}, Time taken: {} ms",
        migration_state,
        get_current_time_ms() - start_time_ms
    );

    migration_state
}

async fn migrate_preferred_cc_languages(platform_state: &PlatformState) -> MigrationState {
    println!("*** _DEBUG: migrate_preferred_cc_languages: entry");
    let mut migration_state = MigrationState::Failed;
    let start_time_ms = get_current_time_ms();

    let preferred_cc_languages =
        StorageManager::get_vec_string(platform_state, StorageProperty::CCPreferredLanguages)
            .await
            .unwrap_or_default();

    println!(
        "*** _DEBUG: migrate_preferred_cc_languages: preferred_cc_languages={:?}",
        preferred_cc_languages
    );

    if let Ok(extn_menssage) = platform_state
        .get_client()
        .send_extn_request(UserSettingsRequest::SetPreferredCaptionsLanguages(
            preferred_cc_languages,
        ))
        .await
    {
        if let Some(ExtnResponse::None(())) = extn_menssage.payload.as_response() {
            migration_state = MigrationState::Succeeded;
        }
    }

    info!(
        "migrate_preferred_cc_languages: migration_state={:?}, Time taken: {} ms",
        migration_state,
        get_current_time_ms() - start_time_ms
    );
    info!(
        "*** _DEBUG: migrate_preferred_cc_languages: migration_state={:?}, Time taken: {} ms",
        migration_state,
        get_current_time_ms() - start_time_ms
    );

    migration_state
}

async fn migrate_cc_enabled(platform_state: &PlatformState) -> MigrationState {
    println!("*** _DEBUG: migrate_cc_enabled: entry");
    let mut migration_state = MigrationState::Failed;
    let start_time_ms = get_current_time_ms();

    if let Ok(enabled) =
        StorageManager::get_bool(platform_state, StorageProperty::ClosedCaptionsEnabled).await
    {
        if let Ok(extn_menssage) = platform_state
            .get_client()
            .send_extn_request(UserSettingsRequest::SetClosedCaptionsEnabled(enabled))
            .await
        {
            if let Some(ExtnResponse::None(())) = extn_menssage.payload.as_response() {
                migration_state = MigrationState::Succeeded;
            }
        }
    }

    info!(
        "migrate_cc_enabled: migration_state={:?}, Time taken: {} ms",
        migration_state,
        get_current_time_ms() - start_time_ms
    );
    info!(
        "*** _DEBUG: migrate_cc_enabled: migration_state={:?}, Time taken: {} ms",
        migration_state,
        get_current_time_ms() - start_time_ms
    );

    migration_state
}
