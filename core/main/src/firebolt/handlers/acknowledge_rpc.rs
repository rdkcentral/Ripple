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

use crate::{
    firebolt::rpc::RippleRPCProvider, service::apps::provider_broker::ProviderBroker,
    state::platform_state::PlatformState,
};
use jsonrpsee::{core::RpcResult, proc_macros::rpc, RpcModule};
use ripple_sdk::async_trait::async_trait;
use ripple_sdk::{
    api::{
        firebolt::{
            fb_general::{ListenRequest, ListenerResponse},
            provider::{
                ChallengeResponse, ExternalProviderResponse, FocusRequest, ProviderResponse,
                ProviderResponsePayload, ACK_CHALLENGE_CAPABILITY, ACK_CHALLENGE_EVENT,
            },
        },
        gateway::rpc_gateway_api::CallContext,
    },
    log::debug,
};

#[rpc(server)]
pub trait AcknowledgeChallenge {
    #[method(name = "acknowledgechallenge.onRequestChallenge")]
    async fn on_request_challenge(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "acknowledgechallenge.challengeFocus")]
    async fn challenge_focus(
        &self,
        ctx: CallContext,
        request: FocusRequest,
    ) -> RpcResult<Option<()>>;
    #[method(name = "acknowledgechallenge.challengeResponse")]
    async fn challenge_response(
        &self,
        ctx: CallContext,
        resp: ExternalProviderResponse<ChallengeResponse>,
    ) -> RpcResult<Option<()>>;
}

pub struct AcknowledgeChallengeImpl {
    pub platform_state: PlatformState,
}

#[async_trait]
impl AcknowledgeChallengeServer for AcknowledgeChallengeImpl {
    async fn on_request_challenge(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        debug!("Acknowledgechallenge provider registered :{:?}", request);
        ProviderBroker::register_or_unregister_provider(
            &self.platform_state,
            String::from(ACK_CHALLENGE_CAPABILITY),
            String::from("challenge"),
            ACK_CHALLENGE_EVENT,
            ctx,
            request,
        )
        .await;

        Ok(ListenerResponse {
            listening: listen,
            event: ACK_CHALLENGE_EVENT.into(),
        })
    }

    async fn challenge_response(
        &self,
        _ctx: CallContext,
        resp: ExternalProviderResponse<ChallengeResponse>,
    ) -> RpcResult<Option<()>> {
        ProviderBroker::provider_response(
            &self.platform_state,
            ProviderResponse {
                correlation_id: resp.correlation_id,
                result: ProviderResponsePayload::ChallengeResponse(resp.result),
            },
        )
        .await;
        Ok(None)
    }

    async fn challenge_focus(
        &self,
        ctx: CallContext,
        request: FocusRequest,
    ) -> RpcResult<Option<()>> {
        ProviderBroker::focus(
            &self.platform_state,
            ctx,
            ACK_CHALLENGE_CAPABILITY.to_string(),
            request,
        )
        .await;
        Ok(None)
    }
}

pub struct AckRPCProvider;

impl RippleRPCProvider<AcknowledgeChallengeImpl> for AckRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<AcknowledgeChallengeImpl> {
        (AcknowledgeChallengeImpl {
            platform_state: state,
        })
        .into_rpc()
    }
}
