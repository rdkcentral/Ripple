use super::accessibility::SetStringProperty;
use crate::{
    api::rpc::rpc_gateway::{CallContext, RPCProvider},
    apps::{
        app_events::{AppEvents, ListenRequest, ListenerResponse},
        app_mgr::{AppError, AppManagerResponse, AppRequest},
    },
    helpers::{
        crypto_util::{extract_app_ref, salt_using_app_scope},
        error_util::{rpc_await_oneshot, CAPABILITY_NOT_AVAILABLE},
        ripple_helper::{IRippleHelper, RippleHelper, RippleHelperFactory, RippleHelperType},
        session_util::{get_distributor_session, get_distributor_session_from_platform_state},
    },
    managers::{
        capability_manager::{
            CapClassifiedRequest, FireboltCap, IGetLoadedCaps, RippleHandlerCaps,
        },
        config_manager::ConfigRequest,
        event::event_administrator::{DabEventAdministrator, DabEventHandler},
        storage::{
            storage_manager::StorageManager,
            storage_property::{
                StorageProperty, EVENT_DEVICE_DEVICE_NAME_CHANGED, EVENT_DEVICE_NAME_CHANGED,
            },
        },
    },
    platform_state::PlatformState,
};
use dab::core::{
    message::{DabEvent, DabRequestPayload, DabResponsePayload, DabSubscribeMessage},
    model::{
        device::{
            AudioProfile, DeviceEvent, DeviceRequest, FireboltSemanticVersion, HDCPStatus,
            HdcpProfile, HdrProfile, NetworkResponse, EVENT_ACTIVE_INPUT_CHANGED,
            EVENT_NETWORK_CHANGED, EVENT_RESOLUTION_CHANGED,
        },
        distributor::{DistributorRequest, DistributorSession},
    },
    utils::get_dimension_from_resolution,
};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    types::error::CallError,
    RpcModule,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use tokio::sync::oneshot;
use tracing::{error, instrument};

const HDCP_CHANGED_EVENT: &'static str = "device.onHdcpChanged";
const HDR_CHANGED_EVENT: &'static str = "device.onHdrChanged";
const SCREEN_RESOLUTION_CHANGED_EVENT: &'static str = "device.onScreenResolutionChanged";
const VIDEO_RESOLUTION_CHANGED_EVENT: &'static str = "device.onVideoResolutionChanged";
const NETWORK_CHANGED_EVENT: &'static str = "device.onNetworkChanged";

include!(concat!(env!("OUT_DIR"), "/version.rs"));

#[derive(Serialize, Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionRequest {
    account_id: String,
    device_id: String,
    distributor_id: Option<String>,
}

impl ProvisionRequest {
    fn get_session(self) -> DistributorSession {
        DistributorSession {
            id: None,
            token: None,
            account_id: Some(self.account_id.clone()),
            device_id: Some(self.device_id.clone()),
        }
    }
}

#[rpc(server)]
pub trait Device {
    #[method(name = "device.name")]
    async fn name(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "device.setName")]
    async fn set_name(
        &self,
        ctx: CallContext,
        _setname_request: SetStringProperty,
    ) -> RpcResult<()>;
    #[method(name = "device.id")]
    async fn id(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "device.uid")]
    async fn uid(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "device.onNameChanged")]
    async fn on_name_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "device.onDeviceNameChanged")]
    async fn on_device_name_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "device.model")]
    async fn model(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "device.sku")]
    async fn sku(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "device.hdcp")]
    async fn hdcp(&self, ctx: CallContext) -> RpcResult<HashMap<HdcpProfile, bool>>;
    #[method(name = "device.onHdcpChanged")]
    async fn on_hdcp_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "device.hdr")]
    async fn hdr(&self, ctx: CallContext) -> RpcResult<HashMap<HdrProfile, bool>>;
    #[method(name = "device.onHdrChanged")]
    async fn on_hdr_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "device.screenResolution")]
    async fn screen_resolution(&self, ctx: CallContext) -> RpcResult<Vec<i32>>;
    #[method(name = "device.onScreenResolutionChanged")]
    async fn on_screen_resolution_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "device.videoResolution")]
    async fn video_resolution(&self, ctx: CallContext) -> RpcResult<Vec<i32>>;
    #[method(name = "device.onVideoResolutionChanged")]
    async fn on_video_resolution_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "device.type")]
    async fn typ(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "device.audio")]
    async fn audio(&self, ctx: CallContext) -> RpcResult<HashMap<AudioProfile, bool>>;
    #[method(name = "device.onAudioChanged")]
    async fn on_audio_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "device.network")]
    async fn network(&self, ctx: CallContext) -> RpcResult<NetworkResponse>;
    #[method(name = "device.onNetworkChanged")]
    async fn on_network_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "device.provision")]
    async fn provision(
        &self,
        ctx: CallContext,
        provision_request: ProvisionRequest,
    ) -> RpcResult<()>;
    #[method(name = "device.distributor")]
    async fn distributor(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "device.make")]
    async fn make(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "device.platform")]
    async fn platform(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "device.version")]
    async fn version(&self, ctx: CallContext) -> RpcResult<DeviceVersionResponse>;
}

#[derive(Clone)]
struct HDCPEventHandler {}

#[async_trait]
impl DabEventHandler for HDCPEventHandler {
    async fn handle_dab_event(
        &self,
        ps: &PlatformState,
        cur_value: &mut Option<Value>,
        _dab_event: &DabEvent,
    ) {
        let resp = ps
            .services
            .send_dab(DabRequestPayload::Device(DeviceRequest::HdcpSupport))
            .await;

        match resp {
            Ok(dab_payload) => match dab_payload {
                DabResponsePayload::HdcpSupportResponse(value) => {
                    let new_val = json!(value.clone());
                    if !cur_value
                        .clone()
                        .unwrap()
                        .to_string()
                        .eq(&new_val.to_string())
                    {
                        AppEvents::emit(&ps, &HDCP_CHANGED_EVENT.to_string(), &json!(value)).await;
                        *cur_value = Some(new_val);
                    }
                }
                _ => {}
            },
            Err(_e) => {}
        }
    }

    fn generate_dab_event_subscribe_request(
        &self,
        ctx: CallContext,
        listen: bool,
    ) -> DabRequestPayload {
        let subscribe_message = DabSubscribeMessage {
            subscribe: listen,
            context: Some(serde_json::to_string(&ctx).unwrap()),
        };

        DabRequestPayload::Device(DeviceRequest::OnActiveInputChanged(subscribe_message))
    }

    fn get_mapped_dab_event_name(&self) -> &str {
        EVENT_ACTIVE_INPUT_CHANGED
    }

    fn dab_event_handler_clone(&self) -> Box<dyn DabEventHandler + Send + Sync> {
        Box::new(self.clone())
    }
}

#[derive(Clone)]
struct HDREventHandler {}

#[async_trait]
impl DabEventHandler for HDREventHandler {
    async fn handle_dab_event(
        &self,
        ps: &PlatformState,
        cur_value: &mut Option<Value>,
        _dab_event: &DabEvent,
    ) {
        let resp = ps
            .services
            .send_dab(DabRequestPayload::Device(DeviceRequest::Hdr))
            .await;

        match resp {
            Ok(dab_payload) => match dab_payload {
                DabResponsePayload::HdrResponse(value) => {
                    let new_val = json!(value.clone());
                    if !cur_value
                        .clone()
                        .unwrap()
                        .to_string()
                        .eq(&new_val.to_string())
                    {
                        AppEvents::emit(&ps, &HDR_CHANGED_EVENT.to_string(), &json!(value)).await;
                        *cur_value = Some(new_val);
                    }
                }
                _ => {}
            },
            Err(_e) => {}
        }
    }

    fn generate_dab_event_subscribe_request(
        &self,
        ctx: CallContext,
        listen: bool,
    ) -> DabRequestPayload {
        let subscribe_message = DabSubscribeMessage {
            subscribe: listen,
            context: Some(serde_json::to_string(&ctx).unwrap()),
        };

        DabRequestPayload::Device(DeviceRequest::OnActiveInputChanged(subscribe_message))
    }

    fn get_mapped_dab_event_name(&self) -> &str {
        EVENT_ACTIVE_INPUT_CHANGED
    }

    fn dab_event_handler_clone(&self) -> Box<dyn DabEventHandler + Send + Sync> {
        Box::new(self.clone())
    }
}

#[derive(Clone)]
struct NetworkEventHandler {}

#[async_trait]
impl DabEventHandler for NetworkEventHandler {
    async fn handle_dab_event(
        &self,
        ps: &PlatformState,
        _cur_value: &mut Option<Value>,
        dab_event: &DabEvent,
    ) {
        match dab_event {
            DabEvent::Device(DeviceEvent::NetworkChangedEvent(network_response)) => {
                AppEvents::emit(
                    &ps,
                    &NETWORK_CHANGED_EVENT.to_string(),
                    &serde_json::to_value(network_response).unwrap(),
                )
                .await;
            }
            _ => {
                error!("Invalid dab_event received");
            }
        }
    }

    fn generate_dab_event_subscribe_request(
        &self,
        ctx: CallContext,
        listen: bool,
    ) -> DabRequestPayload {
        let subscribe_message = DabSubscribeMessage {
            subscribe: listen,
            context: Some(serde_json::to_string(&ctx).unwrap()),
        };

        DabRequestPayload::Device(DeviceRequest::OnNetworkChanged(subscribe_message))
    }

    fn get_mapped_dab_event_name(&self) -> &str {
        EVENT_NETWORK_CHANGED
    }

    fn dab_event_handler_clone(&self) -> Box<dyn DabEventHandler + Send + Sync> {
        Box::new(self.clone())
    }
}
#[derive(Clone)]
struct ScreenResolutionEventHandler {}

#[async_trait]
impl DabEventHandler for ScreenResolutionEventHandler {
    async fn handle_dab_event(
        &self,
        ps: &PlatformState,
        _cur_value: &mut Option<Value>,
        dab_event: &DabEvent,
    ) {
        match dab_event {
            DabEvent::Device(DeviceEvent::ResolutionChangedEvent(event_data)) => {
                let screen_resolution = vec![event_data.width, event_data.height];
                AppEvents::emit(
                    &ps,
                    &SCREEN_RESOLUTION_CHANGED_EVENT.to_string(),
                    &serde_json::to_value(screen_resolution).unwrap(),
                )
                .await;
            }

            _ => {
                error!("Invalid dab_event received");
            }
        }
    }

    fn generate_dab_event_subscribe_request(
        &self,
        ctx: CallContext,
        listen: bool,
    ) -> DabRequestPayload {
        let subscribe_message = DabSubscribeMessage {
            subscribe: listen,
            context: Some(serde_json::to_string(&ctx).unwrap()),
        };

        DabRequestPayload::Device(DeviceRequest::OnResolutionChanged(subscribe_message))
    }

    fn get_mapped_dab_event_name(&self) -> &str {
        EVENT_RESOLUTION_CHANGED
    }

    fn dab_event_handler_clone(&self) -> Box<dyn DabEventHandler + Send + Sync> {
        Box::new(self.clone())
    }
}
#[derive(Clone)]
struct VideoResolutionEventHandler {}

#[async_trait]
impl DabEventHandler for VideoResolutionEventHandler {
    async fn handle_dab_event(
        &self,
        ps: &PlatformState,
        _cur_value: &mut Option<Value>,
        dab_event: &DabEvent,
    ) {
        match dab_event {
            DabEvent::Device(DeviceEvent::ResolutionChangedEvent(event_data)) => {
                let video_resolution = get_dimension_from_resolution(&event_data.resolution);
                AppEvents::emit(
                    &ps,
                    &VIDEO_RESOLUTION_CHANGED_EVENT.to_string(),
                    &serde_json::to_value(video_resolution).unwrap(),
                )
                .await;
            }

            _ => {
                error!("Invalid dab_event received");
            }
        }
    }

    fn generate_dab_event_subscribe_request(
        &self,
        ctx: CallContext,
        listen: bool,
    ) -> DabRequestPayload {
        let subscribe_message = DabSubscribeMessage {
            subscribe: listen,
            context: Some(serde_json::to_string(&ctx).unwrap()),
        };

        DabRequestPayload::Device(DeviceRequest::OnResolutionChanged(subscribe_message))
    }

    fn get_mapped_dab_event_name(&self) -> &str {
        EVENT_RESOLUTION_CHANGED
    }

    fn dab_event_handler_clone(&self) -> Box<dyn DabEventHandler + Send + Sync> {
        Box::new(self.clone())
    }
}

#[derive(Debug)]
pub struct DeviceImpl<IRippleHelper> {
    pub helper: Box<IRippleHelper>,
    pub platform_state: PlatformState,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceVersionResponse {
    pub api: FireboltSemanticVersion,
    pub firmware: FireboltSemanticVersion,
    pub os: FireboltSemanticVersion,
    pub debug: String,
}

pub fn filter_mac(mac_address: String) -> String {
    String::from(mac_address.replace(":", ""))
}

pub async fn get_device_id(helper: &Box<RippleHelper>, ctx: CallContext) -> RpcResult<String> {
    if let Ok(s) = get_distributor_session(helper.clone(), ctx).await {
        if let Some(dev_id) = s.device_id {
            Ok(dev_id)
        } else {
            //None
            get_ll_mac_addr(&helper).await
        }
    } else {
        //Err
        get_ll_mac_addr(&helper).await
    }
}

pub async fn uid(helper: &Box<RippleHelper>, ctx: &CallContext) -> RpcResult<String> {
    let (app_resp_tx, app_resp_rx) = oneshot::channel::<Result<AppManagerResponse, AppError>>();
    let app_id = ctx.app_id.clone();
    let app_request = AppRequest {
        method: crate::apps::app_mgr::AppMethod::GetStartPage(app_id.clone()),
        resp_tx: Some(app_resp_tx),
    };

    helper.send_app_request(app_request).await?;
    let id = get_device_id(helper, ctx.clone()).await?;
    let mut app_scope = app_id;
    if let Ok(resp) = rpc_await_oneshot(app_resp_rx).await? {
        if let AppManagerResponse::StartPage(sp_opt) = resp {
            if let Some(start_page) = sp_opt {
                app_scope = extract_app_ref(start_page)
            }
        }
    }
    return Ok(salt_using_app_scope(
        app_scope,
        id,
        helper.get_config().get_default_id_salt(),
    ));
}

pub async fn get_ll_mac_addr(helper: &Box<RippleHelper>) -> RpcResult<String> {
    let resp = helper
        .send_dab(DabRequestPayload::Device(DeviceRequest::MacAddress))
        .await;
    match resp {
        Ok(payload) => Ok(filter_mac(payload.as_string().unwrap())),
        Err(_e) => {
            // TODO: What do error responses look like?
            Err(jsonrpsee::core::Error::Custom(String::from(
                "FB error response TBD",
            )))
        }
    }
}

pub async fn set_device_name(state: &PlatformState, prop: SetStringProperty) -> RpcResult<()> {
    StorageManager::set_string(state, StorageProperty::DeviceName, prop.value, None).await
}

pub async fn get_device_name(state: &PlatformState) -> RpcResult<String> {
    StorageManager::get_string(state, StorageProperty::DeviceName).await
}

pub async fn get_firmware_info(helper: &Box<RippleHelper>) -> RpcResult<FireboltSemanticVersion> {
    let resp = helper
        .send_dab(DabRequestPayload::Device(DeviceRequest::Version))
        .await;

    match resp {
        Ok(dab_payload) => match dab_payload {
            DabResponsePayload::FirmwareInfo(value) => Ok(value),
            _ => Err(jsonrpsee::core::Error::Custom(String::from(
                "Firmware Info error response TBD",
            ))),
        },
        Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
            "Firmware Info error response TBD",
        ))),
    }
}
pub async fn get_hdr(helper: &Box<RippleHelper>) -> RpcResult<HashMap<HdrProfile, bool>> {
    let resp = helper
        .send_dab(DabRequestPayload::Device(DeviceRequest::Hdr))
        .await;

    match resp {
        Ok(dab_payload) => match dab_payload {
            DabResponsePayload::HdrResponse(value) => Ok(value),
            _ => Err(jsonrpsee::core::Error::Custom(String::from(
                "Hdr capabilities error response TBD",
            ))),
        },
        Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
            "Hdr capabilities error response TBD",
        ))),
    }
}
pub async fn get_make(helper: &Box<RippleHelper>) -> RpcResult<String> {
    match helper
        .send_dab(DabRequestPayload::Device(DeviceRequest::Make))
        .await
    {
        Ok(payload) => Ok(payload.as_string().unwrap()),
        Err(_e) => {
            // TODO: What do error responses look like?
            Err(jsonrpsee::core::Error::Custom(String::from(
                "FB error response TBD",
            )))
        }
    }
}
pub async fn get_model(helper: &Box<RippleHelper>) -> RpcResult<String> {
    let resp = helper
        .send_dab(DabRequestPayload::Device(DeviceRequest::Model))
        .await;

    match resp {
        Ok(payload) => {
            let mfns = helper.clone().get_config().get_model_friendly_names();
            let friendly = mfns.get(&payload.as_string().unwrap());
            if let Some(f) = friendly {
                Ok(f.clone())
            } else {
                // if we cannot find a friendly name, just return the sku
                Ok(payload.as_string().unwrap())
            }
        }
        Err(_e) => {
            // TODO: What do error responses look like?
            Err(jsonrpsee::core::Error::Custom(String::from(
                "FB error response TBD",
            )))
        }
    }
}
pub async fn get_video_resolution(helper: &Box<RippleHelper>) -> RpcResult<Vec<i32>> {
    let resp = helper
        .send_dab(DabRequestPayload::Device(DeviceRequest::VideoResolution))
        .await;

    match resp {
        Ok(dab_payload) => match dab_payload {
            DabResponsePayload::VideoResolutionResponse(value) => Ok(value),
            _ => Err(jsonrpsee::core::Error::Custom(String::from(
                "video_resolution cap error response TBD",
            ))),
        },
        Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
            "video_resolution cap error response TBD",
        ))),
    }
}
pub async fn get_screen_resolution(helper: &Box<RippleHelper>) -> RpcResult<Vec<i32>> {
    let resp = helper
        .send_dab(DabRequestPayload::Device(DeviceRequest::ScreenResolution))
        .await;

    match resp {
        Ok(dab_payload) => match dab_payload {
            DabResponsePayload::ScreenResolutionResponse(value) => Ok(value),
            _ => Err(jsonrpsee::core::Error::Custom(String::from(
                "screen_resolution error response TBD",
            ))),
        },
        Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
            "screen_resolution error response TBD",
        ))),
    }
}
pub async fn get_sku(helper: &Box<RippleHelper>) -> RpcResult<String> {
    let resp = helper
        .send_dab(DabRequestPayload::Device(DeviceRequest::Model))
        .await;

    match resp {
        Ok(payload) => Ok(payload.as_string().unwrap()),
        Err(_e) => {
            // TODO: What do error responses look like?
            Err(jsonrpsee::core::Error::Custom(String::from(
                "FB error response TBD",
            )))
        }
    }
}

pub async fn get_hdcp_status(helper: &Box<RippleHelper>) -> RpcResult<HDCPStatus> {
    let resp = helper
        .send_dab(DabRequestPayload::Device(DeviceRequest::HdcpStatus))
        .await;

    match resp {
        Ok(dab_payload) => match dab_payload {
            DabResponsePayload::HdcpStatusResponse(value) => Ok(value),
            _ => Err(jsonrpsee::core::Error::Custom(String::from(
                "Hdcp status error response TBD",
            ))),
        },
        Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
            "Hdcp status error response TBD",
        ))),
    }
}

impl DeviceImpl<RippleHelper> {
    #[instrument(skip(self))]
    async fn hdcp_status(&self, _ctx: CallContext) -> RpcResult<HDCPStatus> {
        let resp = self
            .helper
            .send_dab(DabRequestPayload::Device(DeviceRequest::HdcpStatus))
            .await;

        match resp {
            Ok(dab_payload) => match dab_payload {
                DabResponsePayload::HdcpStatusResponse(value) => Ok(value),
                _ => Err(jsonrpsee::core::Error::Custom(String::from(
                    "Hdcp status error response TBD",
                ))),
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "Hdcp status error response TBD",
            ))),
        }
    }

    #[instrument(skip(self))]
    async fn firmware_info(&self, _ctx: CallContext) -> RpcResult<FireboltSemanticVersion> {
        let resp = self
            .helper
            .send_dab(DabRequestPayload::Device(DeviceRequest::Version))
            .await;

        match resp {
            Ok(dab_payload) => match dab_payload {
                DabResponsePayload::FirmwareInfo(value) => Ok(value),
                _ => Err(jsonrpsee::core::Error::Custom(String::from(
                    "Firmware Info error response TBD",
                ))),
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "Firmware Info error response TBD",
            ))),
        }
    }
}

#[async_trait]
impl DeviceServer for DeviceImpl<RippleHelper> {
    #[instrument(skip(self))]
    async fn name(&self, _ctx: CallContext) -> RpcResult<String> {
        get_device_name(&self.platform_state).await
    }

    #[instrument(skip(self))]
    async fn set_name(
        &self,
        _ctx: CallContext,
        setname_request: SetStringProperty,
    ) -> RpcResult<()> {
        set_device_name(&self.platform_state, setname_request).await
    }

    #[instrument(skip(self))]
    async fn on_name_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        AppEvents::add_listener(
            &&self.platform_state.app_events_state,
            EVENT_DEVICE_NAME_CHANGED.to_string(),
            ctx,
            request,
        );
        Ok(ListenerResponse {
            listening: listen,
            event: EVENT_DEVICE_NAME_CHANGED,
        })
    }

    #[instrument(skip(self))]
    async fn on_device_name_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        AppEvents::add_listener(
            &&self.platform_state.app_events_state,
            EVENT_DEVICE_DEVICE_NAME_CHANGED.to_string(),
            ctx,
            request,
        );
        Ok(ListenerResponse {
            listening: listen,
            event: EVENT_DEVICE_DEVICE_NAME_CHANGED,
        })
    }

    #[instrument(skip(self))]
    async fn id(&self, ctx: CallContext) -> RpcResult<String> {
        get_device_id(&self.helper, ctx).await
    }

    #[instrument(skip(self))]
    async fn uid(&self, ctx: CallContext) -> RpcResult<String> {
        uid(&self.helper, &ctx).await
    }

    #[instrument(skip(self))]
    async fn platform(&self, _ctx: CallContext) -> RpcResult<String> {
        Ok("WPE".into())
    }

    #[cfg(not(test))]
    #[instrument(skip(self))]
    async fn version(&self, ctx: CallContext) -> RpcResult<DeviceVersionResponse> {
        let mut os = FireboltSemanticVersion::new(
            env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap(),
            env!("CARGO_PKG_VERSION_MINOR").parse().unwrap(),
            env!("CARGO_PKG_VERSION_PATCH").parse().unwrap(),
            "".to_string(),
        );
        os.readable = format!("Firebolt OS v{}", env!("CARGO_PKG_VERSION"));

        let firmware = self.firmware_info(ctx.clone()).await?;
        let api = self.platform_state.firebolt_open_rpc.info.clone();

        Ok(DeviceVersionResponse {
            api,
            firmware,
            os,
            debug: format!("{} ({})", env!("CARGO_PKG_VERSION"), SHA_SHORT),
        })
    }

    #[cfg(test)]
    #[instrument(skip(self))]
    async fn version(&self, ctx: CallContext) -> RpcResult<DeviceVersionResponse> {
        let os = FireboltSemanticVersion::new(1, 2, 3, "Firebolt OS v1.2.3".to_string());

        let firmware = self.firmware_info(ctx.clone()).await?;
        let api = self.platform_state.firebolt_open_rpc.info.clone();

        Ok(DeviceVersionResponse {
            api,
            firmware,
            os,
            debug: format!("1.2.3 ({})", SHA_SHORT),
        })
    }

    #[instrument(skip(self))]
    async fn model(&self, _ctx: CallContext) -> RpcResult<String> {
        let resp = self
            .helper
            .send_dab(DabRequestPayload::Device(DeviceRequest::Model))
            .await;

        match resp {
            Ok(payload) => {
                let mfns = self.helper.get_config().get_model_friendly_names();
                let friendly = mfns.get(&payload.as_string().unwrap());
                if let Some(f) = friendly {
                    Ok(f.clone())
                } else {
                    // if we cannot find a friendly name, just return the sku
                    Ok(payload.as_string().unwrap())
                }
            }
            Err(_e) => {
                // TODO: What do error responses look like?
                Err(jsonrpsee::core::Error::Custom(String::from(
                    "FB error response TBD",
                )))
            }
        }
    }

    #[instrument(skip(self))]
    async fn sku(&self, _ctx: CallContext) -> RpcResult<String> {
        let resp = self
            .helper
            .send_dab(DabRequestPayload::Device(DeviceRequest::Model))
            .await;

        match resp {
            Ok(payload) => Ok(payload.as_string().unwrap()),
            Err(_e) => {
                // TODO: What do error responses look like?
                Err(jsonrpsee::core::Error::Custom(String::from(
                    "FB error response TBD",
                )))
            }
        }
    }

    #[instrument(skip(self))]
    async fn hdcp(&self, _ctx: CallContext) -> RpcResult<HashMap<HdcpProfile, bool>> {
        let resp = self
            .helper
            .send_dab(DabRequestPayload::Device(DeviceRequest::HdcpSupport))
            .await;

        match resp {
            Ok(dab_payload) => match dab_payload {
                DabResponsePayload::HdcpSupportResponse(value) => Ok(value),
                _ => Err(jsonrpsee::core::Error::Custom(String::from(
                    "Hdcp capabilities error response TBD",
                ))),
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "Hdcp capabilities error response TBD",
            ))),
        }
    }

    #[instrument(skip(self))]
    async fn on_hdcp_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;

        AppEvents::add_listener(
            &&self.platform_state.app_events_state,
            HDCP_CHANGED_EVENT.to_string(),
            ctx.clone(),
            request,
        );
        // This requires event subscription to DAB. Using Event Administrator to do DAB subscription
        // and DAB event notifcation.
        let em = DabEventAdministrator::get(self.platform_state.clone());
        if listen {
            // Collect the current HDCP Value.
            let hdcp_val = self.hdcp(ctx.clone()).await.unwrap_or_default();
            let _resp = em
                .dab_event_subscribe(
                    ctx.clone(),
                    HDCP_CHANGED_EVENT.into(),
                    Some(json!(hdcp_val)),
                    Some(Box::new(HDCPEventHandler {})),
                )
                .await;
        } else {
            let _resp = em
                .dab_event_unsubscribe(
                    ctx,
                    HDCP_CHANGED_EVENT.into(),
                    Some(Box::new(HDCPEventHandler {})),
                )
                .await;
        }

        Ok(ListenerResponse {
            listening: listen,
            event: HDCP_CHANGED_EVENT,
        })
    }

    #[instrument(skip(self))]
    async fn hdr(&self, _ctx: CallContext) -> RpcResult<HashMap<HdrProfile, bool>> {
        let resp = self
            .helper
            .send_dab(DabRequestPayload::Device(DeviceRequest::Hdr))
            .await;

        match resp {
            Ok(dab_payload) => match dab_payload {
                DabResponsePayload::HdrResponse(value) => Ok(value),
                _ => Err(jsonrpsee::core::Error::Custom(String::from(
                    "Hdr capabilities error response TBD",
                ))),
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "Hdr capabilities error response TBD",
            ))),
        }
    }

    #[instrument(skip(self))]
    async fn on_hdr_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        AppEvents::add_listener(
            &&self.platform_state.app_events_state,
            HDR_CHANGED_EVENT.to_string(),
            ctx.clone(),
            request,
        );

        // This requires event subscription to DAB. Using EM to do DAB subscription
        // and Event Handling.
        let em = DabEventAdministrator::get(self.platform_state.clone());
        if listen {
            // Collect the current HDR Value.
            let hdr_val = self.hdr(ctx.clone()).await.unwrap_or_default();
            let _resp = em
                .dab_event_subscribe(
                    ctx.clone(),
                    HDR_CHANGED_EVENT.into(),
                    Some(json!(hdr_val)),
                    Some(Box::new(HDREventHandler {})),
                )
                .await;
        } else {
            let _resp = em
                .dab_event_unsubscribe(
                    ctx,
                    HDR_CHANGED_EVENT.into(),
                    Some(Box::new(HDREventHandler {})),
                )
                .await;
        }

        Ok(ListenerResponse {
            listening: listen,
            event: HDR_CHANGED_EVENT,
        })
    }

    #[instrument(skip(self))]
    async fn screen_resolution(&self, _ctx: CallContext) -> RpcResult<Vec<i32>> {
        let resp = self
            .helper
            .send_dab(DabRequestPayload::Device(DeviceRequest::ScreenResolution))
            .await;

        match resp {
            Ok(dab_payload) => match dab_payload {
                DabResponsePayload::ScreenResolutionResponse(value) => Ok(value),
                _ => Err(jsonrpsee::core::Error::Custom(String::from(
                    "screen_resolution error response TBD",
                ))),
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "screen_resolution error response TBD",
            ))),
        }
    }
    #[instrument(skip(self))]
    async fn on_screen_resolution_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        AppEvents::add_listener(
            &&self.platform_state.app_events_state,
            SCREEN_RESOLUTION_CHANGED_EVENT.to_string(),
            ctx.clone(),
            request,
        );

        // This requires event subscription to DAB. Using EM to do DAB subscription
        // and Event Handling.
        let em = DabEventAdministrator::get(self.platform_state.clone());
        if listen {
            let _resp = em
                .dab_event_subscribe(
                    ctx.clone(),
                    SCREEN_RESOLUTION_CHANGED_EVENT.into(),
                    None,
                    Some(Box::new(ScreenResolutionEventHandler {})),
                )
                .await;
        } else {
            let _resp = em
                .dab_event_unsubscribe(
                    ctx,
                    SCREEN_RESOLUTION_CHANGED_EVENT.into(),
                    Some(Box::new(ScreenResolutionEventHandler {})),
                )
                .await;
        }

        Ok(ListenerResponse {
            listening: listen,
            event: SCREEN_RESOLUTION_CHANGED_EVENT,
        })
    }

    #[instrument(skip(self))]
    async fn video_resolution(&self, _ctx: CallContext) -> RpcResult<Vec<i32>> {
        let resp = self
            .helper
            .send_dab(DabRequestPayload::Device(DeviceRequest::VideoResolution))
            .await;

        match resp {
            Ok(dab_payload) => match dab_payload {
                DabResponsePayload::VideoResolutionResponse(value) => Ok(value),
                _ => Err(jsonrpsee::core::Error::Custom(String::from(
                    "video_resolution cap error response TBD",
                ))),
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "video_resolution cap error response TBD",
            ))),
        }
    }

    #[instrument(skip(self))]
    async fn on_video_resolution_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        AppEvents::add_listener(
            &&self.platform_state.app_events_state,
            VIDEO_RESOLUTION_CHANGED_EVENT.to_string(),
            ctx.clone(),
            request,
        );

        // This requires event subscription to DAB. Using EM to do DAB subscription
        // and Event Handling.
        let em = DabEventAdministrator::get(self.platform_state.clone());
        if listen {
            let _resp = em
                .dab_event_subscribe(
                    ctx.clone(),
                    VIDEO_RESOLUTION_CHANGED_EVENT.into(),
                    None,
                    Some(Box::new(VideoResolutionEventHandler {})),
                )
                .await;
        } else {
            let _resp = em
                .dab_event_unsubscribe(
                    ctx,
                    VIDEO_RESOLUTION_CHANGED_EVENT.into(),
                    Some(Box::new(VideoResolutionEventHandler {})),
                )
                .await;
        }

        Ok(ListenerResponse {
            listening: listen,
            event: VIDEO_RESOLUTION_CHANGED_EVENT,
        })
    }
    #[instrument(skip(self))]
    async fn make(&self, _ctx: CallContext) -> RpcResult<String> {
        let resp = self
            .helper
            .send_dab(DabRequestPayload::Device(DeviceRequest::Make))
            .await;

        match resp {
            Ok(payload) => Ok(payload.as_string().unwrap()),
            Err(_e) => {
                // TODO: What do error responses look like?
                Err(jsonrpsee::core::Error::Custom(String::from(
                    "FB error response TBD",
                )))
            }
        }
    }

    #[instrument(skip(self))]
    async fn typ(&self, _ctx: CallContext) -> RpcResult<String> {
        Ok(self
            .helper
            .get_config()
            .clone()
            .str(ConfigRequest::FormFactor))
    }

    #[instrument(skip(self))]
    async fn audio(&self, _ctx: CallContext) -> RpcResult<HashMap<AudioProfile, bool>> {
        let resp = self
            .helper
            .send_dab(DabRequestPayload::Device(DeviceRequest::Audio))
            .await;

        match resp {
            Ok(dab_payload) => match dab_payload {
                DabResponsePayload::AudioProfileResponse(audio) => Ok(audio),
                _ => Err(jsonrpsee::core::Error::Custom(String::from(
                    "Audio error response TBD",
                ))),
            },
            Err(_e) => {
                // TODO: What do error responses look like?
                Err(jsonrpsee::core::Error::Custom(String::from(
                    "Audio error response TBD",
                )))
            }
        }
    }
    #[instrument(skip(self))]
    async fn on_audio_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;

        AppEvents::add_listener(
            &&self.platform_state.app_events_state,
            "device.onAudioChanged".to_string(),
            ctx,
            request,
        );
        Ok(ListenerResponse {
            listening: listen,
            event: "device.onAudioChanged",
        })
    }

    async fn network(&self, _ctx: CallContext) -> RpcResult<NetworkResponse> {
        let resp = self
            .helper
            .send_dab(DabRequestPayload::Device(DeviceRequest::Network))
            .await;

        match resp {
            Ok(dab_payload) => match dab_payload {
                DabResponsePayload::NetworkResponse(value) => Ok(value),
                _ => Err(jsonrpsee::core::Error::Custom(String::from(
                    "Network Status error response TBD",
                ))),
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "Network status error response TBD",
            ))),
        }
    }

    #[instrument(skip(self))]
    async fn on_network_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        AppEvents::add_listener(
            &&self.platform_state.app_events_state,
            NETWORK_CHANGED_EVENT.to_string(),
            ctx.clone(),
            request,
        );

        // This requires event subscription to DAB. Using EM to do DAB subscription
        // and Event Handling.
        let em = DabEventAdministrator::get(self.platform_state.clone());
        if listen {
            let _resp = em
                .dab_event_subscribe(
                    ctx.clone(),
                    NETWORK_CHANGED_EVENT.into(),
                    None,
                    Some(Box::new(NetworkEventHandler {})),
                )
                .await;
        } else {
            let _resp = em
                .dab_event_unsubscribe(
                    ctx,
                    NETWORK_CHANGED_EVENT.into(),
                    Some(Box::new(NetworkEventHandler {})),
                )
                .await;
        }

        Ok(ListenerResponse {
            listening: listen,
            event: NETWORK_CHANGED_EVENT,
        })
    }

    #[instrument(skip(self))]
    async fn provision(
        &self,
        _ctx: CallContext,
        provision_request: ProvisionRequest,
    ) -> RpcResult<()> {
        let session = provision_request.clone().get_session();
        let resp = self
            .helper
            .send_dab(DabRequestPayload::Distributor(
                DistributorRequest::SetProvision(session),
            ))
            .await;

        // clear the cached distributor session
        self.platform_state
            .app_auth_sessions
            .clear_device_auth_session()
            .await;

        match resp {
            Ok(dab_payload) => match dab_payload {
                DabResponsePayload::None => Ok(()),
                _ => Err(jsonrpsee::core::Error::Custom(String::from(
                    "Provision Status error response TBD",
                ))),
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "Provision status error response TBD",
            ))),
        }
    }

    #[instrument(skip(self))]
    async fn distributor(&self, _ctx: CallContext) -> RpcResult<String> {
        let sess = get_distributor_session_from_platform_state(&self.platform_state).await?;
        match sess.id {
            Some(id) => Ok(id),
            None => Err(jsonrpsee::core::Error::Call(CallError::Custom {
                code: CAPABILITY_NOT_AVAILABLE,
                message: format!(
                    "{} is not available",
                    FireboltCap::Short(String::from("device:distributor")).as_str()
                ),
                data: None,
            })),
        }
    }
}

pub struct DeviceRippleProvider;

pub struct DeviceCapHandler;

impl IGetLoadedCaps for DeviceCapHandler {
    fn get_loaded_caps(&self) -> RippleHandlerCaps {
        RippleHandlerCaps {
            caps: Some(vec![CapClassifiedRequest::Supported(vec![
                FireboltCap::short("device:model"),
                FireboltCap::short("device:sku"),
                FireboltCap::short("device:make"),
                FireboltCap::short("device:id"),
                FireboltCap::short("badger:deviceCapabilities"),
                FireboltCap::short("device:info"),
                FireboltCap::short("network:status"),
                FireboltCap::short("device:distributor"),
                FireboltCap::short("device:uid"),
                FireboltCap::short("device:name"),
            ])]),
        }
    }
}

impl RPCProvider<DeviceImpl<RippleHelper>, DeviceCapHandler> for DeviceRippleProvider {
    fn provide(
        self,
        rhf: Box<RippleHelperFactory>,
        platform_state: PlatformState,
    ) -> (RpcModule<DeviceImpl<RippleHelper>>, DeviceCapHandler) {
        let a = DeviceImpl {
            helper: rhf.clone().get(self.get_helper_variant()),
            platform_state: platform_state.clone(),
        };
        (a.into_rpc(), DeviceCapHandler)
    }

    fn get_helper_variant(self) -> Vec<RippleHelperType> {
        vec![
            RippleHelperType::Dab,
            RippleHelperType::Dpab,
            RippleHelperType::AppManager,
        ]
    }
}

