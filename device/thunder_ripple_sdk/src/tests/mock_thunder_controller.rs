use std::{collections::HashMap, future::Future, pin::Pin, str::FromStr, sync::Arc};

use ripple_sdk::{
    api::gateway::rpc_gateway_api::ApiMessage,
    extn::{
        client::extn_client::ExtnClient,
        //mock_extension_client::MockExtnClient
    },
    serde_json,
    tokio::{
        self,
        sync::{
            mpsc::{self, Sender},
            oneshot,
        },
    },
    utils::channel_utils::{mpsc_send_and_log, oneshot_send_and_log},
    //Mockable,
};
use serde_json::Value;

use crate::{
    client::{
        device_operator::{DeviceCallRequest, DeviceChannelRequest, DeviceSubscribeRequest},
        jsonrpc_method_locator::JsonRpcMethodLocator,
        plugin_manager::{PluginState, PluginStateChangeEvent, PluginStatus},
        thunder_async_client::{ThunderAsyncRequest, ThunderAsyncResponse},
        thunder_client::ThunderClient,
        thunder_plugin::ThunderPlugin,
    },
    processors::thunder_device_info::CachedState,
    thunder_state::ThunderState,
};
use ripple_sdk::api::gateway::rpc_gateway_api::JsonRpcApiResponse;

pub type ThunderHandlerFn =
    dyn Fn(DeviceCallRequest, oneshot::Sender<ThunderAsyncResponse>, u64) + Send + Sync;

pub type ThunderSubscriberFn = dyn Fn(
        DeviceSubscribeRequest,
        Sender<ThunderAsyncResponse>,
        u64,
    ) -> Pin<Box<dyn Future<Output = Option<ThunderAsyncResponse>> + Send + 'static>>
    + Send
    + Sync
    + 'static;

#[derive(Clone)]
pub struct MockThunderSubscriberfn {
    fnc: Arc<ThunderSubscriberFn>,
}

impl MockThunderSubscriberfn {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(
                DeviceSubscribeRequest,
                Sender<ThunderAsyncResponse>,
                u64,
            ) -> Pin<Box<dyn Future<Output = Option<ThunderAsyncResponse>> + Send>>
            + Send
            + Sync
            + 'static,
    {
        Self { fnc: Arc::new(f) }
    }

    pub async fn call(
        &self,
        req: DeviceSubscribeRequest,
        sender: Sender<ThunderAsyncResponse>,
        id: u64,
    ) -> Option<ThunderAsyncResponse> {
        println!(
            "MockThunderSubscriberfn call fn gets invoked for DeviceSubscribeRequest : {:?}",
            req
        );
        (self.fnc)(req, sender, id).await
    }
}

#[derive(Default, Clone)]
pub struct CustomHandler {
    pub custom_request_handler: HashMap<String, Arc<ThunderHandlerFn>>,
    pub custom_subscription_handler: HashMap<String, MockThunderSubscriberfn>,
}

#[derive(Default)]
pub struct MockThunderController {
    state_subscribers: Vec<mpsc::Sender<ThunderAsyncResponse>>,
    plugin_states: HashMap<String, String>,
    custom_handlers: CustomHandler,
}

pub struct MockThunderControllerItems {
    pub cached_state: CachedState,
    pub api_message_rx: mpsc::Receiver<ApiMessage>,
}

// const EMPTY_RESPONSE: DeviceResponseMessage = DeviceResponseMessage {
//     message: Value::Null,
//     sub_id: None,
// };

fn empty_response() -> ThunderAsyncResponse {
    ThunderAsyncResponse {
        id: None,
        result: Ok(JsonRpcApiResponse::default()),
    }
}

fn empty_response_with_id(id: u64) -> ThunderAsyncResponse {
    ThunderAsyncResponse {
        id: Some(id),
        result: Ok(JsonRpcApiResponse::default()),
    }
}

impl MockThunderController {
    pub async fn activate(&mut self, callsign: String) {
        let event = PluginStateChangeEvent {
            callsign: callsign.clone(),
            state: PluginState::Activated,
        };
        self.plugin_states
            .insert(callsign.clone(), String::from("activated"));
        for s in &self.state_subscribers {
            let val = serde_json::to_value(event.clone()).unwrap_or_default();
            //let msg = DeviceResponseMessage::call(val);
            let thunderasyncresp = ThunderAsyncResponse {
                id: None,
                result: Ok(JsonRpcApiResponse {
                    jsonrpc: "2.0".to_owned(),
                    id: None,
                    result: Some(val),
                    error: None,
                    method: None,
                    params: None,
                }),
            };

            mpsc_send_and_log(s, thunderasyncresp, "StateChange").await;
        }
    }

    pub async fn status(&self, callsign: String) -> Vec<PluginStatus> {
        let state = match self.plugin_states.get(&callsign) {
            Some(s) => s.clone(),
            None => String::from("deactivated"),
        };
        Vec::from([PluginStatus { state }])
    }

    pub async fn on_state_change(&mut self, callback: mpsc::Sender<ThunderAsyncResponse>) {
        self.state_subscribers.push(callback);
    }

    pub async fn handle_thunder_call(
        &mut self,
        thunder_async_request: ThunderAsyncRequest,
        thunder_async_response_tx: mpsc::Sender<ThunderAsyncResponse>,
    ) {
        println!(
            "*** _DEBUG: handle_thunder_call: DeviceChannelRequest::Call req received : {:?}",
            thunder_async_request
        );

        // <pca> Naveen: You'll need to refactor this method to also handle cases where thunder_async_request.request is
        // not a DeviceCallRequest, I'm just unwrapping here to show you how to access the request.
        let msg = thunder_async_request
            .request
            .get_dev_call_request()
            .unwrap();
        // </pca>

        let locator = JsonRpcMethodLocator::from_str(&msg.method).unwrap();
        let module = locator.module.unwrap();

        if module == ThunderPlugin::Controller.callsign() {
            if locator.method_name == "activate" {
                println!("*** _DEBUG: in activate block");
                let ps = msg.params.unwrap().as_params();
                let psv: Value = serde_json::from_str(ps.as_str()).expect("Message should be JSON");
                let cs = psv.get("callsign").unwrap();

                self.activate(String::from(cs.as_str().unwrap())).await;
                mpsc_send_and_log(&thunder_async_response_tx, empty_response(), "ActivateAck")
                    .await;
            } else if msg.method.starts_with("status") {
                println!("*** _DEBUG: in status block");
                let status = self.status(locator.qualifier.unwrap()).await;
                let val = serde_json::to_value(status).unwrap_or_default();
                let thunderasyncresp = ThunderAsyncResponse {
                    id: None,
                    result: Ok(JsonRpcApiResponse {
                        jsonrpc: "2.0".to_owned(),
                        id: None,
                        result: Some(val),
                        error: None,
                        method: None,
                        params: None,
                    }),
                };
                mpsc_send_and_log(&thunder_async_response_tx, thunderasyncresp, "StatusReturn")
                    .await;
            }
        } else if let Some(handler) = self
            .custom_handlers
            .custom_request_handler
            .get(&format!("{}.{}", module, locator.method_name))
        {
            println!("*** _DEBUG: MockThunderController: handle_thunder_call:  calling custom handler for {}.{}", module, locator.method_name);

            let (handler_response_tx, handler_response_rx) =
                oneshot::channel::<ThunderAsyncResponse>();

            (handler)(msg.clone(), handler_response_tx, thunder_async_request.id);

            if let Ok(response) = handler_response_rx.await {
                println!("*** _DEBUG: MockThunderController: handle_thunder_call:  received response from custom handler for {}.{}", module, locator.method_name);
                println!(
                    "*** _DEBUG: MockThunderController: handle_thunder_call:  response: {:?}",
                    response
                );
                mpsc_send_and_log(&thunder_async_response_tx, response, "CustomResponse").await;
            } else {
                println!("*** _DEBUG: MockThunderController: handle_thunder_call:  no response received from custom handler for {}.{}", module, locator.method_name);
            }
        } else {
            println!(
                "No mock thunder response found for {}.{}",
                module, locator.method_name
            );
            return;
        }
    }

    pub async fn handle_thunder_unsub(&mut self, _msg: ThunderAsyncRequest) {}

    pub async fn handle_thunder_sub(
        &mut self,
        thunder_async_request: ThunderAsyncRequest,
        handler: mpsc::Sender<ThunderAsyncResponse>,
    ) {
        println!("@@@NNA.... we reached  handle_thunder_sub...");
        // Extract DeviceSubscribeRequest, module, and event_name from the ThunderAsyncRequest
        // let (sub_req, module, event_name, id) = match &thunder_async_request.request {
        //     DeviceChannelRequest::Subscribe(sub_req) => (
        //         sub_req.clone(),
        //         sub_req.module.clone(),
        //         sub_req.event_name.clone(),
        //         thunder_async_request.id,
        //     ),
        //     _ => {
        //         println!("handle_thunder_sub called with non-Subscribe request");
        //         // Optionally send an error or ack via handler if needed
        //         let _ = mpsc_send_and_log(&handler, empty_response(), "SubscribeAck").await;
        //         return;
        //     }
        // };

        let sub_req = thunder_async_request
            .request
            .get_dev_subscribe_request()
            .unwrap();

        let module = sub_req.module.clone();
        let event_name = sub_req.event_name.clone();
        let id = thunder_async_request.id;

        if module == "Controller.1" && event_name == "statechange" {
            println!("Controller.1 inside with statechange");
            self.on_state_change(handler.clone()).await;
            let _ = mpsc_send_and_log(&handler, empty_response(), "SubscribeAck").await;
        } else if let Some(handler_fn) = self
            .custom_handlers
            .custom_subscription_handler
            .get(&format!("{}.{}", module, event_name))
        {
            println!("before handler_fn.call");
            let response = handler_fn.call(sub_req, handler.clone(), id).await;
            if let Some(resp) = response {
                println!(" handler_fn.call inside if");
                mpsc_send_and_log(&handler, resp, "OnStatusChange").await;
            } else {
                println!(" handler_fn.call inside else");
                // Always send an ack to avoid hanging if handler returns None
                let _ =
                    mpsc_send_and_log(&handler, empty_response_with_id(id), "SubscribeAck").await;
            }
        } else {
            println!("No mock subscription found for {}.{}", module, event_name);
            // Always send an ack to avoid hanging
            let _ = mpsc_send_and_log(&handler, empty_response(), "SubscribeAck").await;
        }
    }

    // pub async fn handle_thunder_sub(
    //     &mut self,
    //     thunder_async_request: ThunderAsyncRequest,
    //     handler: mpsc::Sender<ThunderAsyncResponse>,
    // ) {
    //     println!("@@@NNA.... we reached  handle_thunder_sub...");
    //     let (tx, _rx) = oneshot::channel::<ThunderAsyncResponse>();

    //     // Extract DeviceSubscribeRequest, module, and event_name from the ThunderAsyncRequest
    //     let (sub_req, module, event_name, id) = match &thunder_async_request.request {
    //         DeviceChannelRequest::Subscribe(sub_req) => (
    //             sub_req.clone(),
    //             sub_req.module.clone(),
    //             sub_req.event_name.clone(),
    //             thunder_async_request.id,
    //         ),
    //         _ => {
    //             println!("handle_thunder_sub called with non-Subscribe request");
    //             oneshot_send_and_log(tx, empty_response(), "SubscribeAck");
    //             return;
    //         }
    //     };

    //     if module == "Controller.1" && event_name == "statechange" {
    //         self.on_state_change(handler).await;
    //         oneshot_send_and_log(tx, empty_response(), "SubscribeAck");
    //     } else if let Some(handler_fn) = self
    //         .custom_handlers
    //         .custom_subscription_handler
    //         .get(&format!("{}.{}", module, event_name))
    //     {
    //         let response = handler_fn.call(sub_req, handler.clone(), id).await;
    //         if let Some(resp) = response {
    //             mpsc_send_and_log(&handler, resp, "OnStatusChange").await;
    //         }
    //     } else {
    //         println!("No mock subscription found for {}.{}", module, event_name);
    //         //oneshot_send_and_log(tx, empty_response(), "SubscribeAck");
    //         return;
    //     }
    //     // oneshot_send_and_log(tx, empty_response(), "SubscribeAck");
    // }

    pub fn start() -> (
        mpsc::Sender<ThunderAsyncRequest>,
        mpsc::Receiver<ThunderAsyncResponse>,
    ) {
        MockThunderController::start_with_custom_handlers(None)
    }

    pub fn start_with_custom_handlers(
        custom_handlers: Option<CustomHandler>,
    ) -> (
        mpsc::Sender<ThunderAsyncRequest>,
        mpsc::Receiver<ThunderAsyncResponse>,
    ) {
        println!("*** _DEBUG: start_with_custom_handlers: invoked");
        let (thunder_async_request_tx, mut thunder_async_request_rx) =
            mpsc::channel::<ThunderAsyncRequest>(32);

        let (thunder_async_response_tx, thunder_async_response_rx) =
            mpsc::channel::<ThunderAsyncResponse>(32);

        tokio::spawn(async move {
            let mut mock_controller = MockThunderController::default();
            if let Some(ch) = custom_handlers {
                mock_controller.custom_handlers = ch;
            }
            while let Some(thunder_async_request) = thunder_async_request_rx.recv().await {
                println!("*** _DEBUG: MockThunderController: start_with_custom_handlers: received request: {:?}", thunder_async_request);

                match thunder_async_request.request {
                    DeviceChannelRequest::Call(ref msg) => {
                        println!("*** _DEBUG: start_with_custom_handlers: DeviceChannelRequest::Call req received : {:?}", msg);
                        mock_controller
                            .handle_thunder_call(
                                thunder_async_request,
                                thunder_async_response_tx.clone(),
                            )
                            .await;
                    }
                    DeviceChannelRequest::Subscribe(ref msg) => {
                        println!("*** _DEBUG: start_with_custom_handlers: DeviceChannelRequest::Subscribe req received : {:?}", msg);
                        mock_controller
                            .handle_thunder_sub(
                                thunder_async_request,
                                thunder_async_response_tx.clone(),
                            )
                            .await;
                    }
                    DeviceChannelRequest::Unsubscribe(ref _msg) => {
                        mock_controller
                            .handle_thunder_unsub(thunder_async_request)
                            .await;
                    }
                }
            }
        });

        (thunder_async_request_tx, thunder_async_response_rx)
    }

    /**
     * Creates state object that points to a mock thunder controller.
     * Pass in the custom thunder handlers to mock the thunder responses
     * Returns the state and a receiver which can be used to listen to responses that
     * come back from the extension
     */
    pub fn state_with_mock(custom_thunder: Option<CustomHandler>) -> MockThunderControllerItems {
        println!("*** _DEBUG: state_with_mock: invoked");
        let (thunder_async_request_tx, mut thunder_async_response_rx) =
            MockThunderController::start_with_custom_handlers(custom_thunder);

        let thunder_client = ThunderClient::mock_thunderclient(thunder_async_request_tx);

        let (api_message_tx, api_message_rx) = mpsc::channel::<ApiMessage>(32);
        let extn_client = ExtnClient::new_main_with_sender(api_message_tx);

        let thunder_state = ThunderState::new(extn_client, thunder_client);

        let cache = CachedState::new(thunder_state);
        let cache_for_task = cache.clone();

        // receive thunderasyncresposne here
        tokio::spawn(async move {
            while let Some(thunder_async_response) = thunder_async_response_rx.recv().await {
                println!(
                    "*** _DEBUG: state_with_mock: Received ThunderAsyncResponse: {:?}",
                    thunder_async_response
                );
                println!("@@@NNa... Entered ThunderAsyncResponse handler block");
                let thndr_client = cache_for_task.state.get_thunder_client();
                ///if let Some(id) = thunder_async_response.get_id() {
                if let Some(id) = thunder_async_response.id {
                    println!("@@@NNa... ThunderAsyncResponse has id={}", id);
                    if let Some(thunder_async_callbacks) = thndr_client.thunder_async_callbacks {
                        println!("@@@NNa... thunder_async_callbacks found");
                        let mut callbacks = thunder_async_callbacks.write().unwrap();
                        if let Some(Some(callback)) = callbacks.remove(&id) {
                            println!("@@@NNa... Callback found for id={}", id);
                            if let Some(device_response_message) =
                                thunder_async_response.get_device_resp_msg(None)
                            {
                                println!("@@@NNa... Sending device_response_message for id={}", id);
                                oneshot_send_and_log(
                                    callback,
                                    device_response_message,
                                    "ThunderResponse",
                                );
                            } else {
                                println!("@@@NNa... No device_response_message for id={}", id);
                            }
                        } else {
                            println!("@@@NNa... No callback found for id={}", id);
                        }
                    } else {
                        println!("@@@NNa... thunder_async_callbacks is None");
                    }
                } else {
                    println!("@@@NNa... ThunderAsyncResponse has no id");
                }
            }
        });

        MockThunderControllerItems {
            cached_state: cache,
            api_message_rx,
        }
    }

    //NNA changes
    pub fn get_thunder_state_mock_with_handler(handler: Option<CustomHandler>) -> ThunderState {
        // let thunder_client = ThunderClient::mock();

        // let extn_client = MockExtnClient::client();
        // ThunderState::new(extn_client, thunder_client)

        let (thunder_async_request_tx, _thunder_async_response_rx) =
            MockThunderController::start_with_custom_handlers(handler);

        let thunder_client = ThunderClient::mock_thunderclient(thunder_async_request_tx);

        let (api_message_tx, _api_message_rx) = mpsc::channel::<ApiMessage>(32);
        let extn_client = ExtnClient::new_main_with_sender(api_message_tx);

        ThunderState::new(extn_client, thunder_client)
    }

    /**
     * Creates state object that points to a mock thunder controller.
     * Pass in the custom thunder handlers to mock the thunder responses
     * Returns the state and a receiver which can be used to listen to responses that
     * come back from the extension
     */
    pub fn get_thunder_state_mock() -> ThunderState {
        Self::get_thunder_state_mock_with_handler(None)
    }
}
