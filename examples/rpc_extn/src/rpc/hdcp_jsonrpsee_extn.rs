use std::{collections::HashMap, time::Duration};

use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use ripple_sdk::{
    api::{
        device::device_request::BaseDeviceRequest, firebolt::fb_hdcp::HdcpProfile,
        gateway::rpc_gateway_api::CallContext,
    },
    crossbeam::channel::{bounded, TryRecvError},
    extn::client::extn_client::ExtnClient,
    extn::extn_client_message::{ExtnMessage, ExtnResponse},
    log::error,
};

pub struct HdcpImpl {
    client: ExtnClient,
}

impl HdcpImpl {
    pub fn new(client: ExtnClient) -> HdcpImpl {
        HdcpImpl { client }
    }
}

#[rpc(server)]
pub trait Hdcp {
    #[method(name = "device.hdcp")]
    fn hdcp(&self, ctx: CallContext) -> RpcResult<HashMap<HdcpProfile, bool>>;
}

impl HdcpServer for HdcpImpl {
    fn hdcp(&self, _ctx: CallContext) -> RpcResult<HashMap<HdcpProfile, bool>> {
        let request = BaseDeviceRequest {
            method: "hdcp-support".into(),
            params: None,
            module: "hdcp".into(),
        };
        let mut client = self.client.clone();

        let (tx, tr) = bounded(2);
        if let Ok(_) = client.request_async(request, tx) {
            loop {
                match tr.try_recv() {
                    Ok(cmessage) => {
                        let message: ExtnMessage = cmessage.try_into().unwrap();
                        if let Some(ExtnResponse::Value(v)) = message.payload.clone().extract() {
                            if let Ok(v) = serde_json::from_value(v) {
                                return Ok(v);
                            }
                        }
                    }
                    Err(e) => match e {
                        TryRecvError::Disconnected => {
                            error!("Channel disconnected");
                            break;
                        }
                        _ => {}
                    },
                }
                std::thread::sleep(Duration::from_millis(50))
            }
        }

        Err(jsonrpsee::core::Error::Custom(String::from(
            "Hdcp capabilities error response TBD",
        )))
    }
}
