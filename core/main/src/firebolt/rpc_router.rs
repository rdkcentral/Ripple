use futures::{future::join_all, StreamExt};
use jsonrpsee::{
    core::{
        server::{
            helpers::MethodSink,
            resource_limiting::Resources,
            rpc_module::{MethodKind, Methods},
        },
        TEN_MB_SIZE_BYTES,
    },
    types::{error::ErrorCode, Id, Params},
};
use ripple_sdk::{
    api::gateway::rpc_gateway_api::{ApiMessage, RpcRequest},
    log::{error, info, trace},
    tokio,
    tokio::sync::mpsc::Sender,
    utils::error::RippleError,
};


pub struct RpcRouter {
    methods: Methods,
    resources: Resources,
}

async fn resolve_route(
    methods: Methods,
    resources: Resources,
    req: RpcRequest,
) -> Result<ApiMessage, RippleError> {
    info!("Routing {}", req.method);
    let id = Id::Number(req.ctx.call_id);
    let (sink_tx, mut sink_rx) = futures_channel::mpsc::unbounded::<String>();
    let sink = MethodSink::new_with_limit(sink_tx, TEN_MB_SIZE_BYTES);
    let mut method_executors = Vec::new();
    let params = Params::new(Some(req.params_json.as_str()));
    match methods.method_with_name(&req.method) {
        None => {
            sink.send_error(id, ErrorCode::MethodNotFound.into());
        }
        Some((name, method)) => match &method.inner() {
            MethodKind::Sync(callback) => match method.claim(name, &resources) {
                Ok(_guard) => {
                    (callback)(id, params, &sink);
                }
                Err(_) => {
                    sink.send_error(id, ErrorCode::MethodNotFound.into());
                }
            },
            MethodKind::Async(callback) => match method.claim(name, &resources) {
                Ok(guard) => {
                    let sink = sink.clone();
                    let id = id.into_owned();
                    let params = params.into_owned();

                    let fut = async move {
                        (callback)(id, params, sink, 1, Some(guard)).await;
                    };
                    method_executors.push(fut);
                }
                Err(_) => {
                    sink.send_error(id, ErrorCode::MethodNotFound.into());
                }
            },
            _ => {
                error!("Unsupported method call");
            }
        },
    }

    join_all(method_executors).await;
    if let Some(r) = sink_rx.next().await {
        return Ok(ApiMessage::new(req.ctx.protocol, r, req.ctx.request_id));
    }
    Err(RippleError::InvalidOutput)
}

impl RpcRouter {
    pub fn new(methods: Methods) -> RpcRouter {
        let resources = Resources::default();
        let init_methods = methods.initialize_resources(&resources).unwrap();
        RpcRouter {
            methods: init_methods,
            resources,
        }
    }

    pub async fn route(&self, req: RpcRequest, sender: Sender<ApiMessage>) {
        let methods = self.methods.clone();
        let resources = self.resources.clone();
        tokio::spawn(async move {
            let session_id = req.ctx.session_id.clone();
            if let Ok(msg) = resolve_route(methods, resources, req).await {
                trace!("sending back to {}", session_id);
                if let Err(e) = sender.send(msg).await {
                    error!("Error while responding back message {:?}", e)
                }
            }
        });
    }
}
