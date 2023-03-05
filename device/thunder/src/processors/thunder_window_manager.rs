use ripple_sdk::{
    api::{
        apps::Dimensions,
        device::{
            device_operator::{DeviceCallRequest, DeviceChannelParams, DeviceOperator},
            device_window_manager::WindowManagerRequest,
        },
    },
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::{ExtnMessage, ExtnResponse},
    },
    log::error,
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
            client: header.client.clone(),
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
            client: header.client.clone(),
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
            client: window_name.clone(),
        };
        match req {
            WindowManagerRequest::Visibility(_, visible) => {
                ThunderVisibilityRequestParams::new(request_header, visible.clone()).into()
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

    fn receiver(&mut self) -> ripple_sdk::tokio::sync::mpsc::Receiver<ExtnMessage> {
        self.streamer.receiver()
    }

    fn sender(&self) -> ripple_sdk::tokio::sync::mpsc::Sender<ExtnMessage> {
        self.streamer.sender()
    }
}

#[async_trait]
impl ExtnRequestProcessor for ThunderWindowManagerRequestProcessor {
    async fn process_error(
        state: Self::STATE,
        msg: ExtnMessage,
        error: ripple_sdk::utils::error::RippleError,
    ) -> Option<bool> {
        if let Err(e) = Self::respond(state.get_client(), msg, ExtnResponse::Error(error)).await {
            error!("Error while processing {:?}", e);
        }
        Some(false)
    }

    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> Option<bool> {
        let device_request = DeviceCallRequest {
            method: Self::get_thunder_method(&extracted_message),
            params: Some(Self::get_thunder_params(&extracted_message)),
        };

        let response = state.get_thunder_client().call(device_request).await;
        if let Some(status) = response.message["success"].as_bool() {
            if status {
                if let Err(_) =
                    Self::respond(state.get_client(), msg.clone(), ExtnResponse::None(())).await
                {
                    error!("Sending back response for browser.destroy");
                }
                return Some(true);
            } else if let WindowManagerRequest::MoveToFront(_) = extracted_message {
                if let Err(_) =
                    Self::respond(state.get_client(), msg.clone(), ExtnResponse::None(())).await
                {
                    error!("Sending back response for browser.destroy");
                }
                return Some(true);
            }
        }
        Self::process_error(
            state,
            msg,
            ripple_sdk::utils::error::RippleError::ProcessorError,
        )
        .await
    }
}
