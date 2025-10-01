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

use jsonrpsee::{core::RpcResult, proc_macros::rpc, tracing::field::debug, RpcModule};

use ripple_sdk::{
    api::{
        apps::{AppEvent, AppManagerResponse, AppMethod, AppRequest, AppResponse},
        caps::CapsRequest,
        firebolt::{
            fb_discovery::{AgePolicy, PolicyIdentifierAlias},
            fb_general::ListenRequestWithEvent,
            fb_telemetry::TelemetryPayload,
        },
        gateway::rpc_gateway_api::{
            CallContext, JsonRpcApiRequest, JsonRpcApiResponse, RpcRequest,
        },
    },
    async_trait::async_trait,
    log::{debug, error},
    service::service_message::JsonRpcRequest,
    tokio::sync::{mpsc, oneshot},
    tokio_tungstenite::tungstenite::http::method,
    utils::rpc_utils::rpc_err,
};

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::{
    broker::{
        broker_utils::BrokerUtils,
        endpoint_broker::{BrokerCallback, BrokerOutput},
    },
    firebolt::rpc::RippleRPCProvider,
    service::{apps::app_events::AppEvents, telemetry_builder::TelemetryBuilder},
    state::platform_state::PlatformState,
    utils::rpc_utils::rpc_await_oneshot,
};

#[rpc(server)]
pub trait Internal {
    #[method(name = "ripple.sendTelemetry")]
    async fn send_telemetry(&self, ctx: CallContext, payload: TelemetryPayload) -> RpcResult<()>;

    #[method(name = "ripple.setTelemetrySessionId")]
    fn set_telemetry_session_id(&self, ctx: CallContext, session_id: String) -> RpcResult<()>;

    #[method(name = "ripple.sendAppEvent")]
    async fn send_app_event(&self, ctx: CallContext, event: AppEvent) -> RpcResult<()>;

    #[method(name = "ripple.registerAppEvent")]
    async fn register_app_event(
        &self,
        ctx: CallContext,
        request: ListenRequestWithEvent,
    ) -> RpcResult<()>;

    #[method(name = "ripple.getAppCatalogId")]
    async fn get_app_catalog_id(&self, ctx: CallContext, app_id: String) -> RpcResult<String>;

    #[method(name = "ripple.checkCapsRequest")]
    async fn check_caps_request(
        &self,
        ctx: CallContext,
        caps_request: CapsRequest,
    ) -> RpcResult<HashMap<String, bool>>;

    #[method(name = "ripple.getSecondScreenPayload")]
    async fn get_second_screen_payload(&self, ctx: CallContext) -> RpcResult<String>;

    #[method(name = "account.setPolicyIdentifierAlias")]
    async fn set_policy_identifier_alias(
        &self,
        ctx: CallContext,
        params: PolicyIdentifierAlias,
    ) -> RpcResult<()>;

    #[method(name = "account.policyIdentifierAlias")]
    async fn get_policy_identifier_alias(&self, ctx: CallContext) -> RpcResult<Vec<AgePolicy>>;

    #[method(name = "broker.route")]
    async fn route_broker_request(
        &self,
        ctx: CallContext,
        enveloped_request: JsonRpcRequest,
    ) -> RpcResult<JsonRpcApiResponse>;
}

#[derive(Debug, Clone, Default)]
pub struct PolicyState {
    pub policy_identifiers_alias: Arc<RwLock<Vec<AgePolicy>>>,
}

#[derive(Debug)]
pub struct InternalImpl {
    pub state: PlatformState,
}

#[async_trait]
impl InternalServer for InternalImpl {
    async fn send_telemetry(&self, _ctx: CallContext, payload: TelemetryPayload) -> RpcResult<()> {
        let _ = TelemetryBuilder::send_telemetry(&self.state, payload);
        Ok(())
    }

    fn set_telemetry_session_id(&self, _ctx: CallContext, session_id: String) -> RpcResult<()> {
        self.state.metrics.update_session_id(Some(session_id));
        Ok(())
    }

    async fn send_app_event(&self, _ctx: CallContext, event: AppEvent) -> RpcResult<()> {
        debug!("Sending App event {:?}", &event);
        AppEvents::emit_with_context(&self.state, &event.event_name, &event.result, event.context)
            .await;
        Ok(())
    }

    async fn register_app_event(
        &self,
        _ctx: CallContext,
        request: ListenRequestWithEvent,
    ) -> RpcResult<()> {
        debug!("registering App event {:?}", &request);
        let event = request.event.clone();
        AppEvents::add_listener(&self.state, event, request.context.clone(), request.request);
        Ok(())
    }

    async fn get_app_catalog_id(&self, _: CallContext, app_id: String) -> RpcResult<String> {
        let (app_resp_tx, app_resp_rx) = oneshot::channel::<AppResponse>();

        let app_request =
            AppRequest::new(AppMethod::GetAppContentCatalog(app_id.clone()), app_resp_tx);
        if let Err(e) = self.state.get_client().send_app_request(app_request) {
            error!("Send error for AppMethod::GetAppContentCatalog {:?}", e);
        }
        let resp = rpc_await_oneshot(app_resp_rx).await;

        if let Ok(Ok(AppManagerResponse::AppContentCatalog(content_catalog))) = resp {
            return Ok(content_catalog.map_or(app_id.to_owned(), |x| x));
        }

        Ok(app_id)
    }

    async fn check_caps_request(
        &self,
        _ctx: CallContext,
        caps_request: CapsRequest,
    ) -> RpcResult<HashMap<String, bool>> {
        match caps_request {
            CapsRequest::Supported(request) => {
                let result = self.state.cap_state.generic.check_for_processor(request);
                Ok(result)
            }
            CapsRequest::Permitted(app_id, request) => {
                let result = self
                    .state
                    .cap_state
                    .permitted_state
                    .check_multiple(&app_id, request);
                Ok(result)
            }
        }
    }

    async fn set_policy_identifier_alias(
        &self,
        _ctx: CallContext,
        params: PolicyIdentifierAlias,
    ) -> RpcResult<()> {
        debug!("Setting policy identifier alias: {:?}", params);
        self.state.add_policy_identifier_alias(params);
        Ok(())
    }

    async fn get_policy_identifier_alias(&self, _ctx: CallContext) -> RpcResult<Vec<AgePolicy>> {
        Ok(self.state.get_policy_identifier_alias())
    }

    async fn get_second_screen_payload(&self, ctx: CallContext) -> RpcResult<String> {
        let (app_resp_tx, app_resp_rx) = oneshot::channel::<AppResponse>();

        let app_request = AppRequest::new(
            AppMethod::GetSecondScreenPayload(ctx.app_id.clone()),
            app_resp_tx,
        );

        if let Err(e) = self.state.get_client().send_app_request(app_request) {
            error!("Send error for GetSecondScreenPayload {:?}", e);
            return Err(rpc_err("Unable to send app request"));
        }

        let resp = rpc_await_oneshot(app_resp_rx).await;
        if let Ok(Ok(AppManagerResponse::SecondScreenPayload(payload))) = resp {
            return Ok(payload);
        }

        // return empty string if the payload is not available
        error!(
            "Failed to get second screen payload for app_id: {}",
            ctx.app_id
        );
        Ok(String::new())
    }
    async fn route_broker_request(
        &self,
        ctx: CallContext,
        enveloped_request: JsonRpcRequest,
    ) -> RpcResult<JsonRpcApiResponse> {
        debug!(
            "route_broker_request called with ctx: {:?}, enveloped_request: {:?}",
            ctx, enveloped_request
        );

        debug!("route_broker_request Creating RPC request for endpoint broker routing");

        // Extract the method and params from the enveloped request

        let params = enveloped_request.params.clone().unwrap();
        let params = params.as_object().unwrap();
        let params = params.get("enveloped_request").unwrap().to_owned();

        // if params.is_none() {
        //     return Err(())
        // }

        let params: JsonRpcRequest = serde_json::from_value(params).unwrap();
        let method = params.method;
        let params = params.params;

        // Create an RPC request for the endpoint broker
        let rpc_request = RpcRequest {
            ctx: ctx.clone(),
            method: method.clone(),
            params_json: if let Some(params) = params {
                serde_json::to_string(&params).unwrap_or_default()
            } else {
                String::new()
            },
        };

        debug!(
            "route_broker_request Routing request to endpoint broker: {:?}",
            rpc_request
        );

        // Route through the endpoint broker instead of calling internal request directly
        let (tx, mut rx) = mpsc::channel::<BrokerOutput>(10);
        let callback: BrokerCallback = BrokerCallback { sender: tx };
        let handled = self.state.endpoint_state.handle_brokerage(
            rpc_request,
            None,           // extn_message
            Some(callback), // custom_callback
            Vec::new(),     // permissions
            None,           // session
            Vec::new(),     // telemetry_response_listeners
        );

        match rx.recv().await {
            Some(broker_output) => {
                debug!(
                    "route_broker_request Received broker output for method: {} {:?}",
                    method, broker_output
                );
                Ok(broker_output.data)
            }
            None => {
                error!("route_broker_request Failed to receive broker output");
                Err(rpc_err(format!(
                    "method {} did not return a response from the endpoint_broker",
                    method
                )))
            }
        }
    }
}

pub struct InternalProvider;
impl RippleRPCProvider<InternalImpl> for InternalProvider {
    fn provide(state: PlatformState) -> RpcModule<InternalImpl> {
        (InternalImpl { state }).into_rpc()
    }
}
