use crate::{
    api::{
        permissions::user_grants::ChallengeResponse,
        rpc::rpc_gateway::{CallContext, RPCProvider},
    },
    apps::{
        app_events::{ListenRequest, ListenerResponse},
        provider_broker::{
            ExternalProviderResponse, FocusRequest, ProviderBroker, ProviderResponse,
            ProviderResponsePayload,
        },
    },
    helpers::ripple_helper::{RippleHelper, RippleHelperFactory, RippleHelperType},
    managers::capability_manager::{
        CapClassifiedRequest, FireboltCap, IGetLoadedCaps, RippleHandlerCaps,
    },
    platform_state::PlatformState,
};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    RpcModule,
};
use tracing::debug;
use tracing::instrument;

pub const CHALLENGE_EVENT: &'static str = "acknowledgechallenge.onRequestChallenge";
pub const ACK_CHALLENGE_CAPABILITY: &'static str =
    "xrn:firebolt:capability:usergrant:acknowledgechallenge";

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

pub struct AcknowledgeChallengeImpl<IRippleHelper> {
    pub helper: Box<IRippleHelper>,
    pub platform_state: PlatformState,
}

#[async_trait]
impl AcknowledgeChallengeServer for AcknowledgeChallengeImpl<RippleHelper> {
    #[instrument(skip(self))]
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

    #[instrument(skip(self))]
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

pub struct AckProvider;

pub struct AckCapHandler;

impl IGetLoadedCaps for AckCapHandler {
    fn get_loaded_caps(&self) -> RippleHandlerCaps {
        RippleHandlerCaps {
            caps: Some(vec![
                CapClassifiedRequest::Supported(vec![FireboltCap::Short(
                    "usergrant:acknowledgechallenge".into(),
                )]),
                CapClassifiedRequest::NotAvailable(vec![FireboltCap::Short(
                    "usergrant:acknowledgechallenge".into(),
                )]),
            ]),
        }
    }
}

impl RPCProvider<AcknowledgeChallengeImpl<RippleHelper>, AckCapHandler> for AckProvider {
    fn provide(
        self,
        rhf: Box<RippleHelperFactory>,
        platform_state: PlatformState,
    ) -> (
        RpcModule<AcknowledgeChallengeImpl<RippleHelper>>,
        AckCapHandler,
    ) {
        let a = AcknowledgeChallengeImpl {
            helper: rhf.get(self.get_helper_variant()),
            platform_state,
        };
        (a.into_rpc(), AckCapHandler)
    }

    fn get_helper_variant(self) -> Vec<RippleHelperType> {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        api::rpc::{api_messages::ApiProtocol, firebolt_gateway::tests::TestGateway},
        apps::app_mgr::AppRequest,
        helpers::channel_util::oneshot_send_and_log,
        managers::capability_manager::CapRequest,
        platform_state::PlatformState,
    };
    use dab::core::message::DabRequest;
    use dpab::core::message::DpabRequest;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_on_request_challenge() {
        let (helper, ctx, dab_rx, dpab_rx, cap_rx, app_events_rx, ps) = helper();
        let request = ListenRequest { listen: true };
        tokio::spawn(async move {
            mock_channel(cap_rx).await;
        });
        let resp = AcknowledgeChallengeImpl {
            helper,
            platform_state: ps,
        }
        .on_request_challenge(ctx, request)
        .await
        .unwrap();
        assert!(resp.listening);
    }

    fn helper() -> (
        Box<RippleHelper>,
        CallContext,
        mpsc::Receiver<DabRequest>,
        mpsc::Receiver<DpabRequest>,
        mpsc::Receiver<CapRequest>,
        mpsc::Receiver<AppRequest>,
        PlatformState,
    ) {
        let (dab_tx, dab_rx) = mpsc::channel::<DabRequest>(32);
        let (dpab_tx, dpab_rx) = mpsc::channel::<DpabRequest>(32);
        let (cap_tx, cap_rx) = mpsc::channel::<CapRequest>(32);
        let (app_mgr_req_tx, app_mgr_req_rx) = mpsc::channel::<AppRequest>(32);
        let (pb_tx, pb_rx) = mpsc::channel::<ProviderResponse>(32);
        let mut helper = RippleHelper::default();
        helper.sender_hub.dab_tx = Some(dab_tx.clone());
        helper.sender_hub.dpab_tx = Some(dpab_tx.clone());
        helper.sender_hub.cap_tx = Some(cap_tx.clone());
        helper.sender_hub.app_mgr_req_tx = Some(app_mgr_req_tx);
        let ctx = CallContext {
            session_id: "a".to_string(),
            request_id: "b".to_string(),
            app_id: "test".to_string(),
            call_id: 5,
            protocol: ApiProtocol::JsonRpc,
            method: "method".to_string(),
        };
        let mut ps = PlatformState::default();
        ps.services = helper.clone();
        ps.app_auth_sessions.rh = Some(Box::new(helper.clone()));
        return (
            Box::new(helper),
            ctx,
            dab_rx,
            dpab_rx,
            cap_rx,
            app_mgr_req_rx,
            ps,
        );
    }

    async fn mock_channel(mut cap_rx: mpsc::Receiver<CapRequest>) {
        loop {
            tokio::select! {
                data = cap_rx.recv() => {
                    if let Some(request) = data{
                        if let CapRequest::UpdateAvailability(_,occ) = request{
                            if let Some(tx) = occ{
                                tx.callback.send(Ok(())).unwrap();
                            }
                        }
                    }
                }
            }
        }
    }
}
