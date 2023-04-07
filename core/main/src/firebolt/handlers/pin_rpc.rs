use std::iter::Map;

use crate::apps::provider_broker;
use crate::apps::provider_broker::ProviderBroker;
use crate::apps::provider_broker::ProviderRequestPayload;
use crate::apps::provider_broker::ProviderResponse;
use crate::apps::provider_broker::ProviderResponsePayload;
use crate::managers::config_manager::ConfigManager;
use crate::managers::config_manager::ConfigRequest;
use crate::platform_state::PlatformState;
use crate::{
    api::permissions::user_grants::ChallengeRequestor, helpers::ripple_helper::RippleHelper,
};
use crate::{api::rpc::rpc_gateway::RPCProvider, apps::provider_broker::ExternalProviderResponse};
use crate::{
    apps::app_events::ListenerResponse,
    helpers::ripple_helper::{RippleHelperFactory, RippleHelperType},
};
use crate::{
    apps::provider_broker::FocusRequest,
    managers::capability_manager::{
        CapClassifiedRequest, FireboltCap, IGetLoadedCaps, RippleHandlerCaps,
    },
};

use jsonrpsee::core::{async_trait, RpcResult};
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::types::error::CallError;
use jsonrpsee::{core::Error, RpcModule};
use serde::Deserialize;
use serde::Serialize;
use serde_json::value::to_raw_value;
use tokio::sync::oneshot;
use tracing::instrument;

use crate::api::rpc::rpc_gateway::CallContext;
use crate::apps::app_events::ListenRequest;
use std::collections::HashMap;

pub const CHALLENGE_EVENT: &'static str = "pinchallenge.onRequestChallenge";
pub const PIN_CHALLENGE_CAPABILITY: &'static str = "xrn:firebolt:capability:usergrant:pinchallenge";

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PinChallengeRequest {
    pub pin_space: PinSpace,
    pub requestor: ChallengeRequestor,
    pub capability: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum PinChallengeResultReason {
    NoPinRequired,
    NoPinRequiredWindow,
    ExceededPinFailures,
    CorrectPin,
    Cancelled,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PinChallengeResponse {
    granted: bool,
    reason: PinChallengeResultReason,
}

impl PinChallengeResponse {
    pub fn get_granted(&self) -> bool {
        self.granted
    }
    pub fn new(granted: bool, reason: PinChallengeResultReason) -> Self {
        PinChallengeResponse { granted, reason }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum PinSpace {
    Purchase,
    Content,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ShowPinOverlayRequest {
    pin_type: String,
    suppress_snooze: Option<SnoozeOption>,
}


#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum SnoozeOption {
    True,
    False,
}



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

pub struct PinChallengeImpl<IRippleHelper> {
    pub helper: Box<IRippleHelper>,
    pub platform_state: PlatformState,
}

#[async_trait]
impl PinChallengeServer for PinChallengeImpl<RippleHelper> {
    #[instrument(skip(self))]
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
            CHALLENGE_EVENT,
            ctx,
            request,
        )
        .await;
        Ok(ListenerResponse {
            listening: listen,
            event: CHALLENGE_EVENT,
        })
    }
    #[instrument(skip(self))]
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

    #[instrument(skip(self))]
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

    async fn approve_content_rating(&self, ctx: CallContext) -> RpcResult<bool> {
        let resp = self.approve_by_pin(ctx, PinSpace::Content).await;
        match resp {
            Ok(r) => Ok(r.granted),
            Err(e) => Err(e),
        }
    }

    async fn approve_purchase(&self, ctx: CallContext) -> RpcResult<bool> {
        let resp = self.approve_by_pin(ctx, PinSpace::Purchase).await;
        match resp {
            Ok(r) => Ok(r.granted),
            Err(e) => Err(e),
        }
    }
    async fn profile_flags(&self, _ctx: CallContext) -> RpcResult<HashMap<String, String>> {
        let config_manager = ConfigManager::get();
        let distributor_experience_id = ConfigManager::get().distributor_experience_id();
        let mut result = HashMap::new();
        result.insert("userExperience".to_string(), distributor_experience_id);
        Ok(result)
    }
}

impl PinChallengeImpl<RippleHelper> {
    async fn approve_by_pin(
        &self,
        ctx: CallContext,
        pin_space: PinSpace,
    ) -> RpcResult<PinChallengeResponse> {
        let req = PinChallengeRequest {
            pin_space,
            requestor: ChallengeRequestor {
                id: ctx.app_id.clone(),
                name: ctx.app_id.clone(),
            },
            capability: None,
        };
        let (session_tx, session_rx) = oneshot::channel::<ProviderResponsePayload>();
        let pr_msg = provider_broker::Request {
            capability: String::from(PIN_CHALLENGE_CAPABILITY),
            method: String::from("challenge"),
            caller: ctx,
            request: ProviderRequestPayload::PinChallenge(req),
            tx: session_tx,
            app_id: None,
        };
        ProviderBroker::invoke_method(&self.platform_state, pr_msg).await;
        match session_rx.await {
            Ok(result) => match result.as_pin_challenge_response() {
                Some(res) => Ok(res),
                None => Err(Error::Custom(String::from(
                    "Invalid response back from provider",
                ))),
            },
            Err(_) => Err(Error::Custom(String::from(
                "Error returning back from pin challenge provider",
            ))),
        }
    }
}

pub struct PinProvider;

pub struct PinCapHandler;

impl IGetLoadedCaps for PinCapHandler {
    fn get_loaded_caps(&self) -> RippleHandlerCaps {
        RippleHandlerCaps {
            caps: Some(vec![
                CapClassifiedRequest::Supported(vec![
                    FireboltCap::Short("approve:purchase".into()),
                    FireboltCap::Short("approve:content".into()),
                    FireboltCap::Short("usergrant:pinchallenge".into()),
                ]),
                CapClassifiedRequest::NotAvailable(vec![FireboltCap::Short(
                    "usergrant:pinchallenge".into(),
                )]),
            ]),
        }
    }
}

impl RPCProvider<PinChallengeImpl<RippleHelper>, PinCapHandler> for PinProvider {
    fn provide(
        self,
        rhf: Box<RippleHelperFactory>,
        platform_state: PlatformState,
    ) -> (RpcModule<PinChallengeImpl<RippleHelper>>, PinCapHandler) {
        let a = PinChallengeImpl {
            helper: rhf.get(self.get_helper_variant()),
            platform_state,
        };
        (a.into_rpc(), PinCapHandler)
    }

    fn get_helper_variant(self) -> Vec<RippleHelperType> {
        vec![]
    }
}

#[cfg(test)]
mod tests {

    use std::collections::HashMap;

    use jsonrpsee::core::Error;
    use serde_json::Value;
    use tokio::sync::{mpsc, oneshot};
    use tracing::debug;

    use crate::api::rpc::api_messages::ApiMessage;
    use crate::api::rpc::firebolt_gateway::tests::TestGateway;
    use crate::apps::app_events::ListenRequest;
    use crate::apps::provider_broker::{ExternalProviderRequest, ExternalProviderResponse};
    use crate::helpers::ripple_helper::mock_ripple_helper::MockRippleBuilder;
    use crate::managers::config_manager::{ConfigManager, ConfigRequest};
    use crate::platform_state::PlatformState;
    use crate::{
        api::handlers::pin_challenge::{
            PinChallengeRequest, PinSpace,
            ShowPinOverlayRequest,
        },
        helpers::ripple_helper::RippleHelper,
    };

    use super::{PinChallengeImpl, PinChallengeResponse};
    use super::{PinChallengeServer, SnoozeOption};

    struct PinChallengeProviderApp {}

    impl PinChallengeProviderApp {
        ///
        /// Starts a thread that handles pin challenge provider events
        /// and responses immediately with the given result
        ///
        /// # Arguments
        ///
        /// * `session_rx` - The channel of which jsonrpc messages come in
        /// * `providers_tx` - Where to send provider commands
        /// * `result` - The result to respond with to pin challenges
        async fn start(
            mut session_rx: mpsc::Receiver<ApiMessage>,
            helper: Box<RippleHelper>,
            purchase_result: PinChallengeResponse,
            content_result: PinChallengeResponse,
            platform_state: PlatformState,
        ) {
            let (provider_rdy_tx, provider_rdy_rx) = oneshot::channel();
            tokio::spawn(async move {
                let provider_handler = PinChallengeImpl {
                    helper,
                    platform_state,
                };
                let ctx_provider = TestGateway::provider_call();

                provider_handler
                    .on_request_challenge(ctx_provider.clone(), ListenRequest { listen: true })
                    .await
                    .unwrap();
                provider_rdy_tx.send(()).unwrap();
                while let Some(message) = session_rx.recv().await {
                    debug!("Got message {}", message.jsonrpc_msg);
                    let jsonrpc: Value =
                        serde_json::from_str(message.jsonrpc_msg.as_str()).unwrap();
                    let req: ExternalProviderRequest<PinChallengeRequest> =
                        serde_json::from_value(jsonrpc.get("result").unwrap().clone()).unwrap();
                    let result = match req.parameters.pin_space {
                        PinSpace::Purchase => purchase_result.clone(),
                        PinSpace::Content => content_result.clone(),
                    };

                    provider_handler
                        .challenge_response(
                            ctx_provider.clone(),
                            ExternalProviderResponse {
                                correlation_id: req.correlation_id,
                                result: result,
                            },
                        )
                        .await
                        .unwrap();
                }
            });
            provider_rdy_rx.await.unwrap();
        }
    }

    #[tokio::test]
    async fn approve_purchase_and_content_test() {
        let state = PlatformState::default();
        let test_gateway = TestGateway::start(state.clone()).await;
        PinChallengeProviderApp::start(
            test_gateway.prov_session_rx,
            test_gateway.pin_helper.clone(),
            PinChallengeResponse {
                granted: true,
                reason: super::PinChallengeResultReason::CorrectPin,
            },
            PinChallengeResponse {
                granted: false,
                reason: super::PinChallengeResultReason::Cancelled,
            },
            state.clone(),
        )
        .await;

        let handler = PinChallengeImpl {
            helper: test_gateway.pin_helper,
            platform_state: state,
        };
        let ctx_cons = TestGateway::consumer_call();
        let resp = handler.approve_purchase(ctx_cons.clone()).await.unwrap();
        assert_eq!(resp, true, "Expected purchase to be approved");
        let resp = handler.approve_content_rating(ctx_cons).await.unwrap();
        assert_eq!(resp, false, "Expected content to be declined");
    }

    #[tokio::test]
    async fn show_pin_overlay_test() {
        let state = PlatformState::default();
        let test_gateway = TestGateway::start(state.clone()).await;
        PinChallengeProviderApp::start(
            test_gateway.prov_session_rx,
            test_gateway.pin_helper.clone(),
            PinChallengeResponse {
                granted: false,
                reason: super::PinChallengeResultReason::ExceededPinFailures,
            },
            PinChallengeResponse {
                granted: true,
                reason: super::PinChallengeResultReason::NoPinRequired,
            },
            state.clone(),
        )
        .await;

        let handler = PinChallengeImpl {
            helper: test_gateway.pin_helper,
            platform_state: state,
        };
        let ctx_cons = TestGateway::consumer_call();
        let req = ShowPinOverlayRequest {
            pin_type: String::from("purchase_pin"),
            suppress_snooze: Some(super::SnoozeOption::True),
        };
        
        let req = ShowPinOverlayRequest {
            pin_type: String::from("content_pin"),
            suppress_snooze: Some(super::SnoozeOption::True),
        };
        
    }

    #[tokio::test]
    async fn profile_flags_test() {
        let ripple_helper = MockRippleBuilder::new().build();
        let platform_state = PlatformState::default();
        let under_test = PinChallengeImpl {
            helper: Box::new(ripple_helper),
            platform_state: platform_state,
        };
        let result = under_test
            .profile_flags(TestGateway::consumer_call())
            .await
            .unwrap();
        let distributor_experience_id = ConfigManager::get().distributor_experience_id();
        let mut expected = HashMap::new();
        expected.insert("userExperience".to_string(), distributor_experience_id);
        assert_eq!(result, expected);
    }
}
