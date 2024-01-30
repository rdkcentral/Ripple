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

use ripple_sdk::{
    api::apps::AppRequest,
    async_channel::{unbounded, Receiver as CReceiver, Sender as CSender},
    extn::ffi::ffi_message::CExtnMessage,
    framework::bootstrap::TransientChannel,
    tokio::sync::mpsc::{self, Receiver, Sender},
    utils::error::RippleError,
};

use crate::{
    bootstrap::manifest::{
        apps::LoadAppLibraryStep, device::LoadDeviceManifestStep, extn::LoadExtnManifestStep,
    },
    broker::endpoint_broker::BrokerOutput,
    firebolt::firebolt_gateway::FireboltGatewayCommand,
    service::extn::ripple_client::RippleClient,
};

use super::{extn_state::ExtnState, platform_state::PlatformState};

#[derive(Debug, Clone)]
pub struct ChannelsState {
    gateway_channel: TransientChannel<FireboltGatewayCommand>,
    app_req_channel: TransientChannel<AppRequest>,
    extn_sender: CSender<CExtnMessage>,
    extn_receiver: CReceiver<CExtnMessage>,
    broker_channel: TransientChannel<BrokerOutput>,
}

impl ChannelsState {
    pub fn new() -> ChannelsState {
        let (gateway_tx, gateway_tr) = mpsc::channel(32);
        let (app_req_tx, app_req_tr) = mpsc::channel(32);
        let (ctx, ctr) = unbounded();
        let (broker_tx, broker_rx) = mpsc::channel(10);

        ChannelsState {
            gateway_channel: TransientChannel::new(gateway_tx, gateway_tr),
            app_req_channel: TransientChannel::new(app_req_tx, app_req_tr),
            extn_sender: ctx,
            extn_receiver: ctr,
            broker_channel: TransientChannel::new(broker_tx, broker_rx),
        }
    }

    pub fn get_app_mgr_sender(&self) -> Sender<AppRequest> {
        self.app_req_channel.get_sender()
    }

    pub fn get_app_mgr_receiver(&self) -> Result<Receiver<AppRequest>, RippleError> {
        self.app_req_channel.get_receiver()
    }

    pub fn get_gateway_sender(&self) -> Sender<FireboltGatewayCommand> {
        self.gateway_channel.get_sender()
    }

    pub fn get_gateway_receiver(&self) -> Result<Receiver<FireboltGatewayCommand>, RippleError> {
        self.gateway_channel.get_receiver()
    }

    pub fn get_extn_sender(&self) -> CSender<CExtnMessage> {
        self.extn_sender.clone()
    }

    pub fn get_extn_receiver(&self) -> CReceiver<CExtnMessage> {
        self.extn_receiver.clone()
    }

    pub fn get_iec_channel() -> (CSender<CExtnMessage>, CReceiver<CExtnMessage>) {
        unbounded()
    }

    pub fn get_broker_sender(&self) -> Sender<BrokerOutput> {
        self.broker_channel.get_sender()
    }

    pub fn get_broker_receiver(&self) -> Result<Receiver<BrokerOutput>, RippleError> {
        self.broker_channel.get_receiver()
    }
}

impl Default for ChannelsState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct BootstrapState {
    pub platform_state: PlatformState,
    pub channels_state: ChannelsState,
    pub extn_state: ExtnState,
}

impl BootstrapState {
    pub fn build() -> Result<BootstrapState, RippleError> {
        let channels_state = ChannelsState::new();
        let client = RippleClient::new(channels_state.clone());
        let device_manifest = LoadDeviceManifestStep::get_manifest();
        let app_manifest_result =
            LoadAppLibraryStep::load_app_library(device_manifest.get_app_library_path())
                .expect("Valid app manifest");
        let extn_manifest = LoadExtnManifestStep::get_manifest();
        let extn_state = ExtnState::new(channels_state.clone(), extn_manifest.clone());
        let platform_state =
            PlatformState::new(extn_manifest, device_manifest, client, app_manifest_result);

        Ok(BootstrapState {
            platform_state,
            channels_state,
            extn_state,
        })
    }
}
