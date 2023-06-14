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

use std::sync::{Arc, RwLock};

use ripple_sdk::{
    api::{apps::AppRequest, manifest::extn_manifest::ExtnSymbol},
    crossbeam::channel::Sender as CSender,
    extn::{
        client::{
            extn_client::ExtnClient,
            extn_processor::{ExtnEventProcessor, ExtnRequestProcessor},
            extn_sender::ExtnSender,
        },
        extn_client_message::{ExtnMessage, ExtnPayloadProvider},
        extn_id::ExtnId,
        ffi::ffi_message::CExtnMessage,
    },
    framework::RippleResponse,
    log::error,
    tokio::{self, sync::mpsc::Sender},
    utils::error::RippleError,
};

use crate::{
    firebolt::firebolt_gateway::FireboltGatewayCommand, state::bootstrap_state::ChannelsState,
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
}

impl RippleClient {
    pub fn new(state: ChannelsState) -> RippleClient {
        let capability = ExtnId::get_main_target("main".into());
        let extn_sender = ExtnSender::new(state.get_extn_sender(), capability, Vec::new());
        let extn_client = ExtnClient::new(state.get_extn_receiver(), extn_sender);
        RippleClient {
            gateway_sender: state.get_gateway_sender(),
            app_mgr_sender: state.get_app_mgr_sender(),
            client: Arc::new(RwLock::new(extn_client)),
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

    pub fn get_extn_client(&self) -> ExtnClient {
        self.client.read().unwrap().clone()
    }

    pub async fn init(&self) {
        let client = self.get_extn_client();
        tokio::spawn(async move { client.initialize().await });
    }

    pub async fn send_extn_request(
        &self,
        payload: impl ExtnPayloadProvider,
    ) -> Result<ExtnMessage, RippleError> {
        self.get_extn_client().clone().request(payload).await
    }

    pub async fn respond(&self, msg: ExtnMessage) -> Result<(), RippleError> {
        self.get_extn_client().clone().send_message(msg).await
    }

    pub fn add_request_processor(&self, stream_processor: impl ExtnRequestProcessor) {
        self.get_extn_client()
            .clone()
            .add_request_processor(stream_processor)
    }

    pub fn add_event_processor(&self, stream_processor: impl ExtnEventProcessor) {
        self.get_extn_client()
            .clone()
            .add_event_processor(stream_processor)
    }

    pub fn add_extn_sender(&self, id: ExtnId, symbol: ExtnSymbol, sender: CSender<CExtnMessage>) {
        self.get_extn_client()
            .clone()
            .add_sender(id, symbol, sender);
    }

    pub fn cleanup_event_processor(&self, capability: ExtnId) {
        self.get_extn_client()
            .clone()
            .cleanup_event_stream(capability);
    }

    pub fn send_event(&self, event: impl ExtnPayloadProvider) -> RippleResponse {
        self.get_extn_client().event(event)
    }
}
