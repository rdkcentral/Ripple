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

use ripple_sdk::{
    api::{
        firebolt::fb_capabilities::{
            CapEvent, DenyReason, DenyReasonWithCap, FireboltCap, FireboltPermission,
        },
        gateway::rpc_gateway_api::CallContext,
        manifest::device_manifest::{GrantPolicy, GrantPrivacySetting},
    },
    log::debug,
};

use crate::{
    service::user_grants::grant_step_enforcer::GrantStepExecutor,
    state::{cap::cap_state::CapState, platform_state::PlatformState},
};

pub struct GrantPolicyEnforcer;

impl GrantPolicyEnforcer {
    pub async fn determine_grant_policies_for_permission(
        platform_state: &PlatformState,
        call_context: &CallContext,
        permission: &FireboltPermission,
    ) -> Result<(), DenyReasonWithCap> {
        if let Some(grant_policy_map) = platform_state.get_device_manifest().get_grant_policies() {
            let result = grant_policy_map.get(&permission.cap.as_str());
            if let Some(policies) = result {
                if let Some(policy) = policies.get_policy(permission) {
                    if Self::is_policy_valid(platform_state, &policy) {
                        return Err(DenyReasonWithCap {
                            caps: vec![permission.clone().cap],
                            reason: DenyReason::Disabled,
                        });
                    }
                    let result = GrantPolicyEnforcer::execute(
                        platform_state,
                        call_context,
                        permission,
                        &policy,
                    )
                    .await;
                    platform_state.cap_state.grant_state.update(
                        permission,
                        &policy,
                        result.is_ok(),
                        &call_context.app_id,
                    )
                }
            }
        }
        Ok(())
    }

    fn is_policy_valid(platform_state: &PlatformState, policy: &GrantPolicy) -> bool {
        if let Some(privacy) = &policy.privacy_setting {
            let privacy_property = &privacy.property;
            return platform_state
                .open_rpc_state
                .check_privacy_property(privacy_property);
        } else {
            if let Some(grant_steps) = policy.get_steps_without_grant() {
                for step in grant_steps {
                    if platform_state
                        .open_rpc_state
                        .get_capability_policy(step.capability.clone())
                        .is_some()
                    {
                        return false;
                    }
                }
                return true;
            }
        }

        false
    }

    async fn evaluate_privacy_settings(
        _platform_state: &PlatformState,
        _privacy_setting: &GrantPrivacySetting,
        _call_ctx: &CallContext,
    ) -> Option<Result<(), DenyReasonWithCap>> {
        // TODO add Privacy logic
        None
    }

    async fn evaluate_options(
        platform_state: &PlatformState,
        call_ctx: &CallContext,
        permission: &FireboltPermission,
        policy: &GrantPolicy,
    ) -> Result<(), DenyReasonWithCap> {
        let generic_cap_state = platform_state.clone().cap_state.generic;
        for grant_requirements in &policy.options {
            for step in &grant_requirements.steps {
                let cap = FireboltCap::Full(step.capability.to_owned());
                let firebolt_cap = vec![cap.clone()];
                debug!(
                    "checking if the cap is supported & available: {:?}",
                    firebolt_cap
                );
                if let Err(e) = generic_cap_state.check_all(&firebolt_cap) {
                    return Err(DenyReasonWithCap {
                        caps: e.caps,
                        reason: DenyReason::GrantDenied,
                    });
                } else {
                    match GrantStepExecutor::execute(step, platform_state, call_ctx, permission)
                        .await
                    {
                        Ok(_) => {
                            CapState::emit(
                                platform_state,
                                CapEvent::OnGranted,
                                cap,
                                Some(permission.role.clone()),
                            )
                            .await;
                            return Ok(());
                        }
                        Err(e) => {
                            CapState::emit(
                                platform_state,
                                CapEvent::OnRevoked,
                                cap,
                                Some(permission.role.clone()),
                            )
                            .await;
                            return Err(e);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn execute(
        platform_state: &PlatformState,
        call_ctx: &CallContext,
        permission: &FireboltPermission,
        policy: &GrantPolicy,
    ) -> Result<(), DenyReasonWithCap> {
        if let Some(privacy_setting) = policy.clone().privacy_setting {
            let resp =
                Self::evaluate_privacy_settings(platform_state, &privacy_setting, call_ctx).await;

            if resp.is_some() {
                return resp.unwrap();
            }
        }
        Self::evaluate_options(platform_state, call_ctx, permission, policy).await
    }
}
