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
use crate::{
    service::apps::provider_broker::{ProviderBroker, ProviderBrokerRequest},
    state::platform_state::PlatformState,
};
use ripple_sdk::{
    api::{
        apps::{AppManagerResponse, AppMethod, AppRequest, AppResponse},
        firebolt::{
            fb_capabilities::{DenyReason, FireboltCap, FireboltPermission},
            fb_pin::{PinChallengeConfiguration, PinChallengeRequest},
            provider::{
                Challenge, ChallengeRequestor, ProviderRequestPayload, ProviderResponsePayload,
            },
        },
        gateway::rpc_gateway_api::CallContext,
    },
    log::debug,
    serde_json::Value,
    tokio::sync::oneshot,
};
use ripple_sdk::{
    api::{
        device::device_user_grants_data::GrantStep, firebolt::fb_capabilities::DenyReasonWithCap,
    },
    serde_json,
};
use serde::Deserialize;
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GrantStepExecutor;

impl GrantStepExecutor {
    pub async fn execute(
        step: &GrantStep,
        platform_state: &PlatformState,
        call_ctx: &CallContext,
        permission: &FireboltPermission,
    ) -> Result<(), DenyReasonWithCap> {
        let capability = step.capability.clone();
        let configuration = step.configuration.clone();
        debug!(
            "Reached execute phase of step for capability: {}",
            capability
        );
        // 1. Check if the capability is supported and available.
        // 2. Call the capability,
        // 3. Get the user response and return
        let firebolt_cap = FireboltCap::Full(capability.to_owned());
        if let Err(e) = platform_state
            .cap_state
            .generic
            .check_all(&vec![firebolt_cap.clone()])
        {
            return Err(DenyReasonWithCap {
                reason: DenyReason::GrantDenied,
                caps: e.caps,
            });
        }

        Self::invoke_capability(
            platform_state,
            call_ctx,
            &firebolt_cap,
            &configuration,
            permission,
        )
        .await
    }

    async fn get_app_name(platform_state: &PlatformState, app_id: String) -> String {
        let mut app_name: String = Default::default();
        let (tx, rx) = oneshot::channel::<AppResponse>();
        let app_request = AppRequest::new(AppMethod::GetAppName(app_id), tx);
        let send_result = platform_state.get_client().send_app_request(app_request);
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

    pub async fn invoke_capability(
        platform_state: &PlatformState,
        call_ctx: &CallContext,
        cap: &FireboltCap,
        param: &Option<Value>,
        permission: &FireboltPermission,
    ) -> Result<(), DenyReasonWithCap> {
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
        let app_name = Self::get_app_name(platform_state, call_ctx.app_id.clone()).await;
        let pr_msg_opt = match p_cap.as_str().as_str() {
            "xrn:firebolt:capability:usergrant:acknowledgechallenge" => {
                let challenge = Challenge {
                    capability: permission.cap.as_str(),
                    requestor: ChallengeRequestor {
                        id: call_ctx.app_id.clone(),
                        name: app_name,
                    },
                };
                Some(ProviderBrokerRequest {
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
                    Some(ProviderBrokerRequest {
                        capability: p_cap.as_str(),
                        method: "challenge".to_owned(),
                        caller: call_ctx.clone(),
                        request: ProviderRequestPayload::PinChallenge(PinChallengeRequest {
                            pin_space: pin_conf.pin_space,
                            requestor: call_ctx.clone(),
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
                Some(ProviderBrokerRequest {
                    capability: p_cap.as_str(),
                    method: String::from("challenge"),
                    caller: call_ctx.clone(),
                    request: ProviderRequestPayload::Generic(param_str),
                    tx: session_tx,
                    app_id: None,
                })
            }
        };
        let result = if let Some(pr_msg) = pr_msg_opt {
            ProviderBroker::invoke_method(&platform_state.clone(), pr_msg).await;
            match session_rx.await {
                Ok(result) => match result.as_challenge_response() {
                    Some(res) => match res.granted {
                        true => {
                            debug!("returning ok from invoke_capability");
                            Ok(())
                        }
                        false => {
                            debug!("returning err from invoke_capability");
                            Err(DenyReason::GrantDenied)
                        }
                    },
                    None => {
                        debug!("Received reponse that is not convertable to challenge response");
                        Err(DenyReason::Ungranted)
                    }
                },
                Err(_) => {
                    debug!("Receive error in channel");
                    Err(DenyReason::Ungranted)
                }
            }
        } else {
            /*
             * We would reach here if the cap is ack or pin
             * and we are not able to parse the configuration in the manifest
             * as pinchallenge or ackchallenge.
             */
            Err(DenyReason::Ungranted)
        };

        if let Err(reason) = result {
            Err(DenyReasonWithCap {
                reason,
                caps: vec![cap.clone()],
            })
        } else {
            Ok(())
        }
    }
}
