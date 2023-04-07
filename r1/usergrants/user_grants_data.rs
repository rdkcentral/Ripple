use crate::api::handlers::pin_challenge::{PinChallengeRequest, PinSpace};
use crate::api::handlers::privacy::{PrivacyImpl, PrivacyServer};
use crate::api::rpc::rpc_gateway::CallContext;
use crate::apps::app_mgr::{AppError, AppManagerResponse, AppMethod, AppRequest};
use crate::apps::provider_broker::{
    self, ProviderBroker, ProviderRequestPayload, ProviderResponsePayload,
};
use crate::helpers::ripple_helper::IRippleHelper;
use crate::managers::capability_manager::{DenyReason, FireboltCap, FireboltPermission};
use crate::managers::capability_resolver::CapabilityResolver;
use crate::platform_state::PlatformState;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use tokio::sync::oneshot;
use tracing::debug;

use super::user_grants::{Challenge, ChallengeRequestor};

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PinChallengeConfiguration {
    pub pin_space: PinSpace,
}
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GrantStep {
    pub capability: String,
    pub configuration: Option<Value>,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GrantRequirements {
    pub steps: Vec<GrantStep>,
}

#[derive(Eq, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Lifespan {
    Once,
    Forever,
    AppActive,
    PowerActive,
    Seconds,
}

impl Lifespan {
    pub fn as_string(&self) -> &'static str {
        match self {
            Lifespan::Once => "once",
            Lifespan::Forever => "forever",
            Lifespan::AppActive => "appActive",
            Lifespan::PowerActive => "powerActive",
            Lifespan::Seconds => "seconds",
        }
    }
}

impl Hash for Lifespan {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u8(match self {
            Lifespan::Once => 0,
            Lifespan::Forever => 1,
            Lifespan::AppActive => 2,
            Lifespan::PowerActive => 3,
            Lifespan::Seconds => 4,
        });
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum AutoApplyPolicy {
    Always,
    Allowed,
    Disallowed,
    Never,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PrivacySetting {
    pub property: String,
    pub auto_apply_policy: AutoApplyPolicy,
    pub update_property: bool,
}

#[derive(Eq, PartialEq, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Scope {
    App,
    Device,
}

impl Hash for Scope {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u8(match self {
            Scope::App => 0,
            Scope::Device => 1,
        });
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GrantPolicy {
    pub options: Vec<GrantRequirements>,
    pub scope: Scope,
    pub lifespan: Lifespan,
    pub overridable: bool,
    pub lifespan_ttl: Option<u64>,
    pub privacy_setting: Option<PrivacySetting>,
}

impl Default for GrantPolicy {
    fn default() -> Self {
        GrantPolicy {
            options: Default::default(),
            scope: Scope::Device,
            lifespan: Lifespan::Once,
            overridable: true,
            lifespan_ttl: None,
            privacy_setting: None,
        }
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct GrantPolicies {
    #[serde(rename = "use")]
    pub _use: Option<GrantPolicy>,
    pub manage: Option<GrantPolicy>,
    pub provide: Option<GrantPolicy>,
}

pub async fn get_app_name(platform_state: &PlatformState, app_id: String) -> String {
    let mut app_name: String = Default::default();
    let (tx, rx) = oneshot::channel::<Result<AppManagerResponse, AppError>>();
    let app_request = AppRequest {
        method: AppMethod::GetAppName(app_id),
        resp_tx: Some(tx),
    };
    let send_result = platform_state.services.send_app_request(app_request).await;
    if send_result.is_err() {
        return app_name;
    }
    if let Ok(app_response_res) = rx.await {
        if let Ok(app_response) = app_response_res {
            if let AppManagerResponse::AppName(name) = app_response {
                app_name = name.unwrap_or_default();
            }
        }
    }
    app_name
}
impl GrantStep {
    pub async fn execute(
        &self,
        platform_state: &PlatformState,
        call_ctx: &CallContext,
        permission: &FireboltPermission,
    ) -> Result<(), DenyReason> {
        let capability = &self.capability;
        let configuration = &self.configuration;
        debug!(
            "Reached execute phase of step for capability: {}",
            capability
        );
        // 1. Check if the capability is supported and available.
        // 2. Call the capability,
        // 3. Get the user response and return
        let firebolt_cap = FireboltCap::Full(capability.to_owned());
        if !platform_state
            .services
            .is_cap_supported(firebolt_cap.clone())
            .await
        {
            debug!("Cap is neither supported nor available");
            return Err(DenyReason::Unpermitted);
        }
        debug!("Cap is supported. Now have to check if it is available");
        if !platform_state
            .services
            .is_cap_available(firebolt_cap.clone())
            .await
        {
            debug!("cap is supported but not available");
            return Err(DenyReason::Unpermitted);
        }
        self.invoke_capability(
            platform_state,
            call_ctx,
            &firebolt_cap,
            configuration,
            permission,
        )
        .await
    }

    pub async fn invoke_capability(
        &self,
        platform_state: &PlatformState,
        call_ctx: &CallContext,
        cap: &FireboltCap,
        param: &Option<Value>,
        permission: &FireboltPermission,
    ) -> Result<(), DenyReason> {
        let (session_tx, session_rx) = oneshot::channel::<ProviderResponsePayload>();
        let p_cap = cap.clone();
        /*
         * We have a concrete struct defined for ack challenge and pin challenge hence handling them separately. If any new
         * caps are introduced in future, the assumption is that capability provider has a method "challenge" and it can
         * deduce its params from a string.
         */

        /*
         * this might be weird looking as_str().as_str(), FireboltCap returns String but has a function named as_str.
         * We call as_str on String to convert String to str to perform our match
         */
        let app_name = get_app_name(platform_state, call_ctx.app_id.clone()).await;
        let pr_msg_opt = match p_cap.as_str().as_str() {
            "xrn:firebolt:capability:usergrant:acknowledgechallenge" => {
                let challenge = Challenge {
                    capability: permission.cap.as_str(),
                    requestor: ChallengeRequestor {
                        id: call_ctx.app_id.clone(),
                        name: app_name,
                    },
                };
                Some(provider_broker::Request {
                    capability: p_cap.as_str(),
                    method: String::from("challenge"),
                    caller: call_ctx.clone(),
                    request: ProviderRequestPayload::AckChallenge(challenge),
                    tx: session_tx,
                    app_id: None,
                })
            }
            "xrn:firebolt:capability:usergrant:pinchallenge" => {
                let pin_space_res = serde_json::from_value::<PinChallengeConfiguration>(
                    param.as_ref().unwrap_or(&Value::Null).clone(),
                );
                pin_space_res.map_or(None, |pin_conf| {
                    Some(provider_broker::Request {
                        capability: p_cap.as_str(),
                        method: "challenge".to_owned(),
                        caller: call_ctx.clone(),
                        request: ProviderRequestPayload::PinChallenge(PinChallengeRequest {
                            pin_space: pin_conf.pin_space,
                            requestor: ChallengeRequestor {
                                id: call_ctx.app_id.clone(),
                                name: app_name,
                            },
                            capability: Some(call_ctx.app_id.clone()),
                        }),
                        tx: session_tx,
                        app_id: None,
                    })
                })
            }
            _ => {
                /*
                 * This is for any other capability, hoping it to deduce its necessary params from a json string
                 * and has a challenge method.
                 */
                let param_str = match param {
                    None => "".to_owned(),
                    Some(val) => val.to_string(),
                };
                Some(provider_broker::Request {
                    capability: p_cap.as_str(),
                    method: String::from("challenge"),
                    caller: call_ctx.clone(),
                    request: ProviderRequestPayload::Generic(param_str),
                    tx: session_tx,
                    app_id: None,
                })
            }
        };
        if let Some(pr_msg) = pr_msg_opt {
            ProviderBroker::invoke_method(&platform_state.clone(), pr_msg).await;
            match session_rx.await {
                Ok(result) => match result.as_challenge_response() {
                    Some(res) => match res.granted {
                        true => {
                            debug!("returning ok from invoke_capability");
                            return Ok(());
                        }
                        false => {
                            debug!("returning err from invoke_capability");
                            return Err(DenyReason::GrantDenied);
                        }
                    },
                    None => {
                        debug!("Received reponse that is not convertable to challenge response");
                        return Err(DenyReason::Unpermitted);
                    }
                },
                Err(_) => {
                    debug!("Receive error in channel");
                    return Err(DenyReason::Unpermitted);
                }
            }
        } else {
            /*
             * We would reach here if the cap is ack or pin
             * and we are not able to parse the configuration in the manifest
             * as pinchallenge or ackchallenge.
             */
            return Err(DenyReason::Ungranted);
        }
    }
}

impl GrantRequirements {
    /*
     * We can execute a grant requirement only if the caps defined in its
     * steps are supported and available. This functions checks if all the
     * steps in the requirements are supported and available.
     */
    async fn has_steps_for_execution(&self, platform_state: &PlatformState) -> bool {
        let mut supported_and_available = true;
        for step in &self.steps {
            let firebolt_cap = FireboltCap::Full(step.capability.to_owned());
            debug!(
                "checking if the cap is supported & available: {:?}",
                firebolt_cap.as_str()
            );
            if platform_state
                .services
                .is_cap_supported(firebolt_cap.clone())
                .await
                && platform_state
                    .services
                    .is_cap_available(firebolt_cap.clone())
                    .await
            {
                supported_and_available = true;
            } else {
                supported_and_available = false;
                break;
            }
        }
        supported_and_available
    }

    async fn execute(
        &self,
        platform_state: &PlatformState,
        call_ctx: &CallContext,
        permission: &FireboltPermission,
    ) -> Result<(), DenyReason> {
        for step in &self.steps {
            step.execute(platform_state, call_ctx, permission).await?
        }
        Ok(())
    }
}

impl GrantPolicy {
    async fn evaluate_options(
        &self,
        platform_state: &PlatformState,
        call_ctx: &CallContext,
        permission: &FireboltPermission,
    ) -> Result<(), DenyReason> {
        let mut result: Result<(), DenyReason> = Err(DenyReason::GrantDenied);
        for grant_requirements in &self.options {
            if grant_requirements
                .has_steps_for_execution(platform_state)
                .await
            {
                result = grant_requirements
                    .execute(platform_state, call_ctx, permission)
                    .await;
                break;
            }
        }
        result
    }

    pub async fn evaluate_privacy_settings(
        platform_state: &PlatformState,
        privacy_setting: &PrivacySetting,
        call_ctx: &CallContext,
    ) -> Option<Result<(), DenyReason>> {
        let privacy_impl = PrivacyImpl {
            helper: Box::new(platform_state.services.clone()),
            platform_state: platform_state.clone(),
        };
        // Find the rpc method which has same name as that mentioned in privacy settings, and has allow property.
        let firebolt_rpc_method_opt = platform_state
            .firebolt_open_rpc
            .get_method_with_allow_value_property(privacy_setting.property.to_owned());
        if firebolt_rpc_method_opt.is_none() {
            return None;
        }
        let method = firebolt_rpc_method_opt.unwrap();
        let allow_value_opt = method.get_allow_value();
        if allow_value_opt.is_none() {
            return None;
        }
        let allow_value = allow_value_opt.unwrap();
        // From privacyImpl make the call to the registered method for the configured privacy settings.
        let privacy_rpc = privacy_impl.into_rpc();
        let res_stored_value = privacy_rpc
            .call::<_, bool>(method.name.as_str(), [call_ctx.clone()])
            .await;
        if res_stored_value.is_err() {
            return None;
        }

        let stored_value = res_stored_value.unwrap();
        match privacy_setting.auto_apply_policy {
            AutoApplyPolicy::Always => {
                if stored_value == allow_value {
                    // Silently Grant if the stored value is same as that of allowed value
                    Some(Ok(()))
                } else {
                    // Silently Deny if the stored value is inverse of allowed value
                    Some(Err(DenyReason::GrantDenied))
                }
            }
            AutoApplyPolicy::Allowed => {
                // Silently Grant if the stored value is same as that of allowed value
                if stored_value == allow_value {
                    Some(Ok(()))
                } else {
                    None
                }
            }
            AutoApplyPolicy::Disallowed => {
                // Silently Deny if the stored value is inverse of allowed value
                if stored_value != allow_value {
                    Some(Err(DenyReason::GrantDenied))
                } else {
                    None
                }
            }
            AutoApplyPolicy::Never => {
                // This is already handled during start of the function.
                // Do nothing using privacy settings get it by evaluating options
                None
            }
        }
    }

    pub async fn execute(
        &self,
        platform_state: &PlatformState,
        call_ctx: &CallContext,
        permission: &FireboltPermission,
    ) -> Result<(), DenyReason> {
        if self.privacy_setting.is_some()
            && self.privacy_setting.as_ref().unwrap().auto_apply_policy != AutoApplyPolicy::Never
        {
            if let Some(priv_sett_response) = Self::evaluate_privacy_settings(
                platform_state,
                &self.privacy_setting.as_ref().unwrap(),
                &call_ctx,
            )
            .await
            {
                return priv_sett_response;
            }
        }
        self.evaluate_options(platform_state, call_ctx, permission)
            .await
    }

    pub async fn override_policy(
        &mut self,
        _plaform_state: &PlatformState,
        _call_ctx: &CallContext,
    ) {
    }

    pub async fn is_valid(&self, platform_state: &PlatformState) -> bool {
        let mut result = false;
        if let Some(privacy_settings) = &self.privacy_setting {
            // Checking if the property is present in firebolt-open-rpc spec
            if let Some(method) = &platform_state
                .firebolt_open_rpc
                .methods
                .iter()
                .find(|x| x.name == privacy_settings.property)
            {
                // Checking if the property tag is havin x-allow-value extension.
                if let Some(tags) = &method.tags {
                    result = tags
                        .iter()
                        .find(|x| x.name == "property" && x.allow_value.is_some())
                        .map_or(false, |_| true);
                }
            }
        } else {
            // Checking if we have at least one Grant Requirements.
            result = true;
            if self.options.len() > 0 {
                // Check if all the Granting capabilities are of the format "xrn:firebolt:capability:usergrant:* and have entry in firebolt-spec"
                'requirements: for grant_requirements in &self.options {
                    for step in &grant_requirements.steps {
                        if !step
                            .capability
                            .starts_with("xrn:firebolt:capability:usergrant:")
                            && CapabilityResolver::get(None, HashMap::default())
                                .cap_policies
                                .contains_key(&step.capability)
                        {
                            result = false;
                            break 'requirements;
                        }
                    }
                }
            } else {
                result = false;
            }
        }
        result
    }
}
