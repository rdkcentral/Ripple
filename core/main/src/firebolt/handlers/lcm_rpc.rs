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

use jsonrpsee::{core::RpcResult, proc_macros::rpc, RpcModule};
use ripple_sdk::{
    api::{
        apps::{AppManagerResponse, AppMethod, AppRequest, AppResponse},
        firebolt::{
            fb_general::{ListenRequest, ListenerResponse},
            fb_lifecycle_management::{
                AppSessionRequest, SessionResponse, SetStateRequest, LCM_EVENT_ON_REQUEST_CLOSE,
                LCM_EVENT_ON_REQUEST_FINISHED, LCM_EVENT_ON_REQUEST_LAUNCH,
                LCM_EVENT_ON_REQUEST_READY,
            },
        },
        gateway::rpc_gateway_api::CallContext,
    },
    async_trait::async_trait,
    log::error,
    tokio::sync::oneshot,
};

use crate::{
    firebolt::rpc::RippleRPCProvider,
    service::apps::provider_broker::ProviderBroker,
    state::platform_state::PlatformState,
    utils::rpc_utils::{rpc_await_oneshot, rpc_err},
};

#[rpc(server)]
pub trait LifecycleManagement {
    #[method(name = "lifecyclemanagement.setState")]
    async fn set_state(&self, ctx: CallContext, request: SetStateRequest) -> RpcResult<()>;
    #[method(name = "lifecyclemanagement.onRequestReady")]
    async fn on_request_ready(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "lifecyclemanagement.onRequestClose")]
    async fn on_request_close(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "lifecyclemanagement.onRequestFinished")]
    async fn on_request_finished(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "lifecyclemanagement.onRequestLaunch")]
    async fn on_request_launch(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "lifecyclemanagement.session")]
    async fn session(
        &self,
        ctx: CallContext,
        session: AppSessionRequest,
    ) -> RpcResult<SessionResponse>;
}

#[derive(Debug)]
pub struct LifecycleManagementImpl {
    pub state: PlatformState,
}

impl LifecycleManagementImpl {
    pub async fn on_request_app_event(
        &self,
        ctx: CallContext,
        request: ListenRequest,
        method: &'static str,
        event_name: &'static str,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        ProviderBroker::register_or_unregister_provider(
            &self.state,
            // TODO update with Firebolt Cap in later effort
            "xrn::firebolt::app:lifecycle".into(),
            method.into(),
            event_name,
            ctx,
            request,
        )
        .await;

        Ok(ListenerResponse {
            listening: listen,
            event: event_name.into(),
        })
    }
}

#[async_trait]
impl LifecycleManagementServer for LifecycleManagementImpl {
    async fn set_state(&self, _ctx: CallContext, request: SetStateRequest) -> RpcResult<()> {
        let (app_resp_tx, app_resp_rx) = oneshot::channel::<AppResponse>();

        let app_request = AppRequest::new(
            AppMethod::SetState(request.app_id, request.state),
            app_resp_tx,
        );

        if let Err(e) = self.state.get_client().send_app_request(app_request) {
            error!("Send error for set_state {:?}", e);
            return Err(rpc_err("Unable send app request"));
        }
        rpc_await_oneshot(app_resp_rx).await??;
        Ok(())
    }

    async fn on_request_ready(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_app_event(ctx, request, "ready", LCM_EVENT_ON_REQUEST_READY)
            .await
    }

    async fn on_request_close(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_app_event(ctx, request, "close", LCM_EVENT_ON_REQUEST_CLOSE)
            .await
    }

    async fn on_request_finished(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_app_event(ctx, request, "finished", LCM_EVENT_ON_REQUEST_FINISHED)
            .await
    }

    async fn on_request_launch(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_app_event(ctx, request, "launch", LCM_EVENT_ON_REQUEST_LAUNCH)
            .await
    }

    async fn session(
        &self,
        _ctx: CallContext,
        req: AppSessionRequest,
    ) -> RpcResult<SessionResponse> {
        let (app_resp_tx, app_resp_rx) = oneshot::channel::<AppResponse>();

        let app_request = AppRequest::new(AppMethod::BrowserSession(req.session), app_resp_tx);

        if let Err(e) = self.state.get_client().send_app_request(app_request) {
            error!("Send error for set_state {:?}", e);
            return Err(rpc_err("Unable send app request"));
        }
        if let Ok(r) = app_resp_rx.await {
            if let Ok(s) = r {
                match s {
                    AppManagerResponse::SessionId(session_id) => {
                        return Ok(SessionResponse { session_id })
                    }
                    _ => error!("unable to register session"),
                }
            } else {
                error!("Unable to register session")
            }
        } else {
            error!("Unable to register session")
        }
        Err(rpc_err("unable to register session"))
    }
}

pub struct LifecycleManagementProvider;
impl RippleRPCProvider<LifecycleManagementImpl> for LifecycleManagementProvider {
    fn provide(state: PlatformState) -> RpcModule<LifecycleManagementImpl> {
        (LifecycleManagementImpl { state }).into_rpc()
    }
}
