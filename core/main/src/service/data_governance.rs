use ripple_sdk::{
    api::{
        distributor::distributor_privacy::{
            DataEventType, ExclusionPolicy, ExclusionPolicyData, PrivacyRequest,
        },
        manifest::device_manifest::DataGovernancePolicy,
        storage_property::StorageProperty,
    },
    log::{debug, info},
    utils::error::RippleError,
};
use std::collections::HashSet;
use std::sync::{Arc, RwLock};

use crate::{
    processor::storage::storage_manager::StorageManager, state::platform_state::PlatformState,
};

pub fn default_enforcement_value() -> bool {
    false
}

pub fn default_drop_on_all_tags() -> bool {
    true
}

pub struct DataGovernance {}

#[derive(Clone)]
pub struct DataGovernanceState {
    pub exclusions: Arc<RwLock<Option<ExclusionPolicy>>>,
}

impl Default for DataGovernanceState {
    fn default() -> Self {
        let state = DataGovernanceState {
            exclusions: Arc::new(RwLock::new(None)),
        };
        state
    }
}

impl std::fmt::Debug for DataGovernanceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataGovernanceState").finish()
    }
}

impl DataGovernance {
    fn update_local_exclusion_policy(state: &DataGovernanceState, excl: ExclusionPolicy) {
        let mut dg = state.exclusions.write().unwrap();
        *dg = Some(excl)
    }

    fn get_local_exclusion_policy(state: &DataGovernanceState) -> Option<ExclusionPolicy> {
        let dg = state.exclusions.read().unwrap();
        (*dg).clone()
    }

    pub async fn get_tags(
        state: &PlatformState,
        app_id: String,
        data_type: DataEventType,
        policy: &DataGovernancePolicy,
    ) -> (HashSet<String>, bool) {
        let mut tags = HashSet::default();
        let mut all_settings_enforced = true;
        let exclusions = DataGovernance::get_partner_exclusions(state)
            .await
            .unwrap_or_default();
        for tag in &policy.setting_tags {
            let mut excluded = None;
            let data = DataGovernance::get_exclusion_data(tag.setting.clone(), exclusions.clone());
            if data.is_some() {
                excluded = Some(DataGovernance::is_app_excluded(
                    &app_id,
                    &data_type,
                    &data.unwrap(),
                ));
                debug!(
                    "get_tags: app_id={:?} setting={:?} is_excluded={:?}",
                    app_id.clone(),
                    tag,
                    excluded
                );
            }

            // do not get user setting if excluded
            if excluded.is_some() && excluded.unwrap() {
                let tags_to_add = tag.tags.clone();
                tags.extend(tags_to_add);
            } else {
                let val = StorageManager::get_bool(state, tag.setting.clone())
                    .await
                    .unwrap_or(false);
                if val == tag.enforcement_value {
                    let tags_to_add = tag.tags.clone();
                    tags.extend(tags_to_add);
                } else {
                    all_settings_enforced = false;
                }
            }
        }
        (tags, all_settings_enforced)
    }

    pub async fn resolve_tags(
        platform_state: &PlatformState,
        app_id: String,
        data_type: DataEventType,
    ) -> (HashSet<String>, bool) {
        let data_gov_cfg = platform_state
            .get_device_manifest()
            .configuration
            .data_governance
            .clone();
        let data_tags = match data_gov_cfg.get_policy(data_type.clone()) {
            Some(policy) => {
                let (t, all) =
                    DataGovernance::get_tags(platform_state, app_id, data_type, &policy).await;
                if policy.drop_on_all_tags && all {
                    return (t, true);
                }
                t
            }
            None => {
                info!("data_governance.policies not found");
                HashSet::default()
            }
        };
        return (data_tags, false);
    }

    pub async fn refresh_partner_exclusions(state: &PlatformState) -> bool {
        if let Some(session) = state.session_state.get_account_session() {
            if let Ok(response) = state
                .get_client()
                .send_extn_request(PrivacyRequest::GetPartnerExclusions(session))
                .await
            {
                if let Some(excl) = response.payload.clone().extract::<ExclusionPolicy>() {
                    DataGovernance::update_local_exclusion_policy(
                        &state.data_governance,
                        excl.clone(),
                    );
                    let result = serde_json::to_string(&excl);
                    // result.unwrap_or("");    // XXX: when server return 404 or empty string
                    if result.is_ok() {
                        let str_excl = result.unwrap();
                        return StorageManager::set_string(
                            state,
                            StorageProperty::PartnerExclusions,
                            str_excl,
                            None,
                        )
                        .await
                        .is_ok();
                    }
                }
            }
        }
        false
    }

    pub async fn get_partner_exclusions(
        state: &PlatformState,
    ) -> Result<ExclusionPolicy, RippleError> {
        let mut result = Err(RippleError::InvalidOutput);
        match DataGovernance::get_local_exclusion_policy(&state.data_governance) {
            Some(excl) => return Ok(excl),
            _ => {}
        }

        let resp = StorageManager::get_string(state, StorageProperty::PartnerExclusions).await;
        debug!("StorageProperty::PartnerExclusions resp={:?}", resp);
        if resp.is_ok() {
            let str_excl = resp.unwrap();
            if !str_excl.is_empty() {
                let excl = serde_json::from_str(&str_excl);
                if excl.is_ok() {
                    let exclusion_policy: ExclusionPolicy = excl.unwrap();
                    DataGovernance::update_local_exclusion_policy(
                        &state.data_governance,
                        exclusion_policy.clone(),
                    );
                    result = Ok(exclusion_policy)
                }
            }
        }
        result
    }

    pub fn get_exclusion_data(
        setting: StorageProperty,
        exclusions: ExclusionPolicy,
    ) -> Option<ExclusionPolicyData> {
        let response = match setting {
            StorageProperty::AllowPersonalization => exclusions.personalization.clone(),
            StorageProperty::AllowProductAnalytics => exclusions.product_analytics.clone(),
            _ => None,
        };
        response
    }

    pub fn is_app_excluded(
        app_id: &String,
        data_type: &DataEventType,
        excl: &ExclusionPolicyData,
    ) -> bool {
        let mut app_found: bool = false;
        let mut event_found: bool = false;

        for evt in &excl.data_events {
            if *evt == *data_type {
                event_found = true;
                break;
            }
        }
        if event_found {
            for app in &excl.entity_reference {
                if app.as_str() == app_id {
                    app_found = true;
                    break;
                }
            }
        }
        app_found
    }
}
