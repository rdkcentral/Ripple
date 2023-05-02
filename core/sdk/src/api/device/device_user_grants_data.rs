use crate::api::rpc::rpc_gateway::CallContext;
use crate::apps::provider_broker::{
    self, ProviderBroker, ProviderRequestPayload, ProviderResponsePayload,
};
use crate::helpers::ripple_helper::IRippleHelper;
use crate::managers::capability_manager::{DenyReason, FireboltCap};
use crate::managers::capability_resolver::CapabilityResolver;
use crate::platform_state::PlatformState;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::hash::{Hash, Hasher};
use tokio::sync::oneshot;
use tracing::debug;

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

#[derive(Deserialize, Debug, Clone)]
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

impl GrantStep {
    pub async fn execute(
        &self,
        platform_state: &PlatformState,
        call_ctx: &CallContext,
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
            return Err(DenyReason::Unsupported);
        }
        debug!("Cap is supported. Now have to check if it is available");
        if !platform_state
            .services
            .is_cap_available(firebolt_cap.clone())
            .await
        {
            debug!("cap is supported but not available");
            return Err(DenyReason::Unavailable);
        }
        self.invoke_capability(platform_state, call_ctx, &firebolt_cap, configuration)
            .await
    }
    pub async fn invoke_capability(
        &self,
        platform_state: &PlatformState,
        call_ctx: &CallContext,
        cap: &FireboltCap,
        param: &Option<Value>,
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
        let pr_msg_opt = match p_cap.as_str().as_str() {
            "xrn:firebolt:capability:usergrant:acknowledgechallenge" => {
                serde_json::from_value(param.as_ref().unwrap().clone()).map_or(None, |challenge| {
                    Some(provider_broker::Request {
                        capability: p_cap.as_str(),
                        method: String::from("challenge"),
                        caller: call_ctx.clone(),
                        request: ProviderRequestPayload::AckChallenge(challenge),
                        tx: session_tx,
                        app_id: None,
                    })
                })
            }
            "xrn:firebolt:capability:usergrant:pinchallenge" => {
                serde_json::from_value(param.as_ref().unwrap().clone()).map_or(None, |challenge| {
                    Some(provider_broker::Request {
                        capability: p_cap.as_str(),
                        method: String::from("challenge"),
                        caller: call_ctx.clone(),
                        request: ProviderRequestPayload::PinChallenge(challenge),
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
                        return Err(DenyReason::Ungranted);
                    }
                },
                Err(_) => {
                    debug!("Receive error in channel");
                    return Err(DenyReason::Ungranted);
                }
            }
        } else {
            /*
             * We would reach here if the cap is ack or pin
             * and we are not able to parse the configuration in the manifest
             * as pinchallenge or ackchallenge.
             */
            return Err(DenyReason::Unsupported);
        }
    }
}

impl GrantRequirements {
    /*
     * We can execute a grant requirement only if the caps defined in its
     * steps are supported and available. This functions checks if all the
     * steps in the requirements are supported and available.
     */
    async fn has_steps_for_execution(
        &self,
        platform_state: &PlatformState,
        call_ctx: &CallContext,
    ) -> bool {
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
    ) -> Result<(), DenyReason> {
        for step in &self.steps {
            step.execute(platform_state, call_ctx).await?
        }
        Ok(())
    }
}

impl GrantPolicy {
    async fn evaluate_options(
        &self,
        platform_state: &PlatformState,
        call_ctx: &CallContext,
    ) -> Result<(), DenyReason> {
        let mut result: Result<(), DenyReason> = Err(DenyReason::GrantDenied);
        for grant_requirements in &self.options {
            if grant_requirements
                .has_steps_for_execution(platform_state, call_ctx)
                .await
            {
                result = grant_requirements.execute(platform_state, call_ctx).await;
                break;
            }
        }
        result
    }

    pub async fn execute(
        &self,
        platform_state: &PlatformState,
        call_ctx: &CallContext,
    ) -> Result<(), DenyReason> {
        let mut result: Result<(), DenyReason>;
        if let Some(_privacy_settings) = &self.privacy_setting {
            result = Ok(());
        } else {
            result = self.evaluate_options(platform_state, call_ctx).await;
        }
        result
    }

    pub async fn override_policy(&mut self, plaform_state: &PlatformState, call_ctx: &CallContext) {
    }

    pub async fn is_valid(&self, platform_state: &PlatformState, call_ctx: &CallContext) -> bool {
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
                        .find(|x| x.name.starts_with("property:") && x.allow_value.is_some())
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
                            && CapabilityResolver::get(None)
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
