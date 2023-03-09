use std::time::Duration;

use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use ripple_sdk::{
    api::gateway::rpc_gateway_api::{ApiProtocol, CallContext, RpcRequest},
    crossbeam::channel::{bounded, TryRecvError},
    extn::client::extn_client::ExtnClient,
    extn::extn_client_message::{ExtnMessage, ExtnResponse},
    log::error,
};

pub struct LegacyImpl {
    client: ExtnClient,
}

impl LegacyImpl {
    pub fn new(client: ExtnClient) -> LegacyImpl {
        LegacyImpl { client }
    }
}

#[rpc(server)]
pub trait Legacy {
    #[method(name = "legacy.make")]
    fn make(&self, ctx: CallContext) -> RpcResult<String>;
}

impl LegacyServer for LegacyImpl {
    fn make(&self, ctx: CallContext) -> RpcResult<String> {
        let mut client = self.client.clone();
        let mut new_ctx = ctx.clone();
        new_ctx.protocol = ApiProtocol::Extn;

        let rpc_request = RpcRequest {
            ctx: new_ctx.clone(),
            method: "device.make".into(),
            params_json: RpcRequest::prepend_ctx(Some(serde_json::Value::Null), &new_ctx),
        };

        let (tx, tr) = bounded(2);
        if let Ok(_) = client.request_async(rpc_request, tx) {
            loop {
                match tr.try_recv() {
                    Ok(cmessage) => {
                        let message: ExtnMessage = cmessage.try_into().unwrap();
                        if let Some(ExtnResponse::Value(v)) = message.payload.clone().extract() {
                            if let Some(v) = v.as_str() {
                                return Ok(v.into());
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

        Ok("some".into())
    }
}
