use serde::Deserialize;
use ripple_sdk::{serde_json::Value, api::gateway::{rpc_gateway_api::CallContext, rpc_error::DenyReason}, log::debug};

use crate::state::platform_state::PlatformState;
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GrantStep {
    pub capability: String,
    pub configuration: Option<Value>,
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
        let firebolt_cap = FireboltCa::Full(capability.to_owned());
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
            return Err(DenyReason::Ungranted);
        }
    }
}