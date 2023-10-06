use crate::{
    firebolt::rpc::RippleRPCProvider, service::apps::app_events::AppEvents,
    state::platform_state::PlatformState, utils::rpc_utils::rpc_err,
};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    RpcModule,
};
use ripple_sdk::api::{
    device::device_events::{
        DeviceEvent, DeviceEventCallback, DeviceEventRequest, HDMI_CONNECTION_CHANGED,
    },
    firebolt::{
        fb_general::{ListenRequest, ListenerResponse},
        panel::fb_hdmi::{
            GetAvailableInputsResponse, HdmiOperation, HdmiSelectOperationRequest,
            HdmiSelectOperationResponse,
        },
    },
    gateway::rpc_gateway_api::CallContext,
};
use ripple_sdk::{
    api::device::device_events::AUTO_LOW_LATENCY_MODE_SIGNAL_CHANGED, log::error, serde_json,
};
use ripple_sdk::{
    api::device::panel::device_hdmi::HdmiRequest,
    extn::extn_client_message::{ExtnPayload, ExtnResponse},
};

#[rpc(server)]
pub trait Hdmi {
    #[method(name = "HDMIInput.select")]
    async fn select_hdmi_operation(
        &self,
        ctx: CallContext,
        request: HdmiSelectOperationRequest,
    ) -> RpcResult<HdmiSelectOperationResponse>;

    #[method(name = "HDMIInput.ports")]
    async fn get_available_inputs(&self, ctx: CallContext)
        -> RpcResult<GetAvailableInputsResponse>;

    #[method(name = "HDMIInput.onConnectionChanged")]
    async fn on_hdmi_connection_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "HDMIInput.onAutoLowLatencyModeSignalChanged")]
    async fn on_auto_low_latency_mode_signal_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
}

#[derive(Debug)]
pub struct HdmiImpl {
    pub state: PlatformState,
}

#[async_trait]
impl HdmiServer for HdmiImpl {
    async fn select_hdmi_operation(
        &self,
        _ctx: CallContext,
        request: HdmiSelectOperationRequest,
    ) -> RpcResult<HdmiSelectOperationResponse> {
        if let HdmiOperation::Start = request.operation {
            if let Ok(response) = self
                .state
                .get_client()
                .send_extn_request(HdmiRequest::StartHdmiInput(request))
                .await
            {
                if let ExtnPayload::Response(ExtnResponse::Value(value)) = response.payload {
                    if let Ok(res) = serde_json::from_value::<HdmiSelectOperationResponse>(value) {
                        return Ok(res);
                    }
                }
            }

            Err(rpc_err("FB error response TBD"))
        } else {
            if let Ok(response) = self
                .state
                .get_client()
                .send_extn_request(HdmiRequest::StopHdmiInput(request))
                .await
            {
                if let ExtnPayload::Response(ExtnResponse::Value(value)) = response.payload {
                    if let Ok(res) = serde_json::from_value::<HdmiSelectOperationResponse>(value) {
                        return Ok(res);
                    }
                }
            }

            Err(rpc_err("FB error response TBD"))
        }
    }

    async fn get_available_inputs(
        &self,
        _ctx: CallContext,
    ) -> RpcResult<GetAvailableInputsResponse> {
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(HdmiRequest::GetAvailableInputs)
            .await
        {
            if let ExtnPayload::Response(ExtnResponse::Value(value)) = response.payload {
                if let Ok(res) = serde_json::from_value::<GetAvailableInputsResponse>(value) {
                    return Ok(res);
                }
            }
        }

        Err(rpc_err("FB error response TBD"))
    }
    async fn on_hdmi_connection_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;

        AppEvents::add_listener(
            &self.state,
            HDMI_CONNECTION_CHANGED.to_string(),
            ctx.clone(),
            request,
        );

        if self
            .state
            .get_client()
            .send_extn_request(DeviceEventRequest {
                event: DeviceEvent::HdmiConnectionChanged,
                id: ctx.app_id,
                subscribe: listen,
                callback_type: DeviceEventCallback::FireboltAppEvent,
            })
            .await
            .is_err()
        {
            error!("Error while registration");
        }

        Ok(ListenerResponse {
            listening: listen,
            event: HDMI_CONNECTION_CHANGED.to_string(),
        })
    }
    async fn on_auto_low_latency_mode_signal_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;

        AppEvents::add_listener(
            &self.state,
            AUTO_LOW_LATENCY_MODE_SIGNAL_CHANGED.to_string(),
            ctx.clone(),
            request,
        );

        if self
            .state
            .get_client()
            .send_extn_request(DeviceEventRequest {
                event: DeviceEvent::AutoLowLatencyModeSignalChanged,
                id: ctx.app_id,
                subscribe: listen,
                callback_type: DeviceEventCallback::FireboltAppEvent,
            })
            .await
            .is_err()
        {
            error!("Error while registration");
        }

        Ok(ListenerResponse {
            listening: listen,
            event: AUTO_LOW_LATENCY_MODE_SIGNAL_CHANGED.to_string(),
        })
    }
}

pub struct HdmiRPCProvider;
impl RippleRPCProvider<HdmiImpl> for HdmiRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<HdmiImpl> {
        (HdmiImpl { state }).into_rpc()
    }
}
