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

use crate::ripple_sdk::{
    api::{
        apps::Dimensions,
        device::{
            device_operator::{DeviceCallRequest, DeviceChannelParams, DeviceOperator},
            device_window_manager::WindowManagerRequest,
        },
    },
    async_trait::async_trait,
    extn::{
        client::{
            extn_client::ExtnClient,
            extn_processor::{
                DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
            },
        },
        extn_client_message::{ExtnMessage, ExtnResponse},
    },
    serde_json,
    tokio::sync::mpsc,
    utils::error::RippleError,
};
use serde::{Deserialize, Serialize};

use crate::{client::thunder_plugin::ThunderPlugin, thunder_state::ThunderState};

#[derive(Debug)]
pub struct ThunderWindowManagerRequestProcessor {
    state: ThunderState,
    streamer: DefaultExtnStreamer,
}

#[derive(Debug, Serialize, Deserialize)]
struct WindowManagerRequestHeader {
    callsign: String,
    client: String,
}

#[derive(Serialize, Deserialize)]
struct ThunderVisibilityRequestParams {
    pub callsign: String,
    pub client: String,
    pub visible: bool,
}

impl ThunderVisibilityRequestParams {
    fn new(header: WindowManagerRequestHeader, visible: bool) -> ThunderVisibilityRequestParams {
        ThunderVisibilityRequestParams {
            callsign: header.callsign.clone(),
            client: header.client,
            visible,
        }
    }
}

impl From<ThunderVisibilityRequestParams> for DeviceChannelParams {
    fn from(params: ThunderVisibilityRequestParams) -> Self {
        DeviceChannelParams::Json(serde_json::to_string(&params).unwrap())
    }
}

#[derive(Serialize, Deserialize)]
struct ThunderDimensionsRequestParams {
    pub callsign: String,
    pub client: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl ThunderDimensionsRequestParams {
    fn new(
        header: WindowManagerRequestHeader,
        dimensions: Dimensions,
    ) -> ThunderDimensionsRequestParams {
        ThunderDimensionsRequestParams {
            callsign: header.callsign.clone(),
            client: header.client,
            x: dimensions.x,
            y: dimensions.y,
            w: dimensions.w,
            h: dimensions.h,
        }
    }
}

impl From<ThunderDimensionsRequestParams> for DeviceChannelParams {
    fn from(params: ThunderDimensionsRequestParams) -> Self {
        DeviceChannelParams::Json(serde_json::to_string(&params).unwrap())
    }
}

impl ThunderWindowManagerRequestProcessor {
    pub fn new(state: ThunderState) -> ThunderWindowManagerRequestProcessor {
        ThunderWindowManagerRequestProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }

    fn get_thunder_method(req: &WindowManagerRequest) -> String {
        match req {
            WindowManagerRequest::Visibility(..) => ThunderPlugin::RDKShell.method("setVisibility"),
            WindowManagerRequest::MoveToFront(..) => ThunderPlugin::RDKShell.method("moveToFront"),
            WindowManagerRequest::MoveToBack(..) => ThunderPlugin::RDKShell.method("moveToBack"),
            WindowManagerRequest::Focus(..) => ThunderPlugin::RDKShell.method("setFocus"),
            WindowManagerRequest::Dimensions(..) => ThunderPlugin::RDKShell.method("setBounds"),
        }
    }

    fn get_thunder_params(req: &WindowManagerRequest) -> DeviceChannelParams {
        let window_name = req.window_name();
        let request_header = WindowManagerRequestHeader {
            callsign: window_name.clone(),
            client: window_name,
        };
        match req {
            WindowManagerRequest::Visibility(_, visible) => {
                ThunderVisibilityRequestParams::new(request_header, *visible).into()
            }
            WindowManagerRequest::Dimensions(_, dimensions) => {
                ThunderDimensionsRequestParams::new(request_header, dimensions.clone()).into()
            }
            _ => DeviceChannelParams::Json(serde_json::to_string(&request_header).unwrap()),
        }
    }
}

impl ExtnStreamProcessor for ThunderWindowManagerRequestProcessor {
    type STATE = ThunderState;
    type VALUE = WindowManagerRequest;

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
impl ExtnRequestProcessor for ThunderWindowManagerRequestProcessor {
    fn get_client(&self) -> ExtnClient {
        self.state.get_client()
    }

    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        let device_request = DeviceCallRequest {
            method: Self::get_thunder_method(&extracted_message),
            params: Some(Self::get_thunder_params(&extracted_message)),
        };

        let response = state.get_thunder_client().call(device_request).await;
        if let Some(status) = response.message["success"].as_bool() {
            if status {
                return Self::respond(state.get_client(), msg.clone(), ExtnResponse::None(()))
                    .await
                    .is_ok();
            } else if let WindowManagerRequest::MoveToFront(_) = extracted_message {
                // This move to front api will return false when the window is already in front which doesnt necessarily
                // term it as an error as the intention of bringing the window to front is still successful
                return Self::respond(state.get_client(), msg.clone(), ExtnResponse::None(()))
                    .await
                    .is_ok();
            }
        }
        Self::handle_error(state.get_client(), msg, RippleError::ProcessorError).await
    }
}
