use crate::{
    api::rpc::rpc_gateway::{CallContext, RPCProvider},
    helpers::{
        error_util::CAPABILITY_NOT_SUPPORTED,
        ripple_helper::{IRippleHelper, RippleHelper, RippleHelperFactory, RippleHelperType},
    },
    managers::capability_manager::{
        CapClassifiedRequest, FireboltCap, IGetLoadedCaps, RippleHandlerCaps,
    },
    platform_state::PlatformState,
};
use dab::core::model::accessory::{
    AccessoryPairRequest, AccessoryProtocol, AccessoryRequest, AccessoryType,
};
use dab::core::{
    message::{DabRequestPayload, DabResponsePayload},
    model::accessory::{
        AccessoryDeviceListResponse, AccessoryDeviceResponse, AccessoryListRequest,
    },
};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    RpcModule,
};

#[rpc(server)]
pub trait Accessory {
    #[method(name = "accessory.list")]
    async fn list(
        &self,
        ctx: CallContext,
        list_request: Option<AccessoryListRequest>,
    ) -> RpcResult<AccessoryDeviceListResponse>;
    #[method(name = "accessory.pair")]
    async fn pair(
        &self,
        ctx: CallContext,
        pair_request: Option<AccessoryPairRequest>,
    ) -> RpcResult<AccessoryDeviceResponse>;
}

pub struct AccessoryImpl<IRippleHelper> {
    pub helper: Box<IRippleHelper>,
}

#[async_trait]
impl AccessoryServer for AccessoryImpl<RippleHelper> {
    async fn list(
        &self,
        _ctx: CallContext,
        list_request_opt: Option<AccessoryListRequest>,
    ) -> RpcResult<AccessoryDeviceListResponse> {
        let list_request = list_request_opt.unwrap_or_default();
        let resp = self
            .helper
            .send_dab(DabRequestPayload::Accessory(AccessoryRequest::List(
                list_request,
            )))
            .await;

        match resp {
            Ok(dab_payload) => match dab_payload {
                DabResponsePayload::AccessoryDeviceListResponse(value) => Ok(value),
                _ => Err(jsonrpsee::core::Error::Custom(String::from(
                    "Accessory List error response TBD",
                ))),
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "Accessory List error response TBD",
            ))),
        }
    }

    async fn pair(
        &self,
        _ctx: CallContext,
        pair_request_opt: Option<AccessoryPairRequest>,
    ) -> RpcResult<AccessoryDeviceResponse> {
        let pair_request = pair_request_opt.unwrap_or_default();
        if pair_request._type.clone() == AccessoryType::Remote {
            match pair_request.protocol.clone() {
                AccessoryProtocol::BluetoothLE => {
                    if !self
                        .helper
                        .is_cap_supported(FireboltCap::short("remote:ble"))
                        .await
                    {
                        return Err(jsonrpsee::core::Error::Custom(format!(
                            "{} Capability Not Supported",
                            CAPABILITY_NOT_SUPPORTED
                        )));
                    }
                }
                AccessoryProtocol::RF4CE => {
                    if !self
                        .helper
                        .is_cap_supported(FireboltCap::short("remote:rf4ce"))
                        .await
                    {
                        return Err(jsonrpsee::core::Error::Custom(format!(
                            "{} Capability Not Supported",
                            CAPABILITY_NOT_SUPPORTED
                        )));
                    }
                }
            }
        }

        let resp = self
            .helper
            .send_dab(DabRequestPayload::Accessory(AccessoryRequest::Pair(
                pair_request,
            )))
            .await;

        match resp {
            Ok(dab_payload) => match dab_payload {
                DabResponsePayload::AccessoryDeviceResponse(value) => Ok(value),
                _ => Err(jsonrpsee::core::Error::Custom(String::from(
                    "Pairing error response TBD",
                ))),
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "Pairing error response TBD",
            ))),
        }
    }
}

pub struct AccessoryRippleProvider;

pub struct AccessoryCapHandler;

impl IGetLoadedCaps for AccessoryCapHandler {
    fn get_loaded_caps(&self) -> RippleHandlerCaps {
        RippleHandlerCaps {
            caps: Some(vec![CapClassifiedRequest::Supported(vec![
                FireboltCap::Short("accessory:pair".into()),
                FireboltCap::Short("accessory:list".into()),
            ])]),
        }
    }
}

impl RPCProvider<AccessoryImpl<RippleHelper>, AccessoryCapHandler> for AccessoryRippleProvider {
    fn provide(
        self,
        rhf: Box<RippleHelperFactory>,
        _platform_state: PlatformState,
    ) -> (RpcModule<AccessoryImpl<RippleHelper>>, AccessoryCapHandler) {
        let a = AccessoryImpl {
            helper: rhf.get(self.get_helper_variant()),
        };
        (a.into_rpc(), AccessoryCapHandler)
    }

    fn get_helper_variant(self) -> Vec<RippleHelperType> {
        vec![
            RippleHelperType::Dab,
            RippleHelperType::Cap,
            RippleHelperType::Dpab,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        api::rpc::api_messages::ApiProtocol, apps::app_events::AppEvents,
        managers::capability_manager::CapRequest,
    };
    use dab::core::{
        message::DabRequest,
        model::{
            accessory::{AccessoryListType, AccessoryProtocolListType},
            distributor::{DistributorRequest, DistributorSession},
        },
    };
    use dpab::core::message::DpabRequest;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_list() {
        let (helper, ctx, dab_rx, dpab_rx, cap_rx, app_events_rx, ps) = helper();
        let request = AccessoryListRequest {
            _type: Some(AccessoryListType::Remote),
            protocol: Some(AccessoryProtocolListType::BluetoothLE),
        };
        tokio::spawn(async move {
            mock_channel(dab_rx, dpab_rx, cap_rx).await;
        });
        let resp = AccessoryImpl { helper }
            .list(ctx, Some(request))
            .await
            .unwrap();
        let accessory_resp = &resp.list[0];
        assert!(
            accessory_resp._type == AccessoryType::Remote
                && accessory_resp.make == "MAKE".to_string()
                && accessory_resp.model == "MODEL".to_string()
                && accessory_resp.protocol == AccessoryProtocol::BluetoothLE
        );
    }

    #[tokio::test]
    async fn test_pair_bluetooth() {
        let (helper, ctx, dab_rx, dpab_rx, cap_rx, app_events_rx, ps) = helper();
        let request = AccessoryPairRequest {
            _type: AccessoryType::Remote,
            protocol: AccessoryProtocol::BluetoothLE,
            timeout: 10,
        };
        tokio::spawn(async move {
            mock_channel(dab_rx, dpab_rx, cap_rx).await;
        });
        let resp_r = AccessoryImpl { helper }.pair(ctx, Some(request)).await;
        println!("x");
        let resp = resp_r.unwrap();

        println!(
            "MK: {:?} MDL: {:?} ACC: {:?}",
            resp.make.clone(),
            resp.model.clone(),
            resp.protocol.clone()
        );
        assert!(
            resp._type == AccessoryType::Remote
                && resp.make == "MAKE".to_string()
                && resp.model == "MODEL".to_string()
                && resp.protocol == AccessoryProtocol::BluetoothLE
        );
    }

    #[tokio::test]
    async fn test_pair_rf4ce() {
        let (helper, ctx, dab_rx, dpab_rx, cap_rx, app_events_rx, ps) = helper();
        let request = AccessoryPairRequest {
            _type: AccessoryType::Remote,
            protocol: AccessoryProtocol::RF4CE,
            timeout: 10,
        };
        tokio::spawn(async move {
            mock_channel(dab_rx, dpab_rx, cap_rx).await;
        });
        let resp = AccessoryImpl { helper }
            .pair(ctx, Some(request))
            .await
            .unwrap();
        assert!(
            resp._type == AccessoryType::Remote
                && resp.make == "MAKE".to_string()
                && resp.model == "MODEL".to_string()
                && resp.protocol == AccessoryProtocol::RF4CE
        );
    }

    fn helper() -> (
        Box<RippleHelper>,
        CallContext,
        mpsc::Receiver<DabRequest>,
        mpsc::Receiver<DpabRequest>,
        mpsc::Receiver<CapRequest>,
        mpsc::Receiver<AppEvents>,
        PlatformState,
    ) {
        let (dab_tx, dab_rx) = mpsc::channel::<DabRequest>(32);
        let (dpab_tx, dpab_rx) = mpsc::channel::<DpabRequest>(32);
        let (cap_tx, cap_rx) = mpsc::channel::<CapRequest>(32);
        let (app_events_tx, app_events_rx) = mpsc::channel::<AppEvents>(32);
        let mut helper = RippleHelper::default();
        helper.sender_hub.dab_tx = Some(dab_tx.clone());
        helper.sender_hub.dpab_tx = Some(dpab_tx.clone());
        helper.sender_hub.cap_tx = Some(cap_tx.clone());
        let ctx = CallContext {
            session_id: "a".to_string(),
            request_id: "b".to_string(),
            app_id: "test".to_string(),
            call_id: 5,
            protocol: ApiProtocol::JsonRpc,
            method: "method".to_string(),
        };
        let mut ps = PlatformState::default();
        ps.services = helper.clone();
        ps.app_auth_sessions.rh = Some(Box::new(helper.clone()));
        return (
            Box::new(helper),
            ctx,
            dab_rx,
            dpab_rx,
            cap_rx,
            app_events_rx,
            ps,
        );
    }

    async fn mock_channel(
        mut dab_rx: mpsc::Receiver<DabRequest>,
        mut dpab_rx: mpsc::Receiver<DpabRequest>,
        mut cap_rx: mpsc::Receiver<CapRequest>,
    ) {
        loop {
            tokio::select! {
                data = dab_rx.recv() => {
                    if let Some(request) = data{
                        match request.payload{
                            DabRequestPayload::Accessory(req) => {
                                match req{
                                    AccessoryRequest::Pair(pair) => {
                                        if let Some(tx) = request.callback{
                                            tx.send(Ok(DabResponsePayload::AccessoryDeviceResponse(AccessoryDeviceResponse{ _type: pair._type, make: "MAKE".to_string(), model: "MODEL".to_string(), protocol: pair.protocol }))).unwrap();
                                        }
                                    },
                                    AccessoryRequest::List(_) => {
                                        if let Some(tx) = request.callback{
                                            tx.send(Ok(DabResponsePayload::AccessoryDeviceListResponse(AccessoryDeviceListResponse{ list: vec![AccessoryDeviceResponse{ _type: AccessoryType::Remote, make: "MAKE".to_string(), model: "MODEL".to_string(), protocol: AccessoryProtocol::BluetoothLE }]}))).unwrap();
                                        }
                                    },
                                }
                            },
                            DabRequestPayload::Distributor(req) => {
                                if let DistributorRequest::Session = req{
                                    if let Some(tx) = request.callback{
                                        tx.send(Ok(DabResponsePayload::DistributorSessionResponse(
                                            DistributorSession {
                                                id: Some(String::from("xglobal")),
                                                token: Some(String::from("token")),
                                                account_id: Some(String::from("test")),
                                                device_id: Some(String::from("device_id")),
                                            },
                                        ))).unwrap();
                                    }
                                }
                            },
                            _ => (),
                        }
                    }
                }
                data = cap_rx.recv() => {
                    if let Some(request) = data{
                        match request{
                        CapRequest::UpdateAvailability(_,occ) =>{
                            if let Some(tx) = occ{
                                tx.callback.send(Ok(())).unwrap();
                            }
                        },
                        CapRequest::IsSupported(_,occ) =>{
                            if let tx = occ{
                                tx.callback.send(Ok(())).unwrap();
                            }
                        },
                        _ => (),
                    }}
                }
            }
        }
    }
}
