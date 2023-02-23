use std::net::SocketAddr;

use super::firebolt_gateway::FireboltGatewayCommand;
use crate::{
    service::extn::ripple_client::RippleClient,
    state::{
        platform_state::PlatformState,
        session_state::{Session, SessionState},
    },
};
use futures::SinkExt;
use futures::StreamExt;
use ripple_sdk::tokio;
use ripple_sdk::{
    api::gateway::rpc_gateway_api::{ClientContext, RpcRequest},
    log::{error, info, trace},
    tokio::{
        net::{TcpListener, TcpStream},
        sync::{mpsc, oneshot},
    },
    utils::channel_utils::oneshot_send_and_log,
    uuid::Uuid,
};
use tokio_tungstenite::{
    tungstenite::{self, Message},
    WebSocketStream,
};
#[allow(dead_code)]
pub struct FireboltWs {}

#[derive(Debug)]
#[allow(dead_code)]
pub struct ClientIdentity {
    session_id: String,
    app_id: String,
}

struct ConnectionCallbackConfig {
    pub next: oneshot::Sender<ClientIdentity>,
    pub sessions_state: SessionState,
    pub secure: bool,
    pub internal_app_id: Option<String>,
}
pub struct ConnectionCallback(ConnectionCallbackConfig);

/**
 * Gets a query parameter from the request at the given key.
 * If required=true, then return an error if the param is missing
 */
fn get_query(
    req: &tungstenite::handshake::server::Request,
    key: &'static str,
    required: bool,
) -> Result<Option<String>, tungstenite::handshake::server::ErrorResponse> {
    let found_q = match req.uri().query() {
        Some(qs) => {
            let qs = querystring::querify(qs);
            qs.iter().find(|q| q.0 == key).cloned()
        }
        None => None,
    };

    if required && found_q.is_none() {
        let err_msg = format!("{} query parameter missing", key);
        error!("{}", err_msg);
        let err = tungstenite::http::response::Builder::new()
            .status(403)
            .body(Some(err_msg))
            .unwrap();
        return Err(err);
    }
    Ok(match found_q {
        Some(q) => Some(String::from(q.1)),
        None => None,
    })
}

impl tungstenite::handshake::server::Callback for ConnectionCallback {
    fn on_request(
        self,
        request: &tungstenite::handshake::server::Request,
        response: tungstenite::handshake::server::Response,
    ) -> Result<
        tungstenite::handshake::server::Response,
        tungstenite::handshake::server::ErrorResponse,
    > {
        info!("New firebolt connection {:?}", request.uri().query());
        let cfg = self.0;
        let app_id_opt = match cfg.secure {
            true => None,
            false => match get_query(request, "appId", false)? {
                Some(a) => Some(a),
                None => cfg.internal_app_id,
            },
        };
        let session_id = match app_id_opt {
            Some(_) => Uuid::new_v4().to_string(),
            // can unwrap here because if session is not given, then error will be returned
            None => get_query(request, "session", true)?.unwrap(),
        };
        let app_id = match app_id_opt {
            Some(a) => a,
            None => {
                let a = cfg.sessions_state.get_app_id(session_id.clone());

                if let None = a {
                    let err = tungstenite::http::response::Builder::new()
                        .status(403)
                        .body(Some(String::from("No application session found")))
                        .unwrap();
                    error!("No application session found");
                    return Err(err);
                }
                a.unwrap()
            }
        };
        let cid = ClientIdentity {
            session_id: String::from(session_id),
            app_id,
        };
        oneshot_send_and_log(cfg.next, cid, "ResolveClientIdentity");
        Ok(response)
    }
}

impl FireboltWs {
    #[allow(dead_code)]
    pub async fn start(
        server_addr: &str,
        state: PlatformState,
        secure: bool,
        internal_app_id: Option<String>,
    ) {
        // Create the event loop and TCP listener we'll accept connections on.
        let try_socket = TcpListener::bind(&server_addr).await; //create the server on the address
        let listener = try_socket.expect(format!("Failed to bind {:?}", server_addr).as_str());
        info!("Listening on: {} secure={}", server_addr, secure);
        let client = state.get_client();
        let session_state = state.session_state.clone();
        // Let's spawn the handling of each connection in a separate task.
        while let Ok((stream, client_addr)) = listener.accept().await {
            let (connect_tx, connect_rx) = oneshot::channel::<ClientIdentity>();
            let cfg = ConnectionCallbackConfig {
                next: connect_tx,
                sessions_state: session_state.clone(),
                secure: secure,
                internal_app_id: internal_app_id.clone(),
            };
            match tokio_tungstenite::accept_hdr_async(stream, ConnectionCallback(cfg)).await {
                Err(e) => {
                    error!("websocket connection error {:?}", e);
                }
                Ok(ws_stream) => {
                    info!("websocket connection success");
                    let client_c = client.clone();
                    tokio::spawn(async move {
                        FireboltWs::handle_connection(
                            client_addr,
                            ws_stream,
                            connect_rx,
                            client_c.clone(),
                        )
                        .await;
                    });
                }
            }
        }
    }

    #[allow(dead_code)]
    async fn handle_connection(
        _client_addr: SocketAddr,
        ws_stream: WebSocketStream<TcpStream>,
        connect_rx: oneshot::Receiver<ClientIdentity>,
        client: RippleClient,
    ) {
        let identity = connect_rx.await.unwrap();
        let (session_tx, mut resp_rx) = mpsc::channel(32);
        let ctx = ClientContext {
            session_id: identity.session_id.clone(),
            app_id: identity.app_id.clone(),
        };
        let session = Session::new(identity.app_id.clone(), session_tx.clone());
        let msg = FireboltGatewayCommand::RegisterSession {
            session_id: identity.session_id.clone(),
            session,
        };
        if let Err(e) = client.send_gateway_command(msg) {
            error!("Error registering the connection {:?}", e);
            return;
        }

        let (mut sender, mut receiver) = ws_stream.split();
        tokio::spawn(async move {
            while let Some(rs) = resp_rx.recv().await {
                let send_result = sender.send(Message::Text(rs.jsonrpc_msg.clone())).await;
                match send_result {
                    Ok(_) => {
                        trace!("Sent Firebolt response");
                    }
                    Err(err) => error!("{:?}", err),
                }
            }
        });
        while let Some(msg) = receiver.next().await {
            if let Ok(msg) = msg {
                if msg.is_text() && !msg.is_empty() {
                    let req_text = String::from(msg.to_text().unwrap());
                    let req_id = Uuid::new_v4().to_string();
                    if let Ok(request) = RpcRequest::parse(
                        req_text.clone(),
                        ctx.app_id.clone(),
                        ctx.session_id.clone(),
                        req_id.clone(),
                    ) {
                        let msg = FireboltGatewayCommand::HandleRpc { request };
                        if let Err(e) = client.clone().send_gateway_command(msg) {
                            error!("failed to send request {:?}", e);
                        }
                    } else {
                        error!("invalid message {}", req_text)
                    }
                }
            }
        }
        let msg = FireboltGatewayCommand::UnregisterSession {
            session_id: identity.session_id.clone(),
        };
        if let Err(e) = client.send_gateway_command(msg) {
            error!("Error Unregistering {:?}", e);
        }
    }
}
