use std::{collections::HashMap, future::Future, pin::Pin, str::FromStr, sync::Arc};

use ripple_sdk::{
    extn::{client::extn_client::ExtnClient, mock_extension_client::MockExtnClient},
    serde_json,
    tokio::{
        self,
        sync::{
            mpsc::{self, Sender},
            oneshot,
        },
    },
    utils::channel_utils::mpsc_send_and_log,
    Mockable,
};
use serde_json::Value;

use crate::{
    client::{
        device_operator::{
            DeviceCallRequest, DeviceChannelRequest, DeviceResponseMessage,
            DeviceUnsubscribeRequest,
        },
        jsonrpc_method_locator::JsonRpcMethodLocator,
        plugin_manager::{PluginState, PluginStateChangeEvent, PluginStatus},
        thunder_async_client::ThunderAsyncResponse,
        thunder_client::ThunderClient,
        thunder_plugin::ThunderPlugin,
    },
    processors::thunder_device_info::CachedState,
    thunder_state::ThunderState,
};
use ripple_sdk::api::gateway::rpc_gateway_api::JsonRpcApiResponse;
// <pca>
//pub type ThunderHandlerFn = dyn Fn(DeviceCallRequest) + Send + Sync;
// pub type ThunderHandlerFn =
//     dyn Fn(DeviceCallRequest, oneshot::Sender<DeviceResponseMessage>) + Send + Sync;

pub type ThunderHandlerFn =
    dyn Fn(DeviceCallRequest, oneshot::Sender<ThunderAsyncResponse>) + Send + Sync;

// </pca>
pub type ThunderSubscriberFn = dyn Fn(
        Sender<DeviceResponseMessage>,
    ) -> Pin<Box<dyn Future<Output = Option<DeviceResponseMessage>> + Send>>
    + Send
    + Sync;

#[derive(Clone)]
pub struct MockThunderSubscriberfn {
    fnc: Arc<ThunderSubscriberFn>,
}

impl MockThunderSubscriberfn {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(
                Sender<DeviceResponseMessage>,
            ) -> Pin<Box<dyn Future<Output = Option<DeviceResponseMessage>> + Send>>
            + Send
            + Sync
            + 'static,
    {
        Self {
            fnc: Arc::new(Box::new(f)),
        }
    }

    async fn call(&self, sender: Sender<DeviceResponseMessage>) -> Option<DeviceResponseMessage> {
        (self.fnc)(sender).await
    }
}

#[derive(Default, Clone)]
pub struct CustomHandler {
    pub custom_request_handler: HashMap<String, Arc<ThunderHandlerFn>>,
    pub custom_subscription_handler: HashMap<String, MockThunderSubscriberfn>,
}

#[derive(Default)]
pub struct MockThunderController {
    state_subscribers: Vec<mpsc::Sender<DeviceResponseMessage>>,
    plugin_states: HashMap<String, String>,
    custom_handlers: CustomHandler,
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
            let msg = DeviceResponseMessage::call(val);
            mpsc_send_and_log(s, msg, "StateChange").await;
        }
    }

    pub async fn status(&self, callsign: String) -> Vec<PluginStatus> {
        let state = match self.plugin_states.get(&callsign) {
            Some(s) => s.clone(),
            None => String::from("deactivated"),
        };
        Vec::from([PluginStatus { state }])
    }

    pub async fn on_state_change(&mut self, callback: mpsc::Sender<DeviceResponseMessage>) {
        self.state_subscribers.push(callback);
    }

    // <pca>
    // pub async fn handle_thunder_call(&mut self, msg: DeviceCallRequest,) {
    //     let locator = JsonRpcMethodLocator::from_str(&msg.method).unwrap();
    //     let module = locator.module.unwrap();

    //     let (tx, _rx) = oneshot::channel::<DeviceResponseMessage>();

    //     if module == ThunderPlugin::Controller.callsign() {
    //         if locator.method_name == "activate" {
    //             let ps = msg.params.unwrap().as_params();
    //             let psv: Value = serde_json::from_str(ps.as_str()).expect("Message should be JSON");
    //             let cs = psv.get("callsign").unwrap();
    //             self.activate(String::from(cs.as_str().unwrap())).await;
    //             oneshot_send_and_log(tx, EMPTY_RESPONSE, "ActivateAck");
    //         } else if msg.method.starts_with("status") {
    //             let status = self.status(locator.qualifier.unwrap()).await;
    //             let val = serde_json::to_value(status).unwrap_or_default();
    //             let m = DeviceResponseMessage::call(val);
    //             oneshot_send_and_log(tx, m, "StatusReturn");
    //         }
    //     } else if let Some(handler) = self
    //         .custom_handlers
    //         .custom_request_handler
    //         .get(&format!("{}.{}", module, locator.method_name))
    //     {
    //         (handler)(msg.clone());
    //     } else {
    //         println!(
    //             "No mock thunder response found for {}.{}",
    //             module, locator.method_name
    //         );
    //         return;
    //     }
    // }
    pub async fn handle_thunder_call(
        &mut self,
        msg: DeviceCallRequest,
        //device_response_message_tx: mpsc::Sender<DeviceResponseMessage>,
        device_response_message_tx: mpsc::Sender<ThunderAsyncResponse>,
    ) {
        println!(
            "*** _DEBUG: handle_thunder_call: DeviceChannelRequest::Call req received : {:?}",
            msg
        );
        let locator = JsonRpcMethodLocator::from_str(&msg.method).unwrap();
        let module = locator.module.unwrap();

        if module == ThunderPlugin::Controller.callsign() {
            if locator.method_name == "activate" {
                println!("*** _DEBUG: in activate block");
                let ps = msg.params.unwrap().as_params();
                let psv: Value = serde_json::from_str(ps.as_str()).expect("Message should be JSON");
                let cs = psv.get("callsign").unwrap();

                self.activate(String::from(cs.as_str().unwrap())).await;
                mpsc_send_and_log(&device_response_message_tx, empty_response(), "ActivateAck")
                    .await;
            } else if msg.method.starts_with("status") {
                println!("*** _DEBUG: in status block");
                let status = self.status(locator.qualifier.unwrap()).await;
                let val = serde_json::to_value(status).unwrap_or_default();
                //let m = DeviceResponseMessage::call(val);
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
                mpsc_send_and_log(
                    &device_response_message_tx,
                    thunderasyncresp,
                    "StatusReturn",
                )
                .await;
            }
        } else if let Some(handler) = self
            .custom_handlers
            .custom_request_handler
            .get(&format!("{}.{}", module, locator.method_name))
        {
            println!("*** _DEBUG: MockThunderController: handle_thunder_call:  calling custom handler for {}.{}", module, locator.method_name);
            // let (handler_response_tx, handler_response_rx) =
            //     oneshot::channel::<DeviceResponseMessage>();
            // (handler)(msg.clone(), handler_response_tx);

            let (handler_response_tx, handler_response_rx) =
                oneshot::channel::<ThunderAsyncResponse>();
            (handler)(msg.clone(), handler_response_tx);

            if let Ok(response) = handler_response_rx.await {
                println!("*** _DEBUG: MockThunderController: handle_thunder_call:  received response from custom handler for {}.{}", module, locator.method_name);
                mpsc_send_and_log(&device_response_message_tx, response, "CustomResponse").await;
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
    // </pca>

    pub async fn handle_thunder_unsub(&mut self, _msg: DeviceUnsubscribeRequest) {}

    // <pca>
    // pub async fn handle_thunder_sub(
    //     &mut self,
    //     msg: DeviceSubscribeRequest,
    //     handler: mpsc::Sender<DeviceResponseMessage>,
    // ) {
    //     let (tx, _rx) = oneshot::channel::<DeviceResponseMessage>();
    //     if msg.module == "Controller.1" && msg.event_name == "statechange" {
    //         self.on_state_change(handler).await;
    //     } else if let Some(handler_fn) = self
    //         .custom_handlers
    //         .custom_subscription_handler
    //         .get(&format!("{}.{}", msg.module, msg.event_name))
    //     {
    //         let response = handler_fn.call(handler.clone()).await;
    //         if let Some(resp) = response {
    //             mpsc_send_and_log(&handler, resp, "OnStatusChange").await;
    //         }
    //     } else {
    //         println!(
    //             "No mock subscription found for {}.{}",
    //             msg.module, &msg.event_name
    //         );
    //     }
    //     oneshot_send_and_log(tx, EMPTY_RESPONSE, "SubscribeAck");
    // }
    //--------------------------------------------------------------------------------------------------
    // pub async fn handle_thunder_sub(
    //     &mut self,
    //     msg: DeviceSubscribeRequest,
    //     handler: mpsc::Sender<DeviceResponseMessage>,
    // ) {
    //     if msg.module == "Controller.1" && msg.event_name == "statechange" {
    //         self.on_state_change(handler.clone()).await;
    //     } else if let Some(handler_fn) = self
    //         .custom_handlers
    //         .custom_subscription_handler
    //         .get(&format!("{}.{}", msg.module, msg.event_name))
    //     {
    //         let response = handler_fn.call(handler.clone()).await;
    //         if let Some(resp) = response {
    //             mpsc_send_and_log(&handler, resp, "OnStatusChange").await;
    //         }
    //     } else {
    //         println!(
    //             "No mock subscription found for {}.{}",
    //             msg.module, &msg.event_name
    //         );
    //         mpsc_send_and_log(&handler, empty_response(), "SubscribeAck").await;
    //     }
    //     mpsc_send_and_log(&handler, EMPTY_RESPONSE, "SubscribeAck").await;
    // }
    //--------------------------------------------------------------------------------------------------
    // </pca>

    // <pca>
    //pub fn start() -> mpsc::Sender<DeviceChannelRequest> {
    pub fn start() -> (
        mpsc::Sender<DeviceChannelRequest>,
        mpsc::Receiver<ThunderAsyncResponse>,
    ) {
        // </pca>
        MockThunderController::start_with_custom_handlers(None)
    }

    // <pca>
    // pub fn start_with_custom_handlers(
    //     custom_handlers: Option<CustomHandler>,
    // ) -> mpsc::Sender<DeviceChannelRequest> {
    //     let (client_tx, mut client_rx) = mpsc::channel(32);
    //     let (resp_tx, _resp_rx): (
    //         mpsc::Sender<DeviceResponseMessage>,
    //         mpsc::Receiver<DeviceResponseMessage>,
    //     ) = mpsc::channel(32);
    //     tokio::spawn(async move {
    //         let mut mock_controller = MockThunderController::default();
    //         if let Some(ch) = custom_handlers {
    //             mock_controller.custom_handlers = ch;
    //         }
    //         while let Some(tm) = client_rx.recv().await {
    //             match tm {
    //                 DeviceChannelRequest::Call(msg) => {
    //                     mock_controller.handle_thunder_call(msg).await;
    //                 }
    //                 DeviceChannelRequest::Subscribe(msg) => {
    //                     mock_controller
    //                         .handle_thunder_sub(msg, resp_tx.clone())
    //                         .await;
    //                 }
    //                 DeviceChannelRequest::Unsubscribe(msg) => {
    //                     mock_controller.handle_thunder_unsub(msg).await;
    //                 }
    //             }
    //         }
    //     });
    //     client_tx
    // }

    // pub fn start_with_custom_handlers(
    //     custom_handlers: Option<CustomHandler>,
    // ) -> (
    //     mpsc::Sender<DeviceChannelRequest>,
    //     mpsc::Receiver<DeviceResponseMessage>,
    // ) {
    //     println!("*** _DEBUG: start_with_custom_handlers: invoked");
    //     let (device_channel_request_tx, mut device_channel_request_rx) = mpsc::channel(32);
    //     //let (device_response_message_tx, mut device_response_message_rx) = mpsc::channel(32);

    //     let (device_response_message_tx, mut device_response_message_rx) =
    //         mpsc::channel::<ThunderAsyncResponse>(32);

    //     tokio::spawn(async move {
    //         let mut mock_controller = MockThunderController::default();
    //         if let Some(ch) = custom_handlers {
    //             mock_controller.custom_handlers = ch;
    //         }
    //         while let Some(device_channel_request) = device_channel_request_rx.recv().await {
    //             println!("*** _DEBUG: MockThunderController: start_with_custom_handlers: received request: {:?}", device_channel_request);

    //             match device_channel_request {
    //                 DeviceChannelRequest::Call(msg) => {
    //                     println!("*** _DEBUG: start_with_custom_handlers: DeviceChannelRequest::Call req received : {:?}", msg);
    //                     mock_controller
    //                         .handle_thunder_call(msg, device_response_message_tx.clone())
    //                         .await;
    //                 }
    //                 DeviceChannelRequest::Subscribe(msg) => {
    //                     println!("*** _DEBUG: start_with_custom_handlers: DeviceChannelRequest::Subscribe req received : {:?}", msg);
    //                     // mock_controller
    //                     //     .handle_thunder_sub(msg, device_response_message_tx.clone())
    //                     //     .await;
    //                 }
    //                 DeviceChannelRequest::Unsubscribe(msg) => {
    //                     //dmock_controller.handle_thunder_unsub(msg).await;
    //                 }
    //             }
    //         }
    //     });

    //     (device_channel_request_tx, device_response_message_rx)
    // }

    pub fn start_with_custom_handlers(
        custom_handlers: Option<CustomHandler>,
    ) -> (
        mpsc::Sender<DeviceChannelRequest>,
        mpsc::Receiver<ThunderAsyncResponse>,
    ) {
        println!("*** _DEBUG: start_with_custom_handlers: invoked");
        let (device_channel_request_tx, mut device_channel_request_rx) = mpsc::channel(32);
        //let (device_response_message_tx, mut device_response_message_rx) = mpsc::channel(32);

        let (device_response_message_tx, mut device_response_message_rx) =
            mpsc::channel::<ThunderAsyncResponse>(32);

        tokio::spawn(async move {
            let mut mock_controller = MockThunderController::default();
            if let Some(ch) = custom_handlers {
                mock_controller.custom_handlers = ch;
            }
            while let Some(device_channel_request) = device_channel_request_rx.recv().await {
                println!("*** _DEBUG: MockThunderController: start_with_custom_handlers: received request: {:?}", device_channel_request);

                match device_channel_request {
                    DeviceChannelRequest::Call(msg) => {
                        println!("*** _DEBUG: start_with_custom_handlers: DeviceChannelRequest::Call req received : {:?}", msg);
                        mock_controller
                            .handle_thunder_call(msg, device_response_message_tx.clone())
                            .await;
                    }
                    DeviceChannelRequest::Subscribe(msg) => {
                        println!("*** _DEBUG: start_with_custom_handlers: DeviceChannelRequest::Subscribe req received : {:?}", msg);
                        // mock_controller
                        //     .handle_thunder_sub(msg, device_response_message_tx.clone())
                        //     .await;
                    }
                    DeviceChannelRequest::Unsubscribe(msg) => {
                        //dmock_controller.handle_thunder_unsub(msg).await;
                    }
                }
            }
        });

        (device_channel_request_tx, device_response_message_rx)
    }
    // </pca>

    /**
     * Creates state object that points to a mock thunder controller.
     * Pass in the custom thunder handlers to mock the thunder responses
     * Returns the state and a receiver which can be used to listen to responses that
     * come back from the extension
     */
    // <pca>
    // pub fn state_with_mock(custom_thunder: Option<CustomHandler>) -> CachedState {
    //     let _s_thunder = MockThunderController::start_with_custom_handlers(custom_thunder);
    //     let thunder_client = ThunderClient::mock();
    //     let extn_client = MockExtnClient::client();
    //     let thunder_state = ThunderState::new(extn_client, thunder_client);
    //     CachedState::new(thunder_state)
    // }
    pub fn state_with_mock(custom_thunder: Option<CustomHandler>) -> CachedState {
        println!("*** _DEBUG: state_with_mock: invoked");
        let (device_channel_request_tx, device_response_message_rx) =
            MockThunderController::start_with_custom_handlers(custom_thunder);

        let thunder_client =
            ThunderClient::start_mock(device_channel_request_tx, device_response_message_rx);
        let extn_client = ExtnClient::new_main();
        let thunder_state = ThunderState::new(extn_client, thunder_client);
        //(CachedState::new(thunder_state), device_response_message_rx)
        CachedState::new(thunder_state)
    }
    // </pca>

    pub fn get_thunder_state_mock_with_handler(_handler: Option<CustomHandler>) -> ThunderState {
        let thunder_client = ThunderClient::mock();

        let extn_client = MockExtnClient::client();
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
