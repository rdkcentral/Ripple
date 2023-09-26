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

use std::collections::HashMap;

use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    RpcModule,
};
use ripple_sdk::{
    api::{
        firebolt::{
            fb_pin::{PinChallengeRequestWithContext, PinSpace, PIN_CHALLENGE_CAPABILITY},
            provider::ChallengeRequestor,
        },
        gateway::rpc_gateway_api::CallContext,
    },
    extn::extn_client_message::ExtnResponse,
};

use crate::{firebolt::rpc::RippleRPCProvider, state::platform_state::PlatformState};

#[rpc(server)]
pub trait Profile {
    #[method(name = "profile.approvePurchase")]
    async fn approve_purchase(&self, ctx: CallContext) -> RpcResult<bool>;

    #[method(name = "profile.approveContentRating")]
    async fn approve_content_rating(&self, ctx: CallContext) -> RpcResult<bool>;
    /*
    https://ccp.sys.comcast.net/browse/RPPL-161
    this is a little awkward here , but least bad home for it:
    */
    #[method(name = "profile.flags")]
    async fn profile_flags(&self, ctx: CallContext) -> RpcResult<HashMap<String, String>>;
}

pub struct ProfileImpl {
    pub platform_state: PlatformState,
}

#[async_trait]
impl ProfileServer for ProfileImpl {
    async fn approve_content_rating(&self, ctx: CallContext) -> RpcResult<bool> {
        let pin_request = PinChallengeRequestWithContext {
            pin_space: PinSpace::Content,
            requestor: ChallengeRequestor {
                id: ctx.app_id.to_owned(),
                name: ctx.app_id.to_owned(),
            },
            capability: Some(String::from(PIN_CHALLENGE_CAPABILITY)),
            call_ctx: ctx.clone(),
        };

        if let Ok(response) = self
            .platform_state
            .get_client()
            .send_extn_request(pin_request)
            .await
        {
            if let Some(ExtnResponse::PinChallenge(v)) = response.payload.extract() {
                return match v.granted {
                    Some(grant) => Ok(grant),
                    None => Ok(false),
                };
            }
        }
        Err(jsonrpsee::core::Error::Custom(String::from(
            "approve_content_rating error response TBD",
        )))
    }

    async fn approve_purchase(&self, ctx: CallContext) -> RpcResult<bool> {
        let pin_request = PinChallengeRequestWithContext {
            pin_space: PinSpace::Purchase,
            requestor: ChallengeRequestor {
                id: ctx.app_id.to_owned(),
                name: ctx.app_id.to_owned(),
            },
            capability: Some(String::from(PIN_CHALLENGE_CAPABILITY)),
            call_ctx: ctx.clone(),
        };

        if let Ok(response) = self
            .platform_state
            .get_client()
            .send_extn_request(pin_request)
            .await
        {
            if let Some(ExtnResponse::PinChallenge(v)) = response.payload.extract() {
                return match v.granted {
                    Some(grant) => Ok(grant),
                    None => Ok(false),
                };
            }
        }
        Err(jsonrpsee::core::Error::Custom(String::from(
            "approve_purchase error response TBD",
        )))
    }

    async fn profile_flags(&self, _ctx: CallContext) -> RpcResult<HashMap<String, String>> {
        let distributor_experience_id = self
            .platform_state
            .get_device_manifest()
            .get_distributor_experience_id();
        let mut result = HashMap::new();
        result.insert("userExperience".to_string(), distributor_experience_id);
        Ok(result)
    }
}

pub struct ProfileRPCProvider;

impl RippleRPCProvider<ProfileImpl> for ProfileRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<ProfileImpl> {
        (ProfileImpl {
            platform_state: state,
        })
        .into_rpc()
    }
}
