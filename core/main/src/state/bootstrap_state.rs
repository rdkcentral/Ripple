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
    log::{info, warn},
    tokio::sync::mpsc::{self, Receiver, Sender},
    utils::error::RippleError,
};

use crate::{
    bootstrap::manifest::{
        apps::LoadAppLibraryStep, device::LoadDeviceManifestStep, extn::LoadExtnManifestStep,
    },
    firebolt::firebolt_gateway::FireboltGatewayCommand,
    service::extn::ripple_client::RippleClient,
};

use super::{extn_state::ExtnState, platform_state::PlatformState};

use env_file_reader::read_file;
#[derive(Debug, Clone)]
pub struct ChannelsState {
    gateway_channel: TransientChannel<FireboltGatewayCommand>,
    app_req_channel: TransientChannel<AppRequest>,
    extn_sender: CSender<CExtnMessage>,
    extn_receiver: CReceiver<CExtnMessage>,
}

impl ChannelsState {
    pub fn new() -> ChannelsState {
        let (gateway_tx, gateway_tr) = mpsc::channel(32);
        let (app_req_tx, app_req_tr) = mpsc::channel(32);
        let (ctx, ctr) = unbounded();

        ChannelsState {
            gateway_channel: TransientChannel::new(gateway_tx, gateway_tr),
            app_req_channel: TransientChannel::new(app_req_tx, app_req_tr),
            extn_sender: ctx,
            extn_receiver: ctr,
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
        let platform_state = PlatformState::new(
            extn_manifest,
            device_manifest,
            client,
            app_manifest_result,
            ripple_version_from_etc(),
        );

        fn ripple_version_from_etc() -> Option<String> {
            /*
            read /etc/rippleversion
            */
            static RIPPLE_VER_FILE_DEFAULT: &str = "/etc/rippleversion.txt";
            static RIPPLE_VER_VAR_NAME_DEFAULT: &str = "RIPPLE_VER";
            let version_file_name = std::env::var("RIPPLE_VERSIONS_FILE")
                .unwrap_or(RIPPLE_VER_FILE_DEFAULT.to_string());
            let version_var_name = std::env::var("RIPPLE_VERSIONS_VAR")
                .unwrap_or(RIPPLE_VER_VAR_NAME_DEFAULT.to_string());

            match read_file(version_file_name.clone()) {
                Ok(env_vars) => {
                    if let Some(version) = env_vars.get(&version_var_name) {
                        info!(
                            "Printing ripple version from rippleversion.txt {:?}",
                            version.clone()
                        );
                        return Some(version.clone());
                    }
                }
                Err(err) => {
                    warn!(
                        "error reading versions from {}, err={:?}",
                        version_file_name, err
                    );
                }
            }
            warn!("error reading versions from {}", version_file_name,);
            None
        }
        Ok(BootstrapState {
            platform_state,
            channels_state,
            extn_state,
        })
    }
}
