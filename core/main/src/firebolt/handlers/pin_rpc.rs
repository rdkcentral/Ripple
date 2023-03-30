use jsonrpsee::{proc_macros::rpc, core::RpcResult, RpcModule};
use ripple_sdk::api::{gateway::rpc_gateway_api::CallContext, firebolt::{fb_general::{ListenRequest, ListenerResponse}, provider::{FocusRequest, ExternalProviderResponse, ProviderResponse, ProviderResponsePayload}, fb_pin::{PinChallengeResponse, PIN_CHALLENGE_CAPABILITY, PIN_CHALLENGE_EVENT}}};
use ripple_sdk::async_trait::async_trait;
use crate::{state::platform_state::PlatformState, service::apps::provider_broker::ProviderBroker, firebolt::rpc::RippleRPCProvider};

#[rpc(server)]
pub trait PinChallenge {
    #[method(name = "pinchallenge.onRequestChallenge")]
    async fn on_request_challenge(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "pinchallenge.challengeFocus")]
    async fn challenge_focus(
        &self,
        ctx: CallContext,
        request: FocusRequest,
    ) -> RpcResult<Option<()>>;

    #[method(name = "pinchallenge.challengeResponse")]
    async fn challenge_response(
        &self,
        ctx: CallContext,
        resp: ExternalProviderResponse<PinChallengeResponse>,
    ) -> RpcResult<Option<()>>;
}

pub struct PinChallengeImpl {
    pub platform_state: PlatformState
}

#[async_trait]
impl PinChallengeServer for PinChallengeImpl {
    async fn on_request_challenge(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        ProviderBroker::register_or_unregister_provider(
            &self.platform_state,
            String::from(PIN_CHALLENGE_CAPABILITY),
            String::from("challenge"),
            PIN_CHALLENGE_EVENT,
            ctx,
            request,
        )
        .await;
        Ok(ListenerResponse {
            listening: listen,
            event: PIN_CHALLENGE_EVENT,
        })
    }

    async fn challenge_response(
        &self,
        _ctx: CallContext,
        resp: ExternalProviderResponse<PinChallengeResponse>,
    ) -> RpcResult<Option<()>> {
        let msg = ProviderResponse {
            correlation_id: resp.correlation_id,
            result: ProviderResponsePayload::PinChallengeResponse(resp.result),
        };
        ProviderBroker::provider_response(&self.platform_state, msg).await;
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
            PIN_CHALLENGE_CAPABILITY.to_string(),
            request,
        )
        .await;
        Ok(None)
    }
}

pub struct PinRPCProvider;

impl RippleRPCProvider<PinChallengeImpl> for PinRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<PinChallengeImpl> {
        (PinChallengeImpl { platform_state: state }).into_rpc()
    }
}