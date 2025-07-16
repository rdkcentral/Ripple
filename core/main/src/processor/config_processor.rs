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
    api::config::{Config, ConfigResponse, LauncherConfig, RfcRequest},
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::{ExtnMessage, ExtnPayload, ExtnPayloadProvider, ExtnResponse},
    },
    serde_json,
    tokio::sync::mpsc::{Receiver as MReceiver, Sender as MSender},
};

use crate::state::platform_state::PlatformState;

/// Supports processing of [Config] request from extensions and also
/// internal services.
#[derive(Debug)]
pub struct ConfigRequestProcessor {
    state: PlatformState,
    streamer: DefaultExtnStreamer,
}

impl ConfigRequestProcessor {
    pub fn new(state: PlatformState) -> ConfigRequestProcessor {
        ConfigRequestProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for ConfigRequestProcessor {
    type STATE = PlatformState;
    type VALUE = Config;
    fn get_state(&self) -> Self::STATE {
        self.state.clone()
    }

    fn sender(&self) -> MSender<ExtnMessage> {
        self.streamer.sender()
    }

    fn receiver(&mut self) -> MReceiver<ExtnMessage> {
        self.streamer.receiver()
    }
}

#[async_trait]
impl ExtnRequestProcessor for ConfigRequestProcessor {
    fn get_client(&self) -> ripple_sdk::extn::client::extn_client::ExtnClient {
        self.state.get_client().get_extn_client()
    }

    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        let device_manifest = state.get_device_manifest();

        let config_request = extracted_message;
        let response = match config_request {
            Config::RippleFeatures => ExtnResponse::Value(
                serde_json::to_value(device_manifest.configuration.features.clone())
                    .unwrap_or_default(),
            ),
            Config::PlatformParameters => {
                ExtnResponse::Value(device_manifest.configuration.platform_parameters.clone())
            }
            Config::LauncherConfig => {
                let config = LauncherConfig {
                    lifecycle_policy: device_manifest.get_lifecycle_policy(),
                    retention_policy: device_manifest.get_retention_policy(),
                    app_library_state: state.app_library_state.clone(),
                };
                if let ExtnPayload::Response(r) = config.get_extn_payload() {
                    r
                } else {
                    ExtnResponse::Error(ripple_sdk::utils::error::RippleError::ProcessorError)
                }
            }
            Config::DefaultLanguage => ExtnResponse::Config(ConfigResponse::String(
                device_manifest.configuration.default_values.language,
            )),
            Config::SavedDir => {
                ExtnResponse::String(device_manifest.configuration.saved_dir.clone())
            }
            Config::FormFactor => ExtnResponse::String(device_manifest.get_form_factor()),
            Config::DistributorExperienceId => {
                ExtnResponse::String(device_manifest.get_distributor_experience_id())
            }
            Config::DistributorServices => {
                if let Some(v) = &device_manifest.configuration.distributor_services {
                    ExtnResponse::Value(v.clone())
                } else {
                    ExtnResponse::None(())
                }
            }
            Config::IdSalt => {
                if let Some(v) = &device_manifest.configuration.distribution_id_salt {
                    ExtnResponse::Config(ConfigResponse::IdSalt(v.clone()))
                } else {
                    ExtnResponse::None(())
                }
            }
            Config::SupportsDistributorSession => {
                ExtnResponse::Boolean(state.supports_distributor_session())
            }
            Config::DefaultValues => ExtnResponse::Value(
                serde_json::to_value(device_manifest.configuration.default_values.clone())
                    .unwrap_or_default(),
            ),
            Config::Firebolt => ExtnResponse::Value(
                serde_json::to_value(&*state.open_rpc_state.get_open_rpc().clone())
                    .unwrap_or_default(),
            ),
            Config::RFC(flag) => {
                let mut resp =
                    ExtnResponse::Error(ripple_sdk::utils::error::RippleError::InvalidAccess);
                if state.supports_rfc() {
                    if let Ok(v) = state
                        .get_client()
                        .send_extn_request(RfcRequest { flag })
                        .await
                    {
                        if let Some(ExtnResponse::Value(v)) = v.payload.extract() {
                            resp = ExtnResponse::Value(v);
                        }
                    }
                }
                resp
            }
            _ => ExtnResponse::Error(ripple_sdk::utils::error::RippleError::InvalidInput),
        };
        Self::respond(state.get_client().get_extn_client(), msg, response)
            .await
            .is_ok()
    }
}
