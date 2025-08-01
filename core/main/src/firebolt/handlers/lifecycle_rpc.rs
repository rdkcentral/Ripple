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

use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::{core::async_trait, RpcModule};
use ripple_sdk::{
    api::{
        apps::{AppManagerResponse, AppMethod, AppRequest, AppResponse},
        firebolt::{
            fb_general::{ListenRequest, ListenerResponse},
            fb_lifecycle::{
                CloseRequest, LIFECYCLE_EVENT_ON_ACTIVATE, LIFECYCLE_EVENT_ON_BACKGROUND,
                LIFECYCLE_EVENT_ON_DESTROY, LIFECYCLE_EVENT_ON_FOREGROUND,
                LIFECYCLE_EVENT_ON_HIBERNATE, LIFECYCLE_EVENT_ON_INACTIVE,
                LIFECYCLE_EVENT_ON_PAUSE, LIFECYCLE_EVENT_ON_RESTORE, LIFECYCLE_EVENT_ON_RESUME,
                LIFECYCLE_EVENT_ON_START, LIFECYCLE_EVENT_ON_START_SUSPEND,
                LIFECYCLE_EVENT_ON_SUSPEND, LIFECYCLE_EVENT_ON_SUSPENDED,
                LIFECYCLE_EVENT_ON_UNLOADING,
            },
        },
        gateway::rpc_gateway_api::CallContext,
    },
    tokio::sync::oneshot,
};

use crate::broker::broker_utils::BrokerUtils;
use crate::{
    firebolt::rpc::RippleRPCProvider,
    service::apps::app_events::AppEvents,
    state::platform_state::PlatformState,
    utils::rpc_utils::{rpc_await_oneshot, rpc_err},
};

#[rpc(server)]
pub trait Lifecycle {
    #[method(name = "lifecycle.ready")]
    async fn ready(&self, ctx: CallContext) -> RpcResult<()>;
    #[method(name = "lifecycle.state")]
    async fn state(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "lifecycle.close")]
    async fn close(&self, ctx: CallContext, request: CloseRequest) -> RpcResult<()>;
    #[method(name = "lifecycle.finished")]
    async fn finished(&self, ctx: CallContext) -> RpcResult<()>;
    #[method(name = "lifecycle.onInactive")]
    async fn on_inactive(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "lifecycle.onForeground")]
    async fn on_foreground(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "lifecycle.onBackground")]
    async fn on_background(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "lifecycle.onSuspended")]
    async fn on_suspended(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "lifecycle.onUnloading")]
    async fn on_unloading(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    // Additional lifecycle2.0 listeners
    #[method(name = "lifecycle.onStart")]
    async fn on_start(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "lifecycle.onStartSuspend")]
    async fn on_start_suspend(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "lifecycle.onPause")]
    async fn on_pause(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "lifecycle.onActivate")]
    async fn on_activate(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "lifecycle.onSuspend")]
    async fn on_suspend(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "lifecycle.onResume")]
    async fn on_resume(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "lifecycle.onHibernate")]
    async fn on_hibernate(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "lifecycle.onRestore")]
    async fn on_restore(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "lifecycle.onDestroy")]
    async fn on_destroy(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
}

pub struct LifecycleImpl {
    pub platform_state: PlatformState,
}

impl LifecycleImpl {
    async fn listen(
        &self,
        ctx: CallContext,
        request: ListenRequest,
        event_name: &'static str,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;

        AppEvents::add_listener(
            self.platform_state.clone(),
            event_name.to_string(),
            ctx,
            request,
        );
        Ok(ListenerResponse {
            listening: listen,
            event: event_name.into(),
        })
    }
}

#[async_trait]
impl LifecycleServer for LifecycleImpl {
    async fn ready(&self, ctx: CallContext) -> RpcResult<()> {
        if ctx.is_rpc_v2() {
            if BrokerUtils::process_for_app_main_request(
                self.platform_state.clone(),
                "lifecycle2.ready",
                None,
                ctx.app_id.as_str(),
            )
            .await
            .is_err()
            {
                rpc_err("Error reporting lifecycle 2 state");
            }
            return Ok(());
        }
        let (app_resp_tx, app_resp_rx) = oneshot::channel::<AppResponse>();

        let app_request = AppRequest::new(AppMethod::Ready(ctx.app_id), app_resp_tx);
        if self
            .platform_state
            .get_client()
            .send_app_request(app_request)
            .is_err()
        {
            return Err(rpc_err("Error sending app request"));
        }

        rpc_await_oneshot(app_resp_rx).await??;
        Ok(())
    }

    async fn state(&self, ctx: CallContext) -> RpcResult<String> {
        let (app_resp_tx, app_resp_rx) = oneshot::channel::<AppResponse>();

        let app_request = AppRequest::new(AppMethod::State(ctx.app_id), app_resp_tx);

        if self
            .platform_state
            .get_client()
            .send_app_request(app_request)
            .is_err()
        {
            return Err(rpc_err("Error sending app request"));
        }

        let resp = rpc_await_oneshot(app_resp_rx).await?;
        if let AppManagerResponse::State(state) = resp? {
            return Ok(state.as_string().to_string());
        }

        Err(jsonrpsee::core::Error::Custom(String::from(
            "Internal Error",
        )))
    }

    async fn close(&self, ctx: CallContext, request: CloseRequest) -> RpcResult<()> {
        let (app_resp_tx, app_resp_rx) = oneshot::channel::<AppResponse>();

        let app_request =
            AppRequest::new(AppMethod::Close(ctx.app_id, request.reason), app_resp_tx);

        if self
            .platform_state
            .get_client()
            .send_app_request(app_request)
            .is_err()
        {
            return Err(rpc_err("Error sending app request"));
        }

        rpc_await_oneshot(app_resp_rx).await??;
        Ok(())
    }

    async fn finished(&self, ctx: CallContext) -> RpcResult<()> {
        let (app_resp_tx, app_resp_rx) = oneshot::channel::<AppResponse>();

        let app_request = AppRequest::new(AppMethod::Finished(ctx.app_id), app_resp_tx);
        if self
            .platform_state
            .get_client()
            .send_app_request(app_request)
            .is_err()
        {
            return Err(rpc_err("Error sending app request"));
        }

        rpc_await_oneshot(app_resp_rx).await??;
        Ok(())
    }

    async fn on_inactive(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.listen(ctx, request, LIFECYCLE_EVENT_ON_INACTIVE).await
    }

    async fn on_foreground(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.listen(ctx, request, LIFECYCLE_EVENT_ON_FOREGROUND)
            .await
    }

    async fn on_background(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.listen(ctx, request, LIFECYCLE_EVENT_ON_BACKGROUND)
            .await
    }

    async fn on_suspended(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.listen(ctx, request, LIFECYCLE_EVENT_ON_SUSPENDED)
            .await
    }

    async fn on_unloading(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.listen(ctx, request, LIFECYCLE_EVENT_ON_UNLOADING)
            .await
    }

    // Additional lifecycle2.0 listeners
    async fn on_start(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.listen(ctx, request, LIFECYCLE_EVENT_ON_START).await
    }

    async fn on_start_suspend(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.listen(ctx, request, LIFECYCLE_EVENT_ON_START_SUSPEND)
            .await
    }

    async fn on_pause(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.listen(ctx, request, LIFECYCLE_EVENT_ON_PAUSE).await
    }

    async fn on_activate(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.listen(ctx, request, LIFECYCLE_EVENT_ON_ACTIVATE).await
    }

    async fn on_suspend(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.listen(ctx, request, LIFECYCLE_EVENT_ON_SUSPEND).await
    }

    async fn on_resume(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.listen(ctx, request, LIFECYCLE_EVENT_ON_RESUME).await
    }

    async fn on_hibernate(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.listen(ctx, request, LIFECYCLE_EVENT_ON_HIBERNATE)
            .await
    }

    async fn on_restore(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.listen(ctx, request, LIFECYCLE_EVENT_ON_RESTORE).await
    }

    async fn on_destroy(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.listen(ctx, request, LIFECYCLE_EVENT_ON_DESTROY).await
    }
}

pub struct LifecycleRippleProvider;
impl RippleRPCProvider<LifecycleImpl> for LifecycleRippleProvider {
    fn provide(platform_state: PlatformState) -> RpcModule<LifecycleImpl> {
        (LifecycleImpl { platform_state }).into_rpc()
    }
}
