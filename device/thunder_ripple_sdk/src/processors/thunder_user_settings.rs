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

use jsonrpsee::core::async_trait;
use ripple_sdk::{
    api::device::{
        device_operator::{DeviceCallRequest, DeviceChannelParams, DeviceOperator},
        device_user_settings::UserSettingsRequest,
    },
    extn::{
        client::{
            extn_client::ExtnClient,
            extn_processor::{
                DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
            },
        },
        extn_client_message::{ExtnMessage, ExtnResponse},
    },
    serde_json::json,
    tokio::sync::mpsc,
    utils::error::RippleError,
};

use crate::{client::thunder_plugin::ThunderPlugin, thunder_state::ThunderState};

#[derive(Debug)]
pub struct ThunderUserSettingsRequestProcessor {
    state: ThunderState,
    streamer: DefaultExtnStreamer,
}

impl ThunderUserSettingsRequestProcessor {
    pub fn new(state: ThunderState) -> ThunderUserSettingsRequestProcessor {
        ThunderUserSettingsRequestProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }

    async fn get_audio_description(state: ThunderState, req: ExtnMessage) -> bool {
        let thunder_method = ThunderPlugin::UserSettings.method("getAudioDescription");
        let thunder_resp = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: thunder_method,
                params: None,
            })
            .await;

        println!(
            "*** _DEBUG: get_audio_description: thunder_resp={:?}",
            thunder_resp
        );

        let extn_resp = match thunder_resp.message["success"].as_bool() {
            Some(v) => ExtnResponse::Boolean(v),
            None => ExtnResponse::Error(RippleError::InvalidOutput),
        };

        println!(
            "*** _DEBUG: get_audio_description: ripple_resp={:?}",
            extn_resp
        );

        Self::respond(state.get_client(), req, extn_resp)
            .await
            .is_ok()
    }

    async fn set_audio_description(state: ThunderState, req: ExtnMessage, enabled: bool) -> bool {
        let thunder_method = ThunderPlugin::UserSettings.method("setAudioDescription");
        let thunder_resp = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: thunder_method,
                params: Some(DeviceChannelParams::Json(
                    json!({"enabled": enabled}).to_string(),
                )),
            })
            .await;

        println!(
            "*** _DEBUG: set_audio_description: thunder_resp={:?}",
            thunder_resp
        );

        let extn_resp = match thunder_resp.message["success"].as_bool() {
            Some(v) => ExtnResponse::Boolean(v),
            None => ExtnResponse::Error(RippleError::InvalidOutput),
        };

        println!(
            "*** _DEBUG: set_audio_description: ripple_resp={:?}",
            extn_resp
        );

        Self::respond(state.get_client(), req, extn_resp)
            .await
            .is_ok()
    }
}

impl ExtnStreamProcessor for ThunderUserSettingsRequestProcessor {
    type STATE = ThunderState;
    type VALUE = UserSettingsRequest;

    fn get_state(&self) -> Self::STATE {
        self.state.clone()
    }

    fn receiver(&mut self) -> mpsc::Receiver<ExtnMessage> {
        self.streamer.receiver()
    }

    fn sender(&self) -> mpsc::Sender<ExtnMessage> {
        self.streamer.sender()
    }
}

#[async_trait]
impl ExtnRequestProcessor for ThunderUserSettingsRequestProcessor {
    fn get_client(&self) -> ExtnClient {
        self.state.get_client()
    }
    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        match extracted_message {
            UserSettingsRequest::GetAudioDescription => {
                Self::get_audio_description(state.clone(), msg).await
            }
            UserSettingsRequest::SetAudioDescription(enabled) => {
                Self::set_audio_description(state.clone(), msg, enabled).await
            }
            _ => Self::respond(
                state.get_client(),
                msg,
                ExtnResponse::Error(RippleError::NotAvailable),
            )
            .await
            .is_ok(),
        }
    }
}
