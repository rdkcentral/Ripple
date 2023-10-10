use crate::{
    client::thunder_plugin::ThunderPlugin,
    events::thunder_event_processor::{
        ThunderEventHandler, ThunderEventHandlerProvider, ThunderEventMessage,
    },
    ripple_sdk::{
        api::{
            apps::{AppEvent, AppEventRequest},
            device::{
                device_events::DeviceEventCallback,
                device_operator::{DeviceCallRequest, DeviceChannelParams, DeviceOperator},
                panel::device_hdmi::HdmiRequest,
            },
            firebolt::panel::fb_hdmi::{
                AutoLowLatencyModeSignalChangedInfo, GetAvailableInputsResponse,
                HdmiConnectionChangedInfo, HdmiOperation, HdmiSelectOperationRequest,
            },
        },
        async_trait::async_trait,
        extn::{
            client::extn_client::ExtnClient,
            client::extn_processor::{
                DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
            },
            extn_client_message::{ExtnEvent, ExtnMessage, ExtnResponse},
        },
        serde_json,
        tokio::sync::mpsc,
        utils::error::RippleError,
    },
    thunder_state::ThunderState,
};

use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct ThunderHdmiRequestProcessor {
    state: ThunderState,
    streamer: DefaultExtnStreamer,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AVInputGetInputDevicesParams {
    type_of_input: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AVInputStartHdmiOperationParams {
    port_id: u32,
    type_of_input: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AVInputStopHdmiOperationParams {
    type_of_input: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum AVInputHdmiOperation {
    GetInputDevices(AVInputGetInputDevicesParams),
    StartHdmiInputOperation(AVInputStartHdmiOperationParams),
    StopHdmiInputOperation(AVInputStopHdmiOperationParams),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ThunderHdmiConnectionChanged {
    pub id: u32,
    pub locator: String,
    pub connected: bool,
}

impl From<ThunderHdmiConnectionChanged> for HdmiConnectionChangedInfo {
    fn from(value: ThunderHdmiConnectionChanged) -> Self {
        HdmiConnectionChangedInfo {
            port: format!("HDMI{}", value.id),
            connected: value.connected,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ThunderAutoLowLatencyModeSignalChanged {
    pub id: u32,
    pub game_feature: String,
    pub mode: bool,
}

impl From<ThunderAutoLowLatencyModeSignalChanged> for AutoLowLatencyModeSignalChangedInfo {
    fn from(value: ThunderAutoLowLatencyModeSignalChanged) -> Self {
        AutoLowLatencyModeSignalChangedInfo {
            port: format!("HDMI{}", value.id),
            auto_low_latency_mode_signalled: value.mode,
        }
    }
}

impl ThunderHdmiRequestProcessor {
    pub fn new(state: ThunderState) -> ThunderHdmiRequestProcessor {
        ThunderHdmiRequestProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }

    async fn get_available_inputs(state: ThunderState, req: ExtnMessage) -> bool {
        let params = AVInputGetInputDevicesParams {
            type_of_input: "HDMI".to_owned(),
        };
        Self::make_av_input_thunder_call(
            state,
            AVInputHdmiOperation::GetInputDevices(params),
            req,
            "getAvailableDevices",
        )
        .await
    }

    async fn select_hdmi_operation(
        state: ThunderState,
        select_hdmi_operation_request: HdmiSelectOperationRequest,
        req: ExtnMessage,
    ) -> bool {
        if let HdmiOperation::Start = select_hdmi_operation_request.operation {
            let params: AVInputStartHdmiOperationParams = AVInputStartHdmiOperationParams {
                port_id: select_hdmi_operation_request.port.parse::<u32>().unwrap(),
                type_of_input: "HDMI".to_owned(),
            };
            Self::make_av_input_thunder_call(
                state,
                AVInputHdmiOperation::StartHdmiInputOperation(params),
                req,
                "startInput",
            )
            .await
        } else {
            let params: AVInputStopHdmiOperationParams = AVInputStopHdmiOperationParams {
                type_of_input: "HDMI".to_owned(),
            };
            Self::make_av_input_thunder_call(
                state,
                AVInputHdmiOperation::StopHdmiInputOperation(params),
                req,
                "stopInput",
            )
            .await
        }
    }
    async fn make_av_input_thunder_call(
        state: ThunderState,
        params: AVInputHdmiOperation,
        req: ExtnMessage,
        method_name: &str,
    ) -> bool {
        let response = state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: ThunderPlugin::AVInput.method(method_name),
                params: serde_json::to_string(&params)
                    .map(DeviceChannelParams::Json)
                    .ok(),
            })
            .await;

        let response =
            serde_json::from_value::<GetAvailableInputsResponse>(response.message.clone())
                .map(|_| ExtnResponse::Value(response.message))
                .unwrap_or(ExtnResponse::Error(RippleError::InvalidOutput));

        Self::respond(state.get_client(), req, response)
            .await
            .is_ok()
    }
}

impl ExtnStreamProcessor for ThunderHdmiRequestProcessor {
    type STATE = ThunderState;
    type VALUE = HdmiRequest;

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
impl ExtnRequestProcessor for ThunderHdmiRequestProcessor {
    fn get_client(&self) -> ExtnClient {
        self.state.get_client()
    }

    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        match extracted_message {
            HdmiRequest::GetAvailableInputs => Self::get_available_inputs(state.clone(), msg).await,
            HdmiRequest::HdmiSelectOperation(hdmi_select_operation_request) => {
                Self::select_hdmi_operation(state.clone(), hdmi_select_operation_request, msg).await
            }
        }
    }
}

pub struct HdmiConnectionHandler;

impl HdmiConnectionHandler {
    pub fn handle(
        state: ThunderState,
        value: ThunderEventMessage,
        callback_type: DeviceEventCallback,
    ) {
        if let ThunderEventMessage::HdmiConnection(v) = value {
            for hdmi in v {
                if let Ok(v) = Self::get_extn_event(hdmi.into(), callback_type.clone()) {
                    ThunderEventHandler::callback_device_event(
                        state.clone(),
                        Self::get_mapped_event(),
                        v,
                    )
                }
            }
        }
    }
    pub fn is_valid(value: ThunderEventMessage) -> bool {
        if let ThunderEventMessage::HdmiConnection(_) = value {
            return true;
        }
        false
    }
}

impl ThunderEventHandlerProvider for HdmiConnectionHandler {
    type EVENT = HdmiConnectionChangedInfo;
    fn provide(id: String, callback_type: DeviceEventCallback) -> ThunderEventHandler {
        ThunderEventHandler {
            request: Self::get_device_request(),
            handle: Self::handle,
            is_valid: Self::is_valid,
            listeners: vec![id],
            id: Self::get_mapped_event(),
            callback_type,
        }
    }

    fn event_name() -> String {
        "onDevicesChanged".into()
    }

    fn get_mapped_event() -> String {
        "onConnectionChanged".into()
    }

    fn module() -> String {
        ThunderPlugin::AVInput.callsign_string()
    }

    fn get_extn_event(
        r: Self::EVENT,
        callback_type: DeviceEventCallback,
    ) -> Result<ExtnEvent, RippleError> {
        let result = serde_json::to_value(r.clone()).unwrap();
        match callback_type {
            DeviceEventCallback::FireboltAppEvent => {
                Ok(ExtnEvent::AppEvent(AppEventRequest::Emit(AppEvent {
                    event_name: Self::get_mapped_event(),
                    context: None,
                    result,
                    app_id: None,
                })))
            }
            DeviceEventCallback::ExtnEvent => Ok(ExtnEvent::HdmiConnectionChanged(r)),
        }
    }
}

pub struct AutoLowLatencyModeSignalHandler;

impl AutoLowLatencyModeSignalHandler {
    pub fn handle(
        state: ThunderState,
        value: ThunderEventMessage,
        callback_type: DeviceEventCallback,
    ) {
        if let ThunderEventMessage::AutoLowLatencyModeSignal(v) = value {
            if let Ok(v) = Self::get_extn_event(v.into(), callback_type) {
                ThunderEventHandler::callback_device_event(state, Self::get_mapped_event(), v)
            }
        }
    }
    pub fn is_valid(value: ThunderEventMessage) -> bool {
        if let ThunderEventMessage::AutoLowLatencyModeSignal(_) = value {
            return true;
        }
        false
    }
}

impl ThunderEventHandlerProvider for AutoLowLatencyModeSignalHandler {
    type EVENT = AutoLowLatencyModeSignalChangedInfo;
    fn provide(id: String, callback_type: DeviceEventCallback) -> ThunderEventHandler {
        ThunderEventHandler {
            request: Self::get_device_request(),
            handle: Self::handle,
            is_valid: Self::is_valid,
            listeners: vec![id],
            id: Self::get_mapped_event(),
            callback_type,
        }
    }

    fn event_name() -> String {
        "gameFeatureStatusUpdate".into()
    }

    fn get_mapped_event() -> String {
        "onAutoLowLatencyModeSignalChanged".into()
    }

    fn module() -> String {
        ThunderPlugin::AVInput.callsign_string()
    }

    fn get_extn_event(
        r: Self::EVENT,
        callback_type: DeviceEventCallback,
    ) -> Result<ExtnEvent, RippleError> {
        let result = serde_json::to_value(r.clone()).unwrap();
        match callback_type {
            DeviceEventCallback::FireboltAppEvent => {
                Ok(ExtnEvent::AppEvent(AppEventRequest::Emit(AppEvent {
                    event_name: Self::get_mapped_event(),
                    context: None,
                    result,
                    app_id: None,
                })))
            }
            DeviceEventCallback::ExtnEvent => Ok(ExtnEvent::AutoLowLatencyModeSignalChanged(r)),
        }
    }
}
