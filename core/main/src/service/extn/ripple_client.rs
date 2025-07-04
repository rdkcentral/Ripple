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

use std::sync::{Arc, RwLock};

use ripple_sdk::{
    api::apps::{AppError, AppManagerResponse, AppMethod, AppRequest},
    extn::{
        client::{
            extn_client::ExtnClient,
            extn_processor::{ExtnEventProcessor, ExtnRequestProcessor},
        },
        extn_client_message::{ExtnMessage, ExtnPayloadProvider},
        extn_id::ExtnId,
    },
    framework::RippleResponse,
    log::error,
    tokio::sync::{mpsc::Sender, oneshot},
    utils::error::RippleError,
};

use crate::{
    broker::endpoint_broker::BrokerOutput, firebolt::firebolt_gateway::FireboltGatewayCommand,
    state::bootstrap_state::ChannelsState, utils::rpc_utils::rpc_await_oneshot,
};

/// RippleClient is an internal delegate component which helps in operating
/// 1. ExtnClient
/// 2. Firebolt Gateway
/// 3. Capabilities
/// 4. Usergrants
///
/// # Examples
/// ```
/// use crate::firebolt::firebolt_gateway::FireboltGatewayCommand;
/// fn send_gateway_command(msg: FireboltGatewayCommand) {
///     let client = RippleClient::new();
///     client.send_gateway_command()
/// }
///
/// ```
#[derive(Debug, Clone)]
pub struct RippleClient {
    client: Arc<RwLock<ExtnClient>>,
    gateway_sender: Sender<FireboltGatewayCommand>,
    app_mgr_sender: Sender<AppRequest>, // will be used by LCM RPC
    broker_sender: Sender<BrokerOutput>,
}

impl RippleClient {
    pub fn new(state: ChannelsState) -> RippleClient {
        let extn_client = ExtnClient::new_main();
        RippleClient {
            gateway_sender: state.get_gateway_sender(),
            app_mgr_sender: state.get_app_mgr_sender(),
            client: Arc::new(RwLock::new(extn_client)),
            broker_sender: state.get_broker_sender(),
        }
    }

    #[cfg(test)]
    pub fn test_client(extn_client: ExtnClient) -> RippleClient {
        let cs = ChannelsState::new();
        RippleClient {
            client: Arc::new(RwLock::new(extn_client)),
            gateway_sender: cs.get_gateway_sender(),
            app_mgr_sender: cs.get_app_mgr_sender(),
            broker_sender: cs.get_broker_sender(),
        }
    }

    pub fn send_gateway_command(&self, cmd: FireboltGatewayCommand) -> Result<(), RippleError> {
        if let Err(e) = self.gateway_sender.try_send(cmd) {
            error!("failed to send firebolt gateway message {:?}", e);
            return Err(RippleError::SendFailure);
        }
        Ok(())
    }

    pub fn send_app_request(&self, request: AppRequest) -> Result<(), RippleError> {
        if let Err(e) = self.app_mgr_sender.try_send(request) {
            error!("failed to send firebolt app message {:?}", e);
            return Err(RippleError::SendFailure);
        }
        Ok(())
    }

    pub async fn get_app_state(&self, app_id: &str) -> Result<String, RippleError> {
        let (app_resp_tx, app_resp_rx) = oneshot::channel::<Result<AppManagerResponse, AppError>>();
        let app_request = AppRequest::new(AppMethod::State(app_id.to_owned()), app_resp_tx);
        if let Err(e) = self.send_app_request(app_request) {
            error!("Send error for get_state {:?}", e);
            return Err(RippleError::SendFailure);
        }
        let resp: Result<Result<AppManagerResponse, AppError>, jsonrpsee::core::Error> =
            rpc_await_oneshot(app_resp_rx).await;
        if let Ok(Ok(AppManagerResponse::State(state))) = resp {
            return Ok(state.as_string().to_string());
        }
        Err(RippleError::SendFailure)
    }

    pub fn get_extn_client(&self) -> ExtnClient {
        self.client.read().unwrap().clone()
    }

    pub async fn send_extn_request(
        &self,
        payload: impl ExtnPayloadProvider,
    ) -> Result<ExtnMessage, RippleError> {
        self.get_extn_client().main_internal_request(payload).await
    }
    pub fn send_extn_request_transient(&self, payload: impl ExtnPayloadProvider) -> RippleResponse {
        self.get_extn_client().request_transient(payload)?;
        Ok(())
    }

    pub async fn respond(&self, msg: ExtnMessage) -> Result<(), RippleError> {
        self.get_extn_client().clone().send_message(msg).await
    }

    pub fn add_request_processor(&self, stream_processor: impl ExtnRequestProcessor) {
        self.get_extn_client()
            .add_request_processor(stream_processor)
    }

    pub fn add_event_processor(&self, stream_processor: impl ExtnEventProcessor) {
        self.get_extn_client().add_event_processor(stream_processor)
    }

    pub fn cleanup_event_processor(&self, capability: ExtnId) {
        self.get_extn_client().cleanup_event_stream(capability);
    }

    pub fn send_event(&self, event: impl ExtnPayloadProvider) -> RippleResponse {
        self.get_extn_client().event(event)
    }

    pub fn get_broker_sender(&self) -> Sender<BrokerOutput> {
        self.broker_sender.clone()
    }

    pub fn get_gateway_sender(&self) -> Sender<FireboltGatewayCommand> {
        self.gateway_sender.clone()
    }
}
