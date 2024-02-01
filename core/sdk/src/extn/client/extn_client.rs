// Copyright 2023 Comcast Cable Communications Management, LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

use async_channel::{bounded, Receiver as CReceiver, Sender as CSender};
use chrono::Utc;
use log::{debug, error, info, trace};
use tokio::sync::{
    mpsc::Sender as MSender,
    oneshot::{self, Sender as OSender},
};

use crate::{
    api::{
        context::{ActivationStatus, RippleContext, RippleContextUpdateRequest},
        device::device_request::{InternetConnectionStatus, TimeZone},
        manifest::extn_manifest::ExtnSymbol,
    },
    extn::{
        extn_client_message::{ExtnMessage, ExtnPayloadProvider, ExtnResponse},
        extn_id::ExtnId,
        ffi::ffi_message::CExtnMessage,
    },
    framework::{ripple_contract::RippleContract, RippleResponse},
    utils::{error::RippleError, extn_utils::ExtnStackSize},
};

use super::{
    extn_processor::{ExtnEventProcessor, ExtnRequestProcessor},
    extn_sender::ExtnSender,
};

/// Defines the SDK Client implementation of the Inter Extension communication.
/// # Overview
/// Core objective for the Extn client is to provide a reliable and robust communication channel between the  `Main` and its extensions. There are challenges when using Dynamic Linked libraries which needs to be carefully handled for memory, security and reliability. `Client` is built into the `core/sdk` for a better Software Delivery and Operational(SDO) performance.
/// Each client within an extension contains the below fields
/// 1. `reciever` - Async Channel Receiver which is connected to the processors for handling incoming messages
/// 2. `sender` - Async Channel Sender to send the request back to `Main` application
/// 3. `extn_sender_map` - Contains a list of senders based on a short [ExtnCapability] string which can be used to send  the request to other extensions.
/// 4. `response_processors` - Map of response processors which are used for Response processor handling
/// 5. `request_processors` - Map of request processors used for Request process handling
/// 6. `event_processors` - Map of event processors used for Event Process handling
///

#[repr(C)]
#[derive(Clone, Debug)]
pub struct ExtnClient {
    receiver: CReceiver<CExtnMessage>,
    sender: ExtnSender,
    extn_sender_map: Arc<RwLock<HashMap<String, CSender<CExtnMessage>>>>,
    contract_map: Arc<RwLock<HashMap<String, String>>>,
    response_processors: Arc<RwLock<HashMap<String, OSender<ExtnMessage>>>>,
    request_processors: Arc<RwLock<HashMap<String, MSender<ExtnMessage>>>>,
    event_processors: Arc<RwLock<HashMap<String, Vec<MSender<ExtnMessage>>>>>,
    ripple_context: Arc<RwLock<RippleContext>>,
}

///
/// This is a utility method simply inserts the key and value to a given Arc<RwLock<HashMap<K,V>>>
///
fn add_stream_processor<P>(id: String, context: P, map: Arc<RwLock<HashMap<String, P>>>) {
    println!("**** extn_client: add_stream_processor id: {}", id);
    let mut processor_state = map.write().unwrap();
    processor_state.insert(id, context);
    println!(
        "**** extn_client: add_stream_processor: processor_state: {:?}",
        processor_state.len()
    );
}

///
/// Utility method which adds a key and list value to a given Arc<RWlock<HashMap<K,Vec<V>>>>
/// If an entry is already present it adds to the Vec if not creates a new one
///
fn add_vec_stream_processor<P>(id: String, context: P, map: Arc<RwLock<HashMap<String, Vec<P>>>>) {
    println!("**** extn_client: add_vec_stream_processor {}", id);
    let mut processor_state = map.write().unwrap();
    if let std::collections::hash_map::Entry::Vacant(e) = processor_state.entry(id.clone()) {
        e.insert(vec![context]);
        println!("**** pushed vec");
    } else {
        println!("**** pushed context");
        processor_state.get_mut(&id).unwrap().push(context)
    }

    println!(
        "**** extn_client: add_vec_stream_processor: processor_state: {:?}",
        processor_state.len()
    );
    println!(
        "**** extn_client: add_vec_stream_processor: processor_state len for {}: {:?}",
        id,
        processor_state.get(&id).map_or(0, |v| v.len())
    );
}

///
/// Utility function which adds a single processor used for response processors
///
fn add_single_processor<P>(id: String, processor: P, map: Arc<RwLock<HashMap<String, P>>>) {
    println!("**** extn_client: add_single_processor: id: {:?}", id);
    // println!("**** add_single_processor: context: {:?}", processor);
    let mut processor_state = map.write().unwrap();
    processor_state.insert(id, processor);
}

impl ExtnClient {
    /// Creates a new ExtnClient to be used by Extensions during initialization.
    ///
    /// # Arguments
    /// `receiver` - Async Channel Receiver provided by the `Main` Application for IEC
    ///
    /// `sender` - [ExtnSender] object provided by `Main` Application with a unique [ExtnCapability]
    pub fn new(receiver: CReceiver<CExtnMessage>, sender: ExtnSender) -> ExtnClient {
        ExtnClient {
            receiver,
            sender,
            extn_sender_map: Arc::new(RwLock::new(HashMap::new())),
            contract_map: Arc::new(RwLock::new(HashMap::new())),
            response_processors: Arc::new(RwLock::new(HashMap::new())),
            request_processors: Arc::new(RwLock::new(HashMap::new())),
            event_processors: Arc::new(RwLock::new(HashMap::new())),
            ripple_context: Arc::new(RwLock::new(RippleContext::default())),
        }
    }

    /// Adds a new request processor reference to the internal map of processors
    ///
    /// Uses the capability provided by the Processor for registration
    ///
    /// Also starts the thread in the processor to accept incoming requests.
    pub fn add_request_processor(&mut self, mut processor: impl ExtnRequestProcessor) -> bool {
        println!(
            "**** ExtnClient: add_request_processor {:?}",
            processor.contract().as_clear_string()
        );
        let contracts = if let Some(multiple_contracts) = processor.fulfills_mutiple() {
            multiple_contracts
        } else {
            vec![processor.contract()]
        };

        println!(
            "**** ExtnClient: add_request_processor contracts: {:?}",
            contracts
        );

        let contracts_supported: Vec<RippleContract> = contracts
            .into_iter()
            .filter(|contract| self.sender.check_contract_fulfillment(contract.clone()))
            .collect();

        println!(
            "**** ExtnClient: add_request_processor contracts_supported: {:?}",
            contracts_supported
        );

        contracts_supported.iter().for_each(|contract| {
            let processor_string: String = contract.as_clear_string();
            println!(
                "**** ExtnClient: adding stream processor {}",
                processor_string
            );
            println!(
                "**** ExtnClient: add_request_processor: add_stream_processor - processor.sender(): {:?}",
                processor.sender()
            );

            add_stream_processor(
                processor_string,
                processor.sender(),
                self.request_processors.clone(),
            );

        });

        // Dont add and start a request processor if there is no contract fulfillment
        if !contracts_supported.is_empty() {
            tokio::spawn(async move {
                println!(
                    "**** starting request processor green tokio thread for {:?}",
                    contracts_supported
                );
                processor.run().await;
            });
            return true;
        }
        false
    }

    /// Adds a new event processor reference to the internal map of processors
    ///
    /// Uses the capability provided by the Processor for registration
    ///
    /// Also starts the thread in the processor to accept incoming events.
    pub fn add_event_processor(&mut self, mut processor: impl ExtnEventProcessor) {
        println!(
            "**** extn_client: add_event_processor {}",
            processor.contract().as_clear_string()
        );
        add_vec_stream_processor(
            processor.contract().as_clear_string(),
            processor.sender(),
            self.event_processors.clone(),
        );
        tokio::spawn(async move { processor.run().await });
    }

    /// Removes an event processor reference on the internal map of processors
    pub fn cleanup_event_stream(&mut self, capability: ExtnId) {
        println!("**** cleanup_event_stream {}", capability.to_string());
        Self::cleanup_vec_stream(capability.to_string(), None, self.event_processors.clone());
        println!(
            "**** cleanup_event_stream len after cleanup vec: {}",
            self.event_processors.clone().read().unwrap().len()
        );
    }

    /// Used mainly by `Main` application to add senders of the extensions for IEC
    pub fn add_sender(&mut self, id: ExtnId, symbol: ExtnSymbol, sender: CSender<CExtnMessage>) {
        println!("**** extn_client: add_sender {:?}", id.clone());
        println!(
            "**** extn_client: self.sender.get_cap() {:?}",
            self.sender.get_cap()
        );
        println!(
            "**** extn_client: self.sender.get_cap().is_main() {:?}",
            self.sender.get_cap().is_main()
        );
        if !self.sender.get_cap().is_main() {
            println!("**** extn_client: Senders cannot be added in an extension");
            return;
        }
        let id = id.to_string();
        {
            let mut sender_map = self.extn_sender_map.write().unwrap();
            sender_map.insert(id.clone(), sender);
        }
        {
            let mut map = HashMap::new();

            for contract in symbol.fulfills {
                match RippleContract::from_manifest(&contract) {
                    Some(v) => {
                        let ripple_contract_string = v.as_clear_string();
                        println!("**** {} will fulfill {}", id, ripple_contract_string);
                        let _ = map.insert(ripple_contract_string, id.clone());
                    }
                    None => println!("**** Unknown contract {}", contract),
                }
            }
            let mut contract_map = self.contract_map.write().unwrap();
            contract_map.extend(map);
        }
    }

    pub fn get_other_senders(&self) -> Vec<CSender<CExtnMessage>> {
        println!("**** extn_client: getting other senders");
        self.extn_sender_map
            .read()
            .unwrap()
            .iter()
            .map(|(_, v)| v)
            .cloned()
            .collect()
    }

    /// Called once per client initialization this is a blocking method. Use a spawned thread to call this method
    pub async fn initialize(&self) {
        println!("**** extn_client: Starting initialize");
        let receiver = self.receiver.clone();
        while let Ok(c_message) = receiver.recv().await {
            let latency = Utc::now().timestamp_millis() - c_message.ts;

            if latency > 1000 {
                println!("IEC Latency {:?}", c_message);
            }
            let message_result: Result<ExtnMessage, RippleError> = c_message.clone().try_into();
            println!(
                "**** extn_client: initialize: message: {:?}",
                message_result
            );
            if message_result.is_err() {
                println!("invalid message {:?}", c_message);
            }
            let message = message_result.unwrap();
            println!(
                "** extn_client: receiving message latency={} msg={:?}",
                latency, message
            );
            if message.payload.is_response() {
                Self::handle_single(message, self.response_processors.clone());
            } else if message.payload.is_event() {
                let is_main = self.sender.get_cap().is_main();
                if !is_main {
                    if let Some(context) = RippleContext::is_ripple_context(&message.payload) {
                        println!(
                            "**** Received ripple context in {} message: {:?}",
                            self.sender.get_cap().to_string(),
                            message
                        );
                        {
                            let mut ripple_context = self.ripple_context.write().unwrap();
                            ripple_context.deep_copy(context);
                        }
                    }
                }

                Self::handle_vec_stream(message, self.event_processors.clone());
            } else {
                let current_cap = self.sender.get_cap();
                let target_contract = message.clone().target;
                println!(
                    "**** extn_client: initialize: target_contract: {:?}",
                    target_contract
                );
                if current_cap.is_main() {
                    if let Some(request) =
                        RippleContextUpdateRequest::is_ripple_context_update(&message.payload)
                    {
                        println!("**** extn_client: updating context");
                        self.context_update(request);
                    }
                    // Forward the message to an extn sender
                    else if let Some(sender) = self.get_extn_sender_with_contract(target_contract)
                    {
                        println!("**** extn_client: intialize: get_extn_sender_with_contract");
                        let mut new_message = message.clone();
                        if new_message.callback.is_none() {
                            // before forwarding check if the requestor needs to be added as callback
                            let req_sender =
                                self.get_extn_sender_with_extn_id(&message.requestor.to_string());

                            if let Some(sender) = req_sender {
                                let _ = new_message.callback.insert(sender);
                            }
                        }

                        tokio::spawn(async move {
                            if let Err(e) = sender.try_send(new_message.into()) {
                                error!("Error forwarding request {:?}", e)
                            }
                        });
                    } else {
                        // could be main contract
                        if !Self::handle_stream(message.clone(), self.request_processors.clone()) {
                            println!("**** extn_client: initialize: handle_stream - false");
                            self.handle_no_processor_error(message);
                        }
                    }
                } else if !Self::handle_stream(message.clone(), self.request_processors.clone()) {
                    println!("**** extn_client: initialize: handle_stream - false");
                    self.handle_no_processor_error(message);
                }
            }
        }

        println!("**** Initialize Ended Abruptly");
    }

    pub fn context_update(&self, request: RippleContextUpdateRequest) {
        println!("**** extn_client: updating context");
        let current_cap = self.sender.get_cap();
        if !current_cap.is_main() {
            error!("**** Updating context is not allowed outside main");
        }

        {
            let mut ripple_context = self.ripple_context.write().unwrap();
            ripple_context.update(request)
        }
        let new_context = { self.ripple_context.read().unwrap().clone() };
        let message = new_context.get_event_message();
        let c_message: CExtnMessage = message.clone().into();
        {
            let senders = self.get_other_senders();
            for sender in senders {
                let send_res = sender.try_send(c_message.clone());
                trace!("Send to other client result: {:?}", send_res);
            }
        }
        Self::handle_vec_stream(message, self.event_processors.clone());
    }

    fn handle_no_processor_error(&self, message: ExtnMessage) {
        println!("**** extn_client: handling no processor error");
        let req_sender = self.get_extn_sender_with_extn_id(&message.requestor.to_string());
        println!(
            "**** extn_client: handling no processor error req_sender: {:?}",
            req_sender
        );

        if let Ok(resp) = message.get_response(ExtnResponse::Error(RippleError::ProcessorError)) {
            println!(
                "**** extn_client: handling no processor error resp: {:?}",
                resp
            );
            if self.sender.respond(resp.into(), req_sender).is_err() {
                println!("**** Couldnt send no processor response");
            }
        }
    }

    fn handle_single(
        msg: ExtnMessage,
        processor: Arc<RwLock<HashMap<String, OSender<ExtnMessage>>>>,
    ) {
        println!("**** extn_client: handling single");
        let id_c = msg.id.clone();
        let processor_result = {
            let mut processors = processor.write().unwrap();
            processors.remove(&id_c)
        };

        if let Some(processor_result) = processor_result {
            tokio::spawn(async move {
                if let Err(e) = processor_result.send(msg) {
                    error!("Error sending the response back {:?}", e);
                }
            });
        } else {
            error!("No response processor for {:?}", msg);
        }
    }

    fn handle_stream(
        msg: ExtnMessage,
        processor: Arc<RwLock<HashMap<String, MSender<ExtnMessage>>>>,
    ) -> bool {
        println!("**** extn_client: handling stream");
        let id_c: String = msg.target.as_clear_string();

        let v = {
            let processors = processor.read().unwrap();
            processors.get(&id_c).cloned()
        };
        if let Some(sender) = v {
            tokio::spawn(async move {
                if let Err(e) = sender.send(msg.clone()).await {
                    println!("**** Error sending the response back {:?}", e);
                }
            });
            true
        } else {
            println!("**** No Request Processor for {} {:?}", id_c, msg);
            false
        }
    }

    fn handle_vec_stream(
        msg: ExtnMessage,
        processor: Arc<RwLock<HashMap<String, Vec<MSender<ExtnMessage>>>>>,
    ) {
        println!("**** extn_client: handling vec stream");
        let id_c = msg.target.as_clear_string();
        let mut gc_sender_indexes: Vec<usize> = Vec::new();
        let mut sender: Option<MSender<ExtnMessage>> = None;
        let read_processor = processor.clone();
        {
            let processors = read_processor.read().unwrap();
            let v = processors.get(&id_c).cloned();
            if let Some(v) = v {
                for (index, s) in v.iter().enumerate() {
                    if !s.is_closed() {
                        let _ = sender.insert(s.clone());
                        break;
                    } else {
                        gc_sender_indexes.push(index);
                    }
                }
            }
        };
        println!(
            "**** extn_client: handling vec stream sender {:?}",
            sender.is_some()
        );
        if let Some(sender) = sender {
            tokio::spawn(async move {
                println!("**** extn_client: send the message");
                if let Err(e) = sender.clone().try_send(msg) {
                    println!("Error sending the response back {:?}", e);
                }
            });
        } else if RippleContext::is_ripple_context(&msg.payload).is_none() {
            // Not every extension will have a context listener
            println!("No Event Processor for {:?}", msg);
        }

        Self::cleanup_vec_stream(id_c, None, processor);
    }

    fn cleanup_vec_stream(
        id_c: String,
        gc_sender_indexes: Option<Vec<usize>>,
        processor: Arc<RwLock<HashMap<String, Vec<MSender<ExtnMessage>>>>>,
    ) {
        println!("**** extn_client: cleaning up vec stream");
        let processors = processor.read().unwrap();
        let avail = processors.keys();
        println!(
            "**** extn_client: removing {}, available processors {:?}",
            id_c, avail
        );
        println!(
            "**** extn_client: removing vec : {:?}",
            processor.read().unwrap().get(&id_c)
        );
        let indices = match gc_sender_indexes {
            Some(i) => Some(i),
            None => processor.read().unwrap().get(&id_c).map(|v| {
                // Print is_closed values directly
                for sender in v.iter() {
                    println!("**** Sender is_closed: {}", sender.is_closed());
                }

                v.iter()
                    .enumerate()
                    .filter(|(_, x)| x.is_closed())
                    .map(|(i, _)| i)
                    .collect()
            }),
        };
        println!(
            "**** extn_client: cleaning up vec stream indices to remove: {:?}",
            indices
        );

        println!(
            "**** extn_client: cleaning up vec stream indices to remove: {:?}",
            indices
        );
        if let Some(indices) = indices {
            if !indices.is_empty() {
                let mut gc_cleanup = processor.write().unwrap();
                if let Some(sender_list) = gc_cleanup.get_mut(&id_c) {
                    for index in indices {
                        let r = sender_list.remove(index);
                        drop(r);
                    }
                }
            }
        }
    }

    fn get_extn_sender_with_contract(
        &self,
        contract: RippleContract,
    ) -> Option<CSender<CExtnMessage>> {
        println!(
            "**** extn_client: getting extn sender with contract: {:?}",
            contract
        );
        let contract_str: String = contract.as_clear_string();
        let id = {
            self.contract_map
                .read()
                .unwrap()
                .get(&contract_str)
                .cloned()
        };
        if let Some(extn_id) = id {
            return self.get_extn_sender_with_extn_id(&extn_id);
        }

        None
    }

    fn get_extn_sender_with_extn_id(&self, id: &str) -> Option<CSender<CExtnMessage>> {
        println!("**** extn_client: get_extn_sender_with_extn_id");
        return self.extn_sender_map.read().unwrap().get(id).cloned();
    }

    /// Critical method used by request processors to send response message back to the requestor
    /// # Arguments
    /// `req` - [ExtnMessage] request object
    /// `response` - [ExtnResponse] object
    pub async fn respond(
        &mut self,
        req: ExtnMessage,
        response: ExtnResponse,
    ) -> Result<(), RippleError> {
        println!("**** extn_client: responding");
        if !req.payload.is_request() {
            Err(RippleError::InvalidInput)
        } else {
            let msg = req.get_response(response).unwrap();
            self.send_message(msg).await
        }
    }

    /// Method used for sending a fully build [ExtnMessage]
    /// # Arguments
    /// `msg` - [ExtnMessage]
    pub async fn send_message(&mut self, msg: ExtnMessage) -> RippleResponse {
        println!("**** extn_client: sending message");
        self.sender.respond(
            msg.clone().into(),
            self.get_extn_sender_with_extn_id(&msg.requestor.to_string()),
        )
    }

    /// Critical method used by event processors to emit event back to the requestor
    /// # Arguments
    /// `msg` - [ExtnMessage] event object
    pub fn event(&mut self, event: impl ExtnPayloadProvider) -> Result<(), RippleError> {
        println!("**** extn_client: event: event {:?}", event.clone());
        let other_sender = self.get_extn_sender_with_contract(event.get_contract());
        self.sender.send_event(event, other_sender)
    }

    /// Request method which accepts a impl [ExtnPayloadProvider] and uses the capability provided by the trait to send the request.
    /// As part of the send process it adds a callback to asynchronously respond back to the caller when the response does get
    /// received.
    ///
    /// # Arguments
    /// `payload` - impl [ExtnPayloadProvider]
    pub async fn request(
        &mut self,
        payload: impl ExtnPayloadProvider,
    ) -> Result<ExtnMessage, RippleError> {
        println!("**** extn_client: request: payload {:?}", payload.clone());
        let id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        add_single_processor(id.clone(), tx, self.response_processors.clone());

        let other_sender = self.get_extn_sender_with_contract(payload.get_contract());
        self.sender.send_request(id, payload, other_sender, None)?;
        if let Ok(r) = rx.await {
            return Ok(r);
        }

        Err(RippleError::ExtnError)
    }

    /// Request method which accepts a impl [ExtnPayloadProvider] and uses the capability provided by the trait to send the request.
    /// As part of the send process it adds a callback to asynchronously respond back to the caller when the response does get
    /// received. This method can be called synchrnously with a timeout
    ///
    /// # Arguments
    /// `payload` - impl [ExtnPayloadProvider]
    pub async fn standalone_request<T: ExtnPayloadProvider>(
        &mut self,
        payload: impl ExtnPayloadProvider,
        timeout_in_msecs: u64,
    ) -> Result<T, RippleError> {
        println!("**** extn_client: standalone requesting");
        let id = uuid::Uuid::new_v4().to_string();
        let (tx, tr) = bounded(2);
        let other_sender = self.get_extn_sender_with_contract(payload.get_contract());
        self.sender
            .send_request(id, payload, other_sender, Some(tx))?;
        match tokio::time::timeout(Duration::from_millis(timeout_in_msecs), tr.recv()).await {
            Ok(Ok(cmessage)) => {
                debug!("** extn_client: receiving message msg={:?}", cmessage);
                let message: Result<ExtnMessage, RippleError> = cmessage.try_into();

                if let Ok(message) = message {
                    if let Some(v) = message.payload.extract() {
                        return Ok(v);
                    } else {
                        return Err(RippleError::ParseError);
                    }
                }
            }
            Ok(Err(_)) => error!("Invalid message"),
            Err(_) => {
                error!("Channel disconnected");
            }
        }

        Err(RippleError::InvalidOutput)
    }

    /// Request method which accepts a impl [ExtnPayloadProvider] and uses the capability provided by the trait to send the request.
    /// This method doesnt provide a response it just provides a result after a successful send. Useful for transient requests from
    /// protocols which do not need a single point of request and response.
    ///
    /// # Arguments
    /// `payload` - impl [ExtnPayloadProvider]
    pub fn request_transient(&mut self, payload: impl ExtnPayloadProvider) -> RippleResponse {
        println!("**** extn_client: request_transient");
        let id = uuid::Uuid::new_v4().to_string();
        let other_sender = self.get_extn_sender_with_contract(payload.get_contract());
        self.sender.send_request(id, payload, other_sender, None)
    }

    pub fn get_stack_size(&self) -> Option<ExtnStackSize> {
        self.get_config("stack_size")
            .map(|v| ExtnStackSize::from(v.as_str()))
    }

    /// Method to get configurations on the manifest per extension
    pub fn get_config(&self, key: &str) -> Option<String> {
        println!("**** extn_client: getting config");
        self.sender.get_config(key)
    }

    /// Method to get configurations on the manifest per extension
    pub fn get_bool_config(&self, key: &str) -> bool {
        if let Some(s) = self.sender.get_config(key) {
            if let Ok(v) = s.parse() {
                return v;
            }
        }
        false
    }

    /// Method to send event to an extension based on its Id
    pub fn send_event_with_id(&self, id: &str, event: impl ExtnPayloadProvider) -> RippleResponse {
        println!("**** extn_client: send_event_with_id");
        if let Some(sender) = self.get_extn_sender_with_extn_id(id) {
            self.sender.send_event(event, Some(sender))
        } else {
            Err(RippleError::SendFailure)
        }
    }

    /// Method to get configurations on the manifest per extension
    pub fn check_contract_fulfillment(&self, contract: RippleContract) -> bool {
        println!("**** extn_client: check_contract_fulfillment");
        self.sender.check_contract_fulfillment(contract)
    }

    // Method to check if contract is permitted
    pub fn check_contract_permitted(&self, contract: RippleContract) -> bool {
        println!("**** extn_client: check_contract_permitted");
        self.sender.check_contract_permission(contract)
    }

    pub fn has_token(&self) -> bool {
        let ripple_context = self.ripple_context.read().unwrap();
        matches!(
            ripple_context.activation_status.clone(),
            ActivationStatus::AccountToken(_)
        )
    }

    pub fn get_activation_status(&self) -> ActivationStatus {
        let ripple_context = self.ripple_context.read().unwrap();
        ripple_context.activation_status.clone()
    }

    pub fn has_internet(&self) -> bool {
        let ripple_context = self.ripple_context.read().unwrap();
        matches!(
            ripple_context.internet_connectivity,
            InternetConnectionStatus::FullyConnected | InternetConnectionStatus::LimitedInternet
        )
    }

    pub fn get_timezone(&self) -> Option<TimeZone> {
        let ripple_context = self.ripple_context.read().unwrap();
        Some(ripple_context.time_zone.clone())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        api::session::AccountSession,
        extn::{
            client::extn_processor::{
                tests::{MockEventProcessor, MockRequestProcessor, MockState},
                DefaultExtnStreamer, ExtnStreamProcessor,
            },
            extn_client_message::{ExtnPayload, ExtnRequest},
            extn_id::{ExtnClassId, ExtnId},
        },
        utils::mock_utils::{get_mock_extn_client, MockContext, MockEvent, MockRequest},
    };
    use async_channel::unbounded;
    use log::Level;
    use rstest::rstest;
    use std::collections::HashMap;
    use testing_logger::{self, validate};
    use tokio::{sync::mpsc, time::Duration};

    #[cfg(test)]
    pub trait Mockable {
        fn mock() -> ExtnClient
        where
            Self: Sized;
        fn mock_with_params(
            id: ExtnId,
            context: Vec<String>,
            fulfills: Vec<String>,
            config: Option<HashMap<String, String>>,
        ) -> ExtnClient
        where
            Self: Sized;
    }

    #[cfg(test)]
    impl Mockable for ExtnClient {
        fn mock_with_params(
            id: ExtnId,
            context: Vec<String>,
            fulfills: Vec<String>,
            config: Option<HashMap<String, String>>,
        ) -> ExtnClient {
            let (tx, rx) = unbounded();
            let mock_sender = ExtnSender::new(
                tx,
                ExtnId::get_main_target("main".into()),
                vec!["context".to_string()],
                vec!["fulfills".to_string()],
                Some(HashMap::new()),
            );
            ExtnClient::new(rx, mock_sender)
        }

        fn mock() -> ExtnClient {
            let (tx, rx) = unbounded();
            let mock_sender = ExtnSender::new(
                tx,
                ExtnId::get_main_target("main".into()),
                vec!["context".to_string()],
                vec!["fulfills".to_string()],
                Some(HashMap::new()),
            );
            ExtnClient::new(rx, mock_sender)
        }
    }

    #[test]
    fn test_add_stream_processor() {
        let extn_client = ExtnClient::mock();
        let processor = MockRequestProcessor {
            state: MockState {
                client: extn_client.clone(),
            },
            streamer: DefaultExtnStreamer::new(),
        };

        add_stream_processor(
            processor.contract().as_clear_string(),
            processor.sender(),
            extn_client.request_processors.clone(),
        );

        assert!(extn_client.request_processors.read().unwrap().len() == 1);
    }

    #[test]
    fn test_add_vec_stream_processor() {
        let extn_client = ExtnClient::mock();
        let processor = MockEventProcessor {
            state: MockState {
                client: extn_client.clone(),
            },
            streamer: DefaultExtnStreamer::new(),
        };
        let id = processor.contract().as_clear_string();

        add_vec_stream_processor(
            id.clone(),
            processor.sender(),
            extn_client.event_processors.clone(),
        );

        assert!(extn_client.event_processors.read().unwrap().len() == 1);
        assert_eq!(
            extn_client
                .event_processors
                .read()
                .unwrap()
                .get(&id)
                .map(|v| v.len()),
                Some(1),
            "Assertion failed: Vec<MSender<ExtnMessage> in event_processors map does not have the expected length"
        );
    }

    #[test]
    fn test_add_single_processor() {
        let extn_client = ExtnClient::mock();
        let id = uuid::Uuid::new_v4().to_string();
        let (tx, _rx) = oneshot::channel();
        add_single_processor(id, tx, extn_client.response_processors.clone());

        assert!(extn_client.response_processors.read().unwrap().len() == 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_add_request_processor() {
        let mut extn_client = ExtnClient::mock();
        let processor = MockRequestProcessor {
            state: MockState {
                client: extn_client.clone(),
            },
            streamer: DefaultExtnStreamer::new(),
        };

        let res = extn_client.add_request_processor(processor);
        assert!(res);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_add_event_processor() {
        let mut extn_client = ExtnClient::mock();
        let processor = MockEventProcessor {
            state: MockState {
                client: extn_client.clone(),
            },
            streamer: DefaultExtnStreamer::new(),
        };
        let id = processor.contract().as_clear_string();
        extn_client.add_event_processor(processor);

        assert!(extn_client.event_processors.read().unwrap().len() == 1);
        assert_eq!(
            extn_client
                .event_processors
                .read()
                .unwrap()
                .get(&id)
                .map(|v| v.len()),
            Some(1),
            "Assertion failed: Vec<MSender<ExtnMessage> in event_processors map does not have the expected length"
        );
    }

    #[rstest(cap, expected_len,
        case(ExtnId::get_main_target("main".into()), 1),
        case(ExtnId::new_channel(ExtnClassId::Internal, "test".into()), 0)
    )]
    #[tokio::test(flavor = "multi_thread")]
    async fn test_add_senders(cap: ExtnId, expected_len: usize) {
        let extn_client = get_mock_extn_client(cap.clone());
        assert_eq!(
            extn_client.get_other_senders().len(),
            0,
            "Assertion failed: extn_client.get_other_senders() does not have the expected length"
        );

        let (s, _receiver) = unbounded();
        extn_client.clone().add_sender(
            cap,
            ExtnSymbol {
                id: "id".to_string(),
                uses: vec!["uses".to_string()],
                fulfills: vec!["fulfills".to_string()],
                config: None,
            },
            s,
        );

        assert_eq!(
            extn_client.get_other_senders().len(),
            expected_len,
            "Assertion failed: extn_client.get_other_senders() does not have the expected length"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_other_senders() {
        let extn_client = ExtnClient::mock();
        assert_eq!(
            extn_client.get_other_senders().len(),
            0,
            "Assertion failed: extn_client.get_other_senders() does not have the expected length"
        );

        let (s, _receiver) = unbounded();
        extn_client.clone().add_sender(
            ExtnId::get_main_target("main".into()),
            ExtnSymbol {
                id: "id".to_string(),
                uses: vec!["uses".to_string()],
                fulfills: vec!["fulfills".to_string()],
                config: None,
            },
            s,
        );
        assert_eq!(
            extn_client.get_other_senders().len(),
            1,
            "Assertion failed: extn_client.get_other_senders() does not have the expected length"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_extn_sender_with_extn_contract() {
        let extn_client = ExtnClient::mock();
        assert_eq!(
            extn_client.get_other_senders().len(),
            0,
            "Assertion failed: extn_client.get_other_senders() does not have the expected length"
        );

        let (s, _receiver) = unbounded();
        extn_client.clone().add_sender(
            ExtnId::get_main_target("main".into()),
            ExtnSymbol {
                id: "id".to_string(),
                uses: vec!["uses".to_string()],
                fulfills: vec!["account.session".to_string()],
                config: None,
            },
            s,
        );
        assert_eq!(
            extn_client.get_other_senders().len(),
            1,
            "Assertion failed: extn_client.get_other_senders() does not have the expected length"
        );
        println!(
            "**** extn_client: extn_client.get_other_senders(): {:?}",
            extn_client.get_other_senders()
        );
        let senders = extn_client.get_extn_sender_with_contract(RippleContract::Session(
            crate::api::session::SessionAdjective::Account,
        ));
        assert!(senders.is_some(), "Expected Some, got None");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_extn_sender_with_extn_id() {
        let extn_client = ExtnClient::mock();
        assert_eq!(
            extn_client.get_other_senders().len(),
            0,
            "Assertion failed: extn_client.get_other_senders() does not have the expected length"
        );

        let (s, _receiver) = unbounded();
        extn_client.clone().add_sender(
            ExtnId::get_main_target("main".into()),
            ExtnSymbol {
                id: "id".to_string(),
                uses: vec!["uses".to_string()],
                fulfills: vec!["account.session".to_string()],
                config: None,
            },
            s,
        );
        assert_eq!(
            extn_client.get_other_senders().len(),
            1,
            "Assertion failed: extn_client.get_other_senders() does not have the expected length"
        );
        let senders = extn_client
            .get_extn_sender_with_extn_id(&ExtnId::get_main_target("main".into()).to_string());
        assert!(senders.is_some(), "Expected Some, got None");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_cleanup_event_stream() {
        let capability = ExtnId::new_channel(ExtnClassId::Internal, "test".into());
        let (s, receiver) = unbounded();
        let mock_sender = ExtnSender::new(
            s,
            ExtnId::get_main_target("main".into()),
            vec!["context".to_string()],
            vec!["fulfills".to_string()],
            Some(HashMap::new()),
        );
        let mut extn_client = ExtnClient::new(receiver, mock_sender.clone());
        let extn_client_for_thread = extn_client.clone();

        let processor = MockEventProcessor {
            state: MockState {
                client: extn_client.clone(),
            },
            streamer: DefaultExtnStreamer::new(),
        };
        let id = &processor.contract().as_clear_string();
        println!(
            "**** extn_client: processor.contract().as_clear_string() {:?}",
            &id.clone()
        );
        extn_client.clone().add_event_processor(processor);
        assert!(extn_client.clone().event_processors.read().unwrap().len() == 1);

        println!(
            "**** extn_client: event_processors: sender len: {:?}",
            extn_client
                .event_processors
                .read()
                .unwrap()
                .get(&id.clone())
                .map(|v| v.len())
        );

        tokio::spawn(async move {
            extn_client_for_thread.initialize().await;
        });

        tokio::spawn(async move {
            let result = mock_sender.clone().send_event(
                MockEvent {
                    event_name: "test_event".to_string(),
                    result: serde_json::json!({"result": "result"}),
                    context: None,
                    app_id: Some("some_id".to_string()),
                },
                Some(mock_sender.clone().tx),
            );

            match result {
                Ok(response) => {
                    // Handle successful response
                    println!("**** Success: {:?}", response);
                }
                Err(err) => {
                    // Handle error
                    println!("**** Error: {:?}", err);
                }
            }
        });

        // Allow some time for the async tasks to complete
        tokio::time::sleep(Duration::from_secs(2)).await;
        assert!(extn_client.event_processors.read().unwrap().len() == 1);

        // Add the assertion to check if there are no Vec<MSender<ExtnMessage> in event_processors map after cleanup
        assert_eq!(
            extn_client
                .event_processors
                .read()
                .unwrap()
                .get(id)
                .map(|v| v.len()),
            None,
            "Assertion failed: Vec<MSender<ExtnMessage> in event_processors map does not have the expected length after cleanup"
        );
    }

    // TODO rename tests to be more descriptive
    #[tokio::test(flavor = "multi_thread")]
    async fn test_request_processor() {
        testing_logger::setup();
        let (s, receiver) = unbounded();
        let mock_sender = ExtnSender::new(
            s,
            ExtnId::get_main_target("main".into()),
            vec!["context".to_string()],
            vec!["fulfills".to_string()],
            Some(HashMap::new()),
        );
        let mut extn_client = ExtnClient::new(receiver, mock_sender.clone());
        let processor = MockRequestProcessor {
            state: MockState {
                client: extn_client.clone(),
            },
            streamer: DefaultExtnStreamer::new(),
        };

        let res = extn_client.add_request_processor(processor);
        assert!(res);

        tokio::spawn(async move {
            extn_client.clone().initialize().await;
        });

        tokio::spawn(async move {
            let result = mock_sender.clone().send_request(
                "some_id".to_string(),
                MockRequest {
                    context: MockContext {
                        app_id: "app_id".to_string(),
                        dist_session: AccountSession {
                            id: "id".to_string(),
                            token: "token".to_string(),
                            account_id: "account_id".to_string(),
                            device_id: "device_id".to_string(),
                        },
                    },
                },
                Some(mock_sender.clone().tx),
                None,
            );

            match result {
                Ok(response) => {
                    // Handle successful response
                    println!("**** Success: {:?}", response);
                }
                Err(err) => {
                    // Handle error
                    println!("**** Error: {:?}", err);
                }
            }
        });

        // Allow some time for the async tasks to complete
        tokio::time::sleep(Duration::from_secs(2)).await;

        validate(|captured_logs| {
            for log in captured_logs {
                assert_ne!(log.level, Level::Error);
                assert!(!log.body.contains("IEC Latency"));
                assert_ne!(log.body, "invalid message");
                assert_eq!(log.level, Level::Debug);
                assert!(log
                    .body
                    .contains("** extn_client: receiving message latency="));
            }
        });
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_initialize() {
        testing_logger::setup();
        let (s, receiver) = unbounded();
        let mock_sender = ExtnSender::new(
            s,
            ExtnId::get_main_target("main".into()),
            vec!["context".to_string()],
            vec!["fulfills".to_string()],
            Some(HashMap::new()),
        );
        let mut extn_client = ExtnClient::new(receiver, mock_sender.clone());
        let processor = MockRequestProcessor {
            state: MockState {
                client: extn_client.clone(),
            },
            streamer: DefaultExtnStreamer::new(),
        };

        let res = extn_client.add_request_processor(processor);
        assert!(res);

        let initialize_task = tokio::spawn(async move {
            extn_client.initialize().await;
        });

        // Use tokio::time::timeout to set a maximum duration for the test
        // if let Err(_) = tokio::time::timeout(Duration::from_secs(10), initialize_task).await {
        //     // If the task takes longer than 2 seconds, the test will fail
        //     panic!("**** Test timed out");
        // }

        tokio::spawn(async move {
            let result = mock_sender.clone().send_request(
                "some_id".to_string(),
                MockRequest {
                    context: MockContext {
                        app_id: "app_id".to_string(),
                        dist_session: AccountSession {
                            id: "id".to_string(),
                            token: "token".to_string(),
                            account_id: "account_id".to_string(),
                            device_id: "device_id".to_string(),
                        },
                    },
                },
                Some(mock_sender.clone().tx),
                None,
            );

            match result {
                Ok(response) => {
                    // Handle successful response
                    println!("**** Success: {:?}", response);
                }
                Err(err) => {
                    // Handle error
                    println!("**** Error: {:?}", err);
                }
            }
        });

        // Allow some time for the async tasks to complete
        tokio::time::sleep(Duration::from_secs(2)).await;

        validate(|captured_logs| {
            for log in captured_logs {
                assert!(!log.body.contains("IEC Latency"));
                assert_ne!(log.body, "invalid message");
                assert!(log
                    .body
                    .contains("** extn_client: receiving message latency="));
            }
        });
    }

    #[tokio::test]
    async fn test_context_update() {
        let (s, receiver) = unbounded();
        let mock_sender = ExtnSender::new(
            s,
            ExtnId::get_main_target("main".into()),
            vec!["context".to_string()],
            vec!["account.session".to_string()],
            Some(HashMap::new()),
        );
        let extn_client = ExtnClient::new(receiver, mock_sender.clone());
        let test_string = "TestString".to_string();
        let request = RippleContextUpdateRequest::TimeZone(TimeZone {
            time_zone: test_string.clone(),
            offset: 0,
        });
        extn_client.context_update(request);
        let ripple_context = extn_client.ripple_context.read().unwrap();
        println!("**** extn_client: ripple_context: {:?}", ripple_context);
        assert_eq!(ripple_context.time_zone.time_zone, test_string);
    }

    #[tokio::test]
    async fn test_handle_no_processor_error() {
        testing_logger::setup();
        let (s, receiver) = unbounded();
        let mock_sender = ExtnSender::new(
            s.clone(),
            ExtnId::get_main_target("main".into()),
            vec!["context".to_string()],
            vec!["account.session".to_string()],
            Some(HashMap::new()),
        );
        let mut extn_client = ExtnClient::new(receiver, mock_sender.clone());
        let processor = MockRequestProcessor {
            state: MockState {
                client: extn_client.clone(),
            },
            streamer: DefaultExtnStreamer::new(),
        };
        extn_client.clone().add_sender(
            ExtnId::get_main_target("main".into()),
            ExtnSymbol {
                id: "id".to_string(),
                uses: vec!["uses".to_string()],
                fulfills: vec!["account.session".to_string()],
                config: None,
            },
            s,
        );
        extn_client.add_request_processor(processor);
        let message = ExtnMessage {
            id: uuid::Uuid::new_v4().to_string(),
            requestor: ExtnId::get_main_target("main".into()),
            target: RippleContract::Internal,
            payload: ExtnPayload::Request(ExtnRequest::Extn(
                serde_json::to_value(MockRequest {
                    context: MockContext {
                        app_id: "app_id".to_string(),
                        dist_session: AccountSession {
                            id: "id".to_string(),
                            token: "token".to_string(),
                            account_id: "account_id".to_string(),
                            device_id: "device_id".to_string(),
                        },
                    },
                })
                .unwrap(),
            )),
            callback: None,
            ts: Some(Utc::now().timestamp_millis()),
        };
        extn_client.handle_no_processor_error(message);

        validate(|captured_logs| {
            for log in captured_logs {
                assert!(log
                    .body
                    .contains("** extn_client: Sending message on the other sender"));
            }
        });
    }

    // Add straightforward test cases
    // -- add_stream_processor
    // -- add_vec_stream_processor
    // -- add_single_processor

    // Mockable for ExtnClient
    // Abilities required for reusablity
    // --Setting up the sender cap as main and not main
    // --Supporting or Unsupporting a given contract
    // --Adding a mock stream processor and returning a result
    // --Adding a mock extension (Sender async_channel) for a contract

    // Testcases for add_request_processor
    // Change this method to return a boolean to indicate whether it was successfully added. (return contracts_supported.is_empty()
    // -- Add a request processor for which contract is supported
    // -- Add a request processor for which a contract is un supported

    // Testcases for add_event_processor , cleanup_event_stream, get_other_senders
    // there are conditions here just a plain operation

    // Testcases for add_sender
    // Check from an extension
    // Check from main

    // Testcases for Inititalizes
    // Check if latency is printed as log (use testing_logger dependency )
    // Check if invalid message is printed
    // Check if debug message is printed with the message
    // Send a response and expect it in a mock processor
    // Send an event which is not ripple context to main
    // Send an event which is not ripple context to extension
    // Send an event which is ripple context to main
    // Send an event which is ripple context to extension
    // Send a request to main for a contract in extension.
    // Send a request to main which is a ripple context update request
    // Send a request to main for an internal contract

    // Testcase for context update
    // Call the method for a client which is not main
    // Other cases

    // Testcase for handle_no_processor_error
    // Try sending to an extension with the fulfiillment
    // Try sending a request for a contract not fulfilled and expect error
}
