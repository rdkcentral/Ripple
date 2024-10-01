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
    StorageProperty::ClosedCaptionsFontFamily,
    StorageProperty::ClosedCaptionsFontSize,
    StorageProperty::ClosedCaptionsFontColor,
    StorageProperty::ClosedCaptionsFontEdge,
    StorageProperty::ClosedCaptionsFontEdgeColor,
    StorageProperty::ClosedCaptionsFontOpacity,
    StorageProperty::ClosedCaptionsBackgroundColor,
    StorageProperty::ClosedCaptionsBackgroundOpacity,
    StorageProperty::ClosedCaptionsWindowColor,
    StorageProperty::ClosedCaptionsWindowOpacity,
    // NOTE: Neither UserSettings nor TextTrack supports these:
    // StorageProperty::ClosedCaptionsTextAlign,
    // StorageProperty::ClosedCaptionsTextAlignVertical,
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

    pub async fn migrate(&mut self) -> u64 {
        info!("migrate: entry");
        let start_time_ms = get_current_time_ms();

        for migration_status in &mut self.migration_statuses {
            migration_status.migration_state =
                migrate_property(&self.platform_state, &migration_status.storage_property).await;
        }

        let total_duration_ms = get_current_time_ms() - start_time_ms;
        info!("migrate: Total duration: {} ms", total_duration_ms);
        total_duration_ms
    }

    const TEST_ITERATIONS: u64 = 10;
    pub async fn migration_test(&mut self) {
        let mut duration_ms = 0;
        for i in 1..Self::TEST_ITERATIONS + 1 {
            info!("migration_test: Iteration: {}", i);
            duration_ms += self.migrate().await;
        }
        info!(
            "migration_test: Average duration: {} ms",
            duration_ms / Self::TEST_ITERATIONS
        );
    }
}

async fn migrate_property(
    platform_state: &PlatformState,
    storage_property: &StorageProperty,
) -> MigrationState {
    let mut migration_state = MigrationState::Failed;
    let start_time_ms = get_current_time_ms();

    match storage_property {
        StorageProperty::AudioDescriptionEnabled => {
            migration_state = migrate_audio_description_enabled(platform_state).await;
        }
        StorageProperty::PreferredAudioLanguages => {
            migration_state = migrate_preferred_audio_languages(platform_state).await;
        }
        StorageProperty::CCPreferredLanguages => {
            migration_state = migrate_preferred_cc_languages(platform_state).await;
        }
        StorageProperty::ClosedCaptionsEnabled => {
            migration_state = migrate_cc_enabled(platform_state).await;
        }
        StorageProperty::ClosedCaptionsFontFamily => {
            migration_state = migrate_font_family(platform_state).await;
        }
        StorageProperty::ClosedCaptionsFontSize => {
            migration_state = migrate_font_size(platform_state).await;
        }
        StorageProperty::ClosedCaptionsFontColor => {
            migration_state = migrate_font_color(platform_state).await;
        }
        StorageProperty::ClosedCaptionsFontEdge => {
            migration_state = migrate_font_edge(platform_state).await;
        }
        StorageProperty::ClosedCaptionsFontEdgeColor => {
            migration_state = migrate_font_edge_color(platform_state).await;
        }
        StorageProperty::ClosedCaptionsFontOpacity => {
            migration_state = migrate_font_opacity(platform_state).await;
        }
        StorageProperty::ClosedCaptionsBackgroundColor => {
            migration_state = migrate_background_color(platform_state).await;
        }
        StorageProperty::ClosedCaptionsBackgroundOpacity => {
            migration_state = migrate_background_opacity(platform_state).await;
        }
        StorageProperty::ClosedCaptionsWindowColor => {
            migration_state = migrate_window_color(platform_state).await;
        }
        StorageProperty::ClosedCaptionsWindowOpacity => {
            migration_state = migrate_window_opacity(platform_state).await;
        }
        _ => {
            error!(
                "migrate_property: Unsupported property: {:?}",
                storage_property
            );
        }
    }

    info!(
        "migrate_property: storage_property={:?}, migration_state={:?}, Time taken: {} ms",
        storage_property,
        migration_state,
        get_current_time_ms() - start_time_ms
    );

    migration_state
}

async fn migrate_audio_description_enabled(platform_state: &PlatformState) -> MigrationState {
    let mut migration_state = MigrationState::Failed;

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

    migration_state
}

async fn migrate_preferred_audio_languages(platform_state: &PlatformState) -> MigrationState {
    let mut migration_state = MigrationState::Failed;

    let preferred_cc_languages =
        StorageManager::get_vec_string(platform_state, StorageProperty::PreferredAudioLanguages)
            .await
            .unwrap_or_default();

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

    migration_state
}

async fn migrate_preferred_cc_languages(platform_state: &PlatformState) -> MigrationState {
    let mut migration_state = MigrationState::Failed;

    let preferred_cc_languages =
        StorageManager::get_vec_string(platform_state, StorageProperty::CCPreferredLanguages)
            .await
            .unwrap_or_default();

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

    migration_state
}

async fn migrate_cc_enabled(platform_state: &PlatformState) -> MigrationState {
    let mut migration_state = MigrationState::Failed;

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

    migration_state
}

async fn migrate_font_family(platform_state: &PlatformState) -> MigrationState {
    let mut migration_state = MigrationState::Failed;

    let family =
        StorageManager::get_string(platform_state, StorageProperty::ClosedCaptionsFontFamily)
            .await
            .unwrap_or_default();
    if let Ok(extn_menssage) = platform_state
        .get_client()
        .send_extn_request(UserSettingsRequest::SetClosedCaptionsFontFamily(family))
        .await
    {
        if let Some(ExtnResponse::None(())) = extn_menssage.payload.as_response() {
            migration_state = MigrationState::Succeeded;
        }
    }

    migration_state
}

async fn migrate_font_size(platform_state: &PlatformState) -> MigrationState {
    let mut migration_state = MigrationState::Failed;

    let size = StorageManager::get_string(platform_state, StorageProperty::ClosedCaptionsFontSize)
        .await
        .unwrap_or_default();
    if let Ok(extn_menssage) = platform_state
        .get_client()
        .send_extn_request(UserSettingsRequest::SetClosedCaptionsFontSize(size))
        .await
    {
        if let Some(ExtnResponse::None(())) = extn_menssage.payload.as_response() {
            migration_state = MigrationState::Succeeded;
        }
    }

    migration_state
}

async fn migrate_font_color(platform_state: &PlatformState) -> MigrationState {
    let mut migration_state = MigrationState::Failed;

    let color =
        StorageManager::get_string(platform_state, StorageProperty::ClosedCaptionsFontColor)
            .await
            .unwrap_or_default();
    if let Ok(extn_menssage) = platform_state
        .get_client()
        .send_extn_request(UserSettingsRequest::SetClosedCaptionsFontColor(color))
        .await
    {
        if let Some(ExtnResponse::None(())) = extn_menssage.payload.as_response() {
            migration_state = MigrationState::Succeeded;
        }
    }

    migration_state
}

async fn migrate_font_edge(platform_state: &PlatformState) -> MigrationState {
    let mut migration_state = MigrationState::Failed;

    let edge = StorageManager::get_string(platform_state, StorageProperty::ClosedCaptionsFontEdge)
        .await
        .unwrap_or_default();
    if let Ok(extn_menssage) = platform_state
        .get_client()
        .send_extn_request(UserSettingsRequest::SetClosedCaptionsFontEdge(edge))
        .await
    {
        if let Some(ExtnResponse::None(())) = extn_menssage.payload.as_response() {
            migration_state = MigrationState::Succeeded;
        }
    }

    migration_state
}

async fn migrate_font_edge_color(platform_state: &PlatformState) -> MigrationState {
    let mut migration_state = MigrationState::Failed;

    let color =
        StorageManager::get_string(platform_state, StorageProperty::ClosedCaptionsFontEdgeColor)
            .await
            .unwrap_or_default();
    if let Ok(extn_menssage) = platform_state
        .get_client()
        .send_extn_request(UserSettingsRequest::SetClosedCaptionsFontEdgeColor(color))
        .await
    {
        if let Some(ExtnResponse::None(())) = extn_menssage.payload.as_response() {
            migration_state = MigrationState::Succeeded;
        }
    }

    migration_state
}

async fn migrate_font_opacity(platform_state: &PlatformState) -> MigrationState {
    let mut migration_state = MigrationState::Failed;

    let opacity =
        StorageManager::get_string(platform_state, StorageProperty::ClosedCaptionsFontOpacity)
            .await
            .unwrap_or_default();
    if let Ok(extn_menssage) = platform_state
        .get_client()
        .send_extn_request(UserSettingsRequest::SetClosedCaptionsFontOpacity(opacity))
        .await
    {
        if let Some(ExtnResponse::None(())) = extn_menssage.payload.as_response() {
            migration_state = MigrationState::Succeeded;
        }
    }

    migration_state
}

async fn migrate_background_color(platform_state: &PlatformState) -> MigrationState {
    let mut migration_state = MigrationState::Failed;

    let color = StorageManager::get_string(
        platform_state,
        StorageProperty::ClosedCaptionsBackgroundColor,
    )
    .await
    .unwrap_or_default();
    if let Ok(extn_menssage) = platform_state
        .get_client()
        .send_extn_request(UserSettingsRequest::SetClosedCaptionsBackgroundColor(color))
        .await
    {
        if let Some(ExtnResponse::None(())) = extn_menssage.payload.as_response() {
            migration_state = MigrationState::Succeeded;
        }
    }

    migration_state
}

async fn migrate_background_opacity(platform_state: &PlatformState) -> MigrationState {
    let mut migration_state = MigrationState::Failed;

    let opacity = StorageManager::get_string(
        platform_state,
        StorageProperty::ClosedCaptionsBackgroundOpacity,
    )
    .await
    .unwrap_or_default();
    if let Ok(extn_menssage) = platform_state
        .get_client()
        .send_extn_request(UserSettingsRequest::SetClosedCaptionsBackgroundOpacity(
            opacity,
        ))
        .await
    {
        if let Some(ExtnResponse::None(())) = extn_menssage.payload.as_response() {
            migration_state = MigrationState::Succeeded;
        }
    }

    migration_state
}

async fn migrate_window_color(platform_state: &PlatformState) -> MigrationState {
    let mut migration_state = MigrationState::Failed;

    let color =
        StorageManager::get_string(platform_state, StorageProperty::ClosedCaptionsWindowColor)
            .await
            .unwrap_or_default();
    if let Ok(extn_menssage) = platform_state
        .get_client()
        .send_extn_request(UserSettingsRequest::SetClosedCaptionsWindowColor(color))
        .await
    {
        if let Some(ExtnResponse::None(())) = extn_menssage.payload.as_response() {
            migration_state = MigrationState::Succeeded;
        }
    }

    migration_state
}

async fn migrate_window_opacity(platform_state: &PlatformState) -> MigrationState {
    let mut migration_state = MigrationState::Failed;

    let opacity =
        StorageManager::get_string(platform_state, StorageProperty::ClosedCaptionsWindowOpacity)
            .await
            .unwrap_or_default();
    if let Ok(extn_menssage) = platform_state
        .get_client()
        .send_extn_request(UserSettingsRequest::SetClosedCaptionsWindowOpacity(opacity))
        .await
    {
        if let Some(ExtnResponse::None(())) = extn_menssage.payload.as_response() {
            migration_state = MigrationState::Succeeded;
        }
    }

    migration_state
}
