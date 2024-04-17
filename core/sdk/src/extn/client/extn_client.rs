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
#[cfg(not(test))]
use log::{debug, error, info, trace};

#[cfg(test)]
use {println as info, println as trace, println as debug, println as error};

use tokio::sync::{
    mpsc::Sender as MSender,
    oneshot::{self, Sender as OSender},
};

use crate::{
    api::{
        context::{
            ActivationStatus, RippleContext, RippleContextUpdateRequest, RippleContextUpdateType,
        },
        device::{
            device_info_request::DeviceInfoRequest,
            device_request::{InternetConnectionStatus, TimeZone},
        },
        firebolt::fb_metrics::MetricsContext,
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

fn add_stream_processor<P>(id: String, context: P, map: Arc<RwLock<HashMap<String, P>>>) {
    let mut processor_state = map.write().unwrap();
    processor_state.insert(id, context);
}

fn add_vec_stream_processor<P>(id: String, context: P, map: Arc<RwLock<HashMap<String, Vec<P>>>>) {
    let mut processor_state = map.write().unwrap();
    if let std::collections::hash_map::Entry::Vacant(e) = processor_state.entry(id.clone()) {
        e.insert(vec![context]);
    } else {
        processor_state.get_mut(&id).unwrap().push(context)
    }
}

fn add_single_processor<P>(id: String, processor: Option<P>, map: Arc<RwLock<HashMap<String, P>>>) {
    if let Some(processor) = processor {
        let mut processor_state = map.write().unwrap();
        processor_state.insert(id, processor);
    }
}

pub fn remove_processor<P>(id: String, map: Arc<RwLock<HashMap<String, P>>>) {
    let mut processor_state = map.write().unwrap();
    let sender = processor_state.remove(&id);
    drop(sender);
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
    pub fn add_request_processor(&mut self, mut processor: impl ExtnRequestProcessor) {
        let contracts = if let Some(multiple_contracts) = processor.fulfills_mutiple() {
            multiple_contracts
        } else {
            vec![processor.contract()]
        };
        let contracts_supported: Vec<RippleContract> = contracts
            .into_iter()
            .filter(|contract| self.sender.check_contract_fulfillment(contract.clone()))
            .collect();

        contracts_supported.iter().for_each(|contract| {
            let processor_string: String = contract.as_clear_string();
            info!("adding request processor {}", processor_string);
            add_stream_processor(
                processor_string,
                processor.sender(),
                self.request_processors.clone(),
            );
        });
        // Dont add and start a request processor if there is no contract fulfillment
        if !contracts_supported.is_empty() {
            tokio::spawn(async move {
                trace!(
                    "starting request processor green tokio thread for {:?}",
                    contracts_supported
                );
                processor.run().await
            });
        }
    }

    /// Removes a request processor reference on the internal map of processors
    pub fn remove_request_processor(&mut self, capability: ExtnId) {
        remove_processor(capability.to_string(), self.request_processors.clone());
    }

    /// Adds a new event processor reference to the internal map of processors
    ///
    /// Uses the capability provided by the Processor for registration
    ///
    /// Also starts the thread in the processor to accept incoming events.
    pub fn add_event_processor(&mut self, mut processor: impl ExtnEventProcessor) {
        add_vec_stream_processor(
            processor.contract().as_clear_string(),
            processor.sender(),
            self.event_processors.clone(),
        );
        let processor_clone = self.event_processors.clone();
        let id_clone = processor.contract().as_clear_string();
        tokio::spawn(async move {
            processor.run().await;
            Self::cleanup_vec_stream(id_clone, None, processor_clone);
        });
    }

    /// Removes an event processor reference on the internal map of processors
    pub fn cleanup_event_stream(&mut self, capability: ExtnId) {
        Self::cleanup_vec_stream(capability.to_string(), None, self.event_processors.clone());
    }

    /// Used mainly by `Main` application to add senders of the extensions for IEC
    pub fn add_sender(&mut self, id: ExtnId, symbol: ExtnSymbol, sender: CSender<CExtnMessage>) {
        let id = id.to_string();
        {
            // creating a map - extnId & sender used for requestor mapping to add to callback in extnMessage
            let mut sender_map = self.extn_sender_map.write().unwrap();
            sender_map.insert(id.clone(), sender);
        }
        {
            let mut map = HashMap::new();
            for contract in symbol.fulfills {
                match RippleContract::from_manifest(&contract) {
                    Some(v) => {
                        let ripple_contract_string = v.as_clear_string();
                        info!("{} will fulfill {}", id, ripple_contract_string);
                        // creating a map - contract & sender used for request mapping
                        let _ = map.insert(ripple_contract_string, id.clone());
                    }
                    None => error!("Unknown contract {}", contract),
                }
            }
            let mut contract_map = self.contract_map.write().unwrap();
            contract_map.extend(map);
        }
    }

    pub fn get_other_senders(&self) -> Vec<CSender<CExtnMessage>> {
        self.extn_sender_map
            .read()
            .unwrap()
            .iter()
            .inspect(|item| debug!("other sender: {:?}", item.0))
            .map(|(_, v)| v)
            .cloned()
            .collect()
    }

    /// Called once per client initialization this is a blocking method. Use a spawned thread to call this method
    pub async fn initialize(&self) {
        debug!("Starting initialize");
        let receiver = self.receiver.clone();
        while let Ok(c_message) = receiver.recv().await {
            let latency = Utc::now().timestamp_millis() - c_message.ts;

            if latency > 1000 {
                error!("IEC Latency {:?}", c_message);
            }
            let message_result: Result<ExtnMessage, RippleError> = c_message.clone().try_into();
            let message = match message_result {
                Ok(extn_msg) => extn_msg,
                Err(_) => {
                    error!("invalid message {:?}", c_message);
                    continue;
                }
            };
            debug!("** receiving message latency={} msg={:?}", latency, message);
            if message.payload.is_response() {
                Self::handle_single(message, self.response_processors.clone());
            } else if message.payload.is_event() {
                let is_main = self.sender.get_cap().is_main();
                if is_main // This part of code is for the main ExntClient to handle
                            && message.target_id.is_some() // The sender knew the target
                            && !message.target_id.as_ref().unwrap().is_main()
                // But it is not for main. So main has to fwd it.
                {
                    if let Some(sender) = self.get_extn_sender_with_extn_id(
                        &message.target_id.as_ref().unwrap().to_string(),
                    ) {
                        let send_response = self.sender.respond(message.into(), Some(sender));
                        debug!("fwding event result: {:?}", send_response);
                    } else {
                        debug!("unable to get sender for target: {:?}", message.target_id);
                        self.handle_no_processor_error(message);
                    }
                } else {
                    if !is_main {
                        if let Some(context) = RippleContext::is_ripple_context(&message.payload) {
                            trace!(
                                "Received ripple context in {} message: {:?}",
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
                }
            } else {
                let current_cap = self.sender.get_cap();
                let target_contract = message.clone().target;
                if current_cap.is_main() {
                    if let Some(request) =
                        RippleContextUpdateRequest::is_ripple_context_update(&message.payload)
                    {
                        self.context_update(request);
                    }
                    // if its a request coming as an extn provider the extension is calling on itself.
                    // for eg an extension has a RPC Method provider and also a channel to process the
                    // requests this below impl will take care of sending the data back to the Extension
                    else if let Some(extn_id) = target_contract.is_extn_provider() {
                        if let Some(s) = self.get_extn_sender_with_extn_id(&extn_id) {
                            let new_message = message.clone();
                            tokio::spawn(async move {
                                if let Err(e) = s.send(new_message.into()).await {
                                    error!("Error forwarding request {:?}", e)
                                }
                            });
                        } else {
                            error!("couldn't find the extension id registered the extn channel {:?} is not available", extn_id);
                            self.handle_no_processor_error(message);
                        }
                    }
                    // Forward the message to an extn sender
                    else if let Some(sender) = self.get_extn_sender_with_contract(target_contract)
                    {
                        let mut new_message = message.clone();
                        if new_message.callback.is_none() {
                            // before forwarding check if the requestor needs to be added as callback
                            let req_sender =
                                self.get_extn_sender_with_extn_id(&message.requestor.to_string());

                            if let Some(sender) = req_sender {
                                let _ = new_message.callback.insert(sender);
                            }
                            Self::handle_vec_stream(message, self.event_processors.clone());
                        }

                        tokio::spawn(async move {
                            if let Err(e) = sender.try_send(new_message.into()) {
                                error!("Error forwarding request {:?}", e)
                            }
                        });
                    } else {
                        // could be main contract
                        if !Self::handle_stream(message.clone(), self.request_processors.clone()) {
                            self.handle_no_processor_error(message);
                        }
                    }
                } else if !Self::handle_stream(message.clone(), self.request_processors.clone()) {
                    self.handle_no_processor_error(message);
                }
            }
        }

        debug!("Initialize Ended Abruptly");
    }

    pub fn context_update(&self, request: RippleContextUpdateRequest) {
        let current_cap = self.sender.get_cap();
        if !current_cap.is_main() {
            error!("Updating context is not allowed outside main");
            return;
        }
        if let RippleContextUpdateRequest::RefreshContext(refresh_context) = &request {
            if let Some(RippleContextUpdateType::InternetConnectionChanged) = refresh_context {
                let resp = self.request_transient(DeviceInfoRequest::InternetConnectionStatus);
                if let Err(_err) = resp {
                    error!("Error in starting internet monitoring");
                }
            }
            return;
        }
        // Main's Extn client will receive Context events and if it results in changing any of its
        // context members then it propagates the event to other extension's extn client.
        // Propagating 'known information' to other clients increases processing but no meaningful task is performed.
        let propagate = {
            let mut ripple_context = self.ripple_context.write().unwrap();
            debug!(
                "Received context request: {:?} current ripple_context: {:?}",
                request, ripple_context
            );
            ripple_context.update(request)
        };
        let new_context = { self.ripple_context.read().unwrap().clone() };
        let message = new_context.get_event_message();
        if propagate {
            debug!("Formed Context update event: {:?}", message);
            let c_message: CExtnMessage = message.clone().into();
            {
                let senders = self.get_other_senders();
                for sender in senders {
                    let send_res = sender.try_send(c_message.clone());
                    trace!("Send to other client result: {:?}", send_res);
                }
            }
        } else {
            debug!("Context information is already updated. Hence not propagating");
        }
        Self::handle_vec_stream(message, self.event_processors.clone());
    }

    fn handle_no_processor_error(&self, message: ExtnMessage) {
        let req_sender = self.get_extn_sender_with_extn_id(&message.requestor.to_string());

        if let Ok(resp) = message.get_response(ExtnResponse::Error(RippleError::ProcessorError)) {
            if self.sender.respond(resp.into(), req_sender).is_err() {
                error!("Couldnt send no processor response");
            }
        }
    }

    fn handle_single(
        msg: ExtnMessage,
        processor: Arc<RwLock<HashMap<String, OSender<ExtnMessage>>>>,
    ) {
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
        let id_c: String = msg.target.as_clear_string();

        let v = {
            let processors = processor.read().unwrap();
            processors.get(&id_c).cloned()
        };
        if let Some(sender) = v {
            tokio::spawn(async move {
                if let Err(e) = sender.send(msg.clone()).await {
                    error!("Error sending the response back {:?}", e);
                }
            });
            true
        } else {
            error!("No Request Processor for {} {:?}", id_c, msg);
            false
        }
    }

    fn handle_vec_stream(
        msg: ExtnMessage,
        processor: Arc<RwLock<HashMap<String, Vec<MSender<ExtnMessage>>>>>,
    ) {
        let id_c = msg.target.as_clear_string();
        let mut gc_sender_indexes: Vec<usize> = Vec::new();
        let read_processor = processor.clone();
        {
            let processors = read_processor.read().unwrap();
            let v = processors.get(&id_c).cloned();
            if let Some(v) = v {
                for (index, s) in v.iter().enumerate() {
                    if !s.is_closed() {
                        let sndr = s.clone();
                        let m = msg.clone();
                        tokio::spawn(async move {
                            if let Err(e) = sndr.try_send(m) {
                                error!("Error sending the response back {:?}", e);
                            }
                        });
                    } else {
                        gc_sender_indexes.push(index);
                    }
                }
            }
        };

        if RippleContext::is_ripple_context(&msg.payload).is_none() {
            // Not every extension will have a context listener
            error!("No Event Processor for {:?}", msg);
        }

        let cleanup_indexes = match gc_sender_indexes.is_empty() {
            true => None,
            false => Some(gc_sender_indexes),
        };
        Self::cleanup_vec_stream(id_c, cleanup_indexes, processor);
    }

    fn cleanup_vec_stream(
        id_c: String,
        gc_sender_indexes: Option<Vec<usize>>,
        processor: Arc<RwLock<HashMap<String, Vec<MSender<ExtnMessage>>>>>,
    ) {
        let indices = match gc_sender_indexes {
            Some(i) => Some(i),
            None => processor.read().unwrap().get(&id_c).map(|v| {
                v.iter()
                    .filter(|x| x.is_closed())
                    .enumerate()
                    .map(|(i, _)| i)
                    .collect()
            }),
        };
        if let Some(indices) = indices {
            if !indices.is_empty() {
                let mut gc_cleanup = processor.write().unwrap();
                if let Some(sender_list) = gc_cleanup.get_mut(&id_c) {
                    for index in indices {
                        let r = sender_list.remove(index);
                        drop(r);
                    }
                    if sender_list.is_empty() {
                        let _ = gc_cleanup.remove(&id_c);
                    }
                }
            }
        }
    }

    fn get_extn_sender_with_contract(
        &self,
        contract: RippleContract,
    ) -> Option<CSender<CExtnMessage>> {
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
        self.sender.respond(
            msg.clone().into(),
            self.get_extn_sender_with_extn_id(&msg.requestor.to_string()),
        )
    }

    /// Critical method used by event processors to emit event back to the requestor
    /// # Arguments
    /// `msg` - [ExtnMessage] event object
    pub fn event(&self, event: impl ExtnPayloadProvider) -> Result<(), RippleError> {
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
        let id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        add_single_processor(id.clone(), Some(tx), self.response_processors.clone());
        let other_sender = self.get_extn_sender_with_contract(payload.get_contract());
        self.sender.send_request(id, payload, other_sender, None)?;
        if let Ok(r) = rx.await {
            return Ok(r);
        }

        Err(RippleError::ExtnError)
    }

    ///
    /// Same as request except will inspect the response payload for errors
    /// and place error in the returned result instead of in the payload
    /// TODO: should request just do this?
    pub async fn request_and_flatten(
        &mut self,
        payload: impl ExtnPayloadProvider,
    ) -> Result<ExtnMessage, RippleError> {
        let res = self.request(payload).await;
        match res {
            Err(e) => Err(e),
            Ok(r) => {
                if let Some(ExtnResponse::Error(e)) = r.payload.extract() {
                    Err(e)
                } else {
                    Ok(r)
                }
            }
        }
    }

    /// Request method which accepts a impl [ExtnPayloadProvider] and uses the capability provided by the trait to send the request.
    /// As part of the send process it adds a callback to asynchronously respond back to the caller when the response does get
    /// received. This method can be called synchronously with a timeout
    ///
    /// # Arguments
    /// `payload` - impl [ExtnPayloadProvider]
    pub async fn standalone_request<T: ExtnPayloadProvider>(
        &self,
        payload: impl ExtnPayloadProvider,
        timeout_in_msecs: u64,
    ) -> Result<T, RippleError> {
        let id = uuid::Uuid::new_v4().to_string();
        let (tx, tr) = bounded(2);
        let other_sender = self.get_extn_sender_with_contract(payload.get_contract());
        self.sender
            .send_request(id, payload, other_sender, Some(tx))?;
        match tokio::time::timeout(Duration::from_millis(timeout_in_msecs), tr.recv()).await {
            Ok(Ok(cmessage)) => {
                debug!("** receiving message msg={:?}", cmessage);
                let message: Result<ExtnMessage, RippleError> = cmessage.try_into();

                if let Ok(message) = message {
                    if let Some(v) = message.payload.extract() {
                        return Ok(v);
                    } else {
                        return Err(RippleError::ParseError);
                    }
                }
            }
            Ok(Err(e)) => error!("Invalid message: e={:?}", e),
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
    pub fn request_transient(&self, payload: impl ExtnPayloadProvider) -> RippleResponse {
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
        if let Some(sender) = self.get_extn_sender_with_extn_id(id) {
            self.sender.send_event(event, Some(sender))
        } else {
            debug!("current client has so sender information so call forward event");
            self.sender.forward_event(id, event)
        }
    }

    /// Method to get configurations on the manifest per extension
    pub fn check_contract_fulfillment(&self, contract: RippleContract) -> bool {
        self.sender.check_contract_fulfillment(contract)
    }

    // Method to check if contract is permitted
    pub fn check_contract_permitted(&self, contract: RippleContract) -> bool {
        self.sender.check_contract_permission(contract)
    }

    pub fn has_token(&self) -> bool {
        let ripple_context = self.ripple_context.read().unwrap();
        // matches!(
        //     ripple_context.activation_status.clone(),
        //     ActivationStatus::AccountToken(_)
        // )
        matches!(
            ripple_context.activation_status.as_ref(),
            Some(activation_status) if matches!(activation_status, ActivationStatus::AccountToken(_))
        )
    }

    pub fn get_activation_status(&self) -> Option<ActivationStatus> {
        // pub fn get_activation_status(&self) -> ActivationStatus {
        let ripple_context = self.ripple_context.read().unwrap();
        ripple_context.activation_status.clone()
    }

    pub fn has_internet(&self) -> bool {
        let ripple_context = self.ripple_context.read().unwrap();
        matches!(
            ripple_context.internet_connectivity.as_ref(), Some(internet_connectivity) if matches!(internet_connectivity, InternetConnectionStatus::FullyConnected | InternetConnectionStatus::LimitedInternet)
        )
    }

    pub fn internet_status(&self) -> Option<InternetConnectionStatus> {
        let ripple_contract = self.ripple_context.read().unwrap();
        ripple_contract.internet_connectivity.clone()
    }

    pub fn get_timezone(&self) -> Option<TimeZone> {
        let ripple_context = self.ripple_context.read().unwrap();
        // Some(ripple_context.time_zone.clone())
        ripple_context.time_zone.clone()
    }

    pub fn get_features(&self) -> Vec<String> {
        let ripple_context = self.ripple_context.read().unwrap();
        ripple_context.features.clone()
    }

    pub fn get_metrics_context(&self) -> Option<MetricsContext> {
        let ripple_context = self.ripple_context.read().unwrap();
        ripple_context.metrics_context.clone()
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        api::{
            config::Config,
            device::{
                device_info_request::DeviceInfoRequest,
                device_request::{AccountToken, DeviceRequest},
            },
            session::SessionAdjective,
        },
        extn::{
            client::{
                extn_processor::{
                    tests::{MockEventProcessor, MockRequestProcessor},
                    ExtnStreamProcessor,
                },
                extn_sender::tests::Mockable as extn_sender_mockable,
            },
            extn_client_message::{ExtnPayload, ExtnRequest},
            extn_id::{ExtnClassId, ExtnId},
        },
        utils::mock_utils::{get_mock_extn_client, MockEvent, MockRequest},
    };
    use async_channel::unbounded;
    use core::panic;
    use rstest::rstest;
    use std::collections::HashMap;
    use testing_logger::{self, validate};
    use tokio::sync::oneshot;
    use tokio::time::Duration;
    use uuid::Uuid;

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
            _id: ExtnId,
            _context: Vec<String>,
            _fulfills: Vec<String>,
            _config: Option<HashMap<String, String>>,
        ) -> ExtnClient {
            let (tx, rx) = unbounded();
            let mock_sender = ExtnSender::new(
                tx,
                ExtnId::get_main_target("main".into()),
                vec!["context".to_string()],
                Vec::new(),
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
                Vec::new(),
                Some(HashMap::new()),
            );
            ExtnClient::new(rx, mock_sender)
        }
    }

    #[test]
    fn test_add_stream_processor() {
        let extn_client = ExtnClient::mock();
        let processor =
            MockRequestProcessor::new_v1(extn_client.clone(), vec![RippleContract::Internal]);

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
        let processor =
            MockEventProcessor::new_v1(extn_client.clone(), vec![RippleContract::Internal]);
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
        add_single_processor(id, Some(tx), extn_client.response_processors.clone());

        assert!(extn_client.response_processors.read().unwrap().len() == 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_add_request_processor() {
        let mut extn_client = ExtnClient::mock();
        let processor =
            MockRequestProcessor::new_v1(extn_client.clone(), vec![RippleContract::Internal]);

        extn_client.add_request_processor(processor);
        tokio::time::sleep(Duration::from_millis(10)).await;

        assert!(extn_client.request_processors.read().unwrap().len() == 1);
        validate(|captured_logs| {
            for log in captured_logs {
                assert!(log
                    .body
                    .contains("starting request processor green tokio thread for"));
                assert!(log.body.contains("processing request"));
            }
        });
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_add_event_processor() {
        let mut extn_client = ExtnClient::mock();
        let processor =
            MockEventProcessor::new_v1(extn_client.clone(), vec![RippleContract::Internal]);
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
        case(ExtnId::new_channel(ExtnClassId::Internal, "test".into()), 1)
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
                uses: Vec::new(),
                fulfills: Vec::new(),
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
                uses: Vec::new(),
                fulfills: Vec::new(),
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
                uses: Vec::new(),
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
                uses: Vec::new(),
                fulfills: vec![RippleContract::Session(SessionAdjective::Device).as_clear_string()],
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
        let (mock_sender, mock_rx) = ExtnSender::mock();
        let extn_client = ExtnClient::new(mock_rx, mock_sender);
        let mut processor = MockEventProcessor::new_v1(
            extn_client.clone(),
            vec![RippleContract::Session(SessionAdjective::Device)],
        );

        add_vec_stream_processor(
            processor.contract().as_clear_string(),
            processor.sender(),
            extn_client.event_processors.clone(),
        );

        let processor_clone = extn_client.event_processors.clone();
        let id_clone = processor.contract().as_clear_string();
        let mut rx = processor.receiver();
        rx.close();
        drop(rx);

        assert_eq!(
            extn_client.event_processors.read().unwrap().len(),
            1,
            "Assertion failed: event_processors map should be empty before cleanup"
        );

        ExtnClient::cleanup_vec_stream(id_clone, None, processor_clone);

        assert_eq!(
            extn_client.event_processors.read().unwrap().len(),
            0,
            "Assertion failed: event_processors map should be empty after cleanup"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_request() {
        let (mock_sender, mock_rx) = ExtnSender::mock();
        let mut extn_client = ExtnClient::new(mock_rx.clone(), mock_sender.clone());
        let processor = MockRequestProcessor::new_v1(
            extn_client.clone(),
            vec![
                RippleContract::Internal,
                RippleContract::Session(SessionAdjective::Device),
            ],
        );

        extn_client.add_request_processor(processor);

        let extn_client_for_thread = extn_client.clone();

        tokio::spawn(async move {
            extn_client_for_thread.initialize().await;
        });

        let response = extn_client
            .request(MockRequest {
                app_id: "test_app_id".to_string(),
                contract: RippleContract::Internal,
                expected_response: Some(ExtnResponse::Boolean(true)),
            })
            .await;

        match response {
            Ok(actual_response) => {
                let expected_message = ExtnMessage {
                    id: "some-uuid".to_string(),
                    requestor: ExtnId::get_main_target("main".into()),
                    target: RippleContract::Internal,
                    target_id: None,
                    payload: ExtnPayload::Response(ExtnResponse::Boolean(true)),
                    callback: None,
                    ts: Some(Utc::now().timestamp_millis()),
                };

                assert!(Uuid::parse_str(&actual_response.id).is_ok());
                assert_eq!(actual_response.requestor, expected_message.requestor);
                assert_eq!(actual_response.target, expected_message.target);
                assert_eq!(actual_response.target_id, expected_message.target_id);

                assert_eq!(
                    actual_response.callback.is_some(),
                    expected_message.callback.is_some()
                );
                assert!(actual_response.ts.is_some());
            }
            Err(_) => {
                panic!("Received an unexpected error");
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_request_between_main_extn() {
        // test case: main <=> extn

        // create main client
        let (main_sender, main_rx) = ExtnSender::mock();
        let mut main_client = ExtnClient::new(main_rx.clone(), main_sender.clone());

        // create extn client
        let (extn_sender, extn_tx, extn_rx) = ExtnSender::mock_extn(
            ExtnId::new_extn(ExtnClassId::Device, "info".into()),
            Vec::new(),
            vec![RippleContract::DeviceInfo.as_clear_string()],
            Some(HashMap::new()),
            main_sender.tx.clone(),
        );
        let mut extn_client = ExtnClient::new(extn_rx.clone(), extn_sender.clone());

        // add processor to extn
        let processor =
            MockRequestProcessor::new_v1(extn_client.clone(), vec![RippleContract::DeviceInfo]);
        extn_client.add_request_processor(processor);

        // add sender to main
        main_client.clone().add_sender(
            ExtnId::new_extn(ExtnClassId::Device, "info".into()),
            ExtnSymbol {
                id: ExtnId::new_extn(ExtnClassId::Device, "info".into()).to_string(),
                uses: Vec::new(),
                fulfills: vec![RippleContract::DeviceInfo.as_clear_string()],
                config: Some(HashMap::new()),
            },
            extn_tx,
        );

        // initialize the clients
        let extn_client_for_thread = extn_client.clone();
        let main_client_for_thread = main_client.clone();

        tokio::spawn(async move {
            main_client_for_thread.initialize().await;
        });

        tokio::spawn(async move {
            extn_client_for_thread.initialize().await;
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        // make the request from main to extn
        let response = main_client
            .request(MockRequest {
                app_id: "test_app_id".to_string(),
                contract: RippleContract::DeviceInfo,
                expected_response: Some(ExtnResponse::Boolean(true)),
            })
            .await;
        println!("**** response: {:?}", response);

        match response {
            Ok(actual_response) => {
                let expected_message = ExtnMessage {
                    id: "some-uuid".to_string(),
                    requestor: ExtnId::get_main_target("main".into()),
                    target: RippleContract::DeviceInfo,
                    target_id: None,
                    payload: ExtnPayload::Response(ExtnResponse::Boolean(true)),
                    callback: None,
                    ts: Some(Utc::now().timestamp_millis()),
                };

                assert!(Uuid::parse_str(&actual_response.id).is_ok());
                assert_eq!(actual_response.requestor, expected_message.requestor);
                assert_eq!(actual_response.target, expected_message.target);
                assert_eq!(actual_response.target_id, expected_message.target_id);

                assert_eq!(
                    actual_response.callback.is_some(),
                    expected_message.callback.is_some()
                );
                assert!(actual_response.ts.is_some());
            }
            Err(_) => {
                panic!("Received an unexpected error");
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_request_between_extn_main() {
        // test case: extn <=> main

        // create main client
        let (main_sender, main_rx) = ExtnSender::mock();
        let mut main_client = ExtnClient::new(main_rx.clone(), main_sender.clone());

        // create extn client
        let (extn_sender, extn_tx, extn_rx) = ExtnSender::mock_extn(
            ExtnId::new_extn(ExtnClassId::Device, "info".into()),
            vec![RippleContract::Config.as_clear_string()],
            vec![RippleContract::DeviceInfo.as_clear_string()],
            Some(HashMap::new()),
            main_sender.tx.clone(),
        );
        let mut extn_client = ExtnClient::new(extn_rx.clone(), extn_sender.clone());

        // add processor to main
        let processor =
            MockRequestProcessor::new_v1(main_client.clone(), vec![RippleContract::Config]);
        main_client.add_request_processor(processor);

        // add sender to contract map
        main_client.clone().add_sender(
            ExtnId::new_extn(ExtnClassId::Device, "info".into()),
            ExtnSymbol {
                id: ExtnId::new_extn(ExtnClassId::Device, "info".into()).to_string(),
                uses: vec![RippleContract::Config.as_clear_string()],
                fulfills: vec![RippleContract::DeviceInfo.as_clear_string()],
                config: Some(HashMap::new()),
            },
            extn_tx,
        );

        // initialize the clients
        let extn_client_for_thread = extn_client.clone();
        let main_client_for_thread = main_client.clone();

        tokio::spawn(async move {
            main_client_for_thread.initialize().await;
        });

        tokio::spawn(async move {
            extn_client_for_thread.initialize().await;
        });

        // make the request from extn to main
        let response = extn_client
            .request(MockRequest {
                app_id: "test_app_id".to_string(),
                contract: RippleContract::Config,
                expected_response: Some(ExtnResponse::String("some_config_resp".to_string())),
            })
            .await;
        println!("**** response: {:?}", response);

        match response {
            Ok(actual_response) => {
                let expected_message = ExtnMessage {
                    id: "some-uuid".to_string(),
                    requestor: ExtnId::new_extn(ExtnClassId::Device, "info".into()),
                    target: RippleContract::Config,
                    target_id: None,
                    payload: ExtnPayload::Response(ExtnResponse::String(
                        "some_config_resp".to_string(),
                    )),
                    callback: None,
                    ts: Some(Utc::now().timestamp_millis()),
                };

                assert!(Uuid::parse_str(&actual_response.id).is_ok());
                assert_eq!(actual_response.requestor, expected_message.requestor);
                assert_eq!(actual_response.target, expected_message.target);
                assert_eq!(actual_response.target_id, expected_message.target_id);

                assert_eq!(
                    actual_response.callback.is_some(),
                    expected_message.callback.is_some()
                );
                assert!(actual_response.ts.is_some());
            }
            Err(_) => {
                panic!("Received an unexpected error");
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_request_between_extns() {
        // test case : dist -> main -> device => device -> dist

        // create main client
        let (main_sender, main_rx) = ExtnSender::mock();
        let main_client = ExtnClient::new(main_rx.clone(), main_sender.clone());

        // create dist extn client
        let (dist_sender, dist_tx, dist_rx) = ExtnSender::mock_extn(
            ExtnId::new_extn(ExtnClassId::Distributor, "general".into()),
            vec![
                RippleContract::Config.as_clear_string(),
                RippleContract::DeviceInfo.as_clear_string(),
            ],
            vec![RippleContract::Permissions.as_clear_string()],
            Some(HashMap::new()),
            main_sender.tx.clone(),
        );
        let mut dist_extn_client = ExtnClient::new(dist_rx.clone(), dist_sender.clone());

        // add dist sender to main contract map
        main_client.clone().add_sender(
            ExtnId::new_extn(ExtnClassId::Distributor, "general".into()),
            ExtnSymbol {
                id: ExtnId::new_extn(ExtnClassId::Distributor, "general".into()).to_string(),
                uses: vec![
                    RippleContract::Config.as_clear_string(),
                    RippleContract::DeviceInfo.as_clear_string(),
                ],
                fulfills: vec![RippleContract::Permissions.as_clear_string()],
                config: Some(HashMap::new()),
            },
            dist_tx,
        );

        // create device extn client
        let (dev_sender, dev_tx, dev_rx) = ExtnSender::mock_extn(
            ExtnId::new_extn(ExtnClassId::Device, "info".into()),
            vec![RippleContract::Config.as_clear_string()],
            vec![RippleContract::DeviceInfo.as_clear_string()],
            Some(HashMap::new()),
            main_sender.tx.clone(),
        );
        let mut dev_extn_client = ExtnClient::new(dev_rx.clone(), dev_sender.clone());

        // add device processor to dev_client
        let processor =
            MockRequestProcessor::new_v1(dev_extn_client.clone(), vec![RippleContract::DeviceInfo]);
        dev_extn_client.add_request_processor(processor);

        // add device sender to main contract map
        main_client.clone().add_sender(
            ExtnId::new_extn(ExtnClassId::Device, "info".into()),
            ExtnSymbol {
                id: ExtnId::new_extn(ExtnClassId::Device, "info".into()).to_string(),
                uses: vec![RippleContract::Config.as_clear_string()],
                fulfills: vec![RippleContract::DeviceInfo.as_clear_string()],
                config: Some(HashMap::new()),
            },
            dev_tx,
        );

        // initialize the clients
        let dev_extn_client_for_thread = dev_extn_client.clone();
        let dist_extn_client_for_thread = dist_extn_client.clone();
        let main_client_for_thread = main_client.clone();

        tokio::spawn(async move {
            dev_extn_client_for_thread.initialize().await;
        });

        tokio::spawn(async move {
            dist_extn_client_for_thread.initialize().await;
        });

        tokio::spawn(async move {
            main_client_for_thread.initialize().await;
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        // make the request from dist to device
        let response = dist_extn_client
            .request(MockRequest {
                app_id: "test_app_id".to_string(),
                contract: RippleContract::DeviceInfo,
                expected_response: Some(ExtnResponse::String("some_response".to_string())),
            })
            .await;
        println!("**** response: {:?}", response);

        match response {
            Ok(actual_response) => {
                assert!(Uuid::parse_str(&actual_response.id).is_ok());
                assert_eq!(
                    actual_response.requestor,
                    ExtnId::new_extn(ExtnClassId::Distributor, "general".into())
                );
                assert_eq!(actual_response.target, RippleContract::DeviceInfo);
                assert_eq!(actual_response.target_id, None);

                assert!(actual_response.callback.is_some());
                assert!(actual_response.ts.is_some());
            }
            Err(_) => {
                panic!("Received an unexpected error");
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_request_context_update() {
        let (mock_sender, mock_rx) = ExtnSender::mock();
        let main_client = ExtnClient::new(mock_rx, mock_sender.clone());
        main_client.clone().add_sender(
            ExtnId::get_main_target("main".into()),
            ExtnSymbol {
                id: "id".to_string(),
                uses: vec!["config".to_string()],
                fulfills: vec!["permissions".to_string()],
                config: None,
            },
            mock_sender.tx,
        );

        let main_client_for_thread = main_client.clone();

        tokio::spawn(async move {
            main_client_for_thread.initialize().await;
        });

        let time_zone = "America/New_York".to_string();
        let offset = -5;

        let result =
            main_client.request_transient(RippleContextUpdateRequest::TimeZone(TimeZone {
                time_zone: time_zone.clone(),
                offset,
            }));
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(result.is_ok());

        let ripple_context = main_client.ripple_context.read().unwrap();
        assert_eq!(
            ripple_context.time_zone.as_ref().unwrap().time_zone,
            time_zone
        );
        assert_eq!(ripple_context.time_zone.as_ref().unwrap().offset, offset);
    }

    // TODO - add test case for event subscribe & case with with callback?
    // TODO: to add event response verification
    #[tokio::test(flavor = "multi_thread")]
    async fn test_event() {
        let (mock_sender, mock_rx) = ExtnSender::mock();
        let mut extn_client = ExtnClient::new(mock_rx, mock_sender);
        let processor =
            MockEventProcessor::new_v1(extn_client.clone(), vec![RippleContract::Internal]);

        extn_client.add_event_processor(processor);

        let extn_client_for_thread = extn_client.clone();
        tokio::spawn(async move {
            extn_client_for_thread.initialize().await;
        });

        let response = extn_client.event(MockEvent {
            event_name: "test_event".to_string(),
            result: serde_json::json!({"result": "result"}),
            context: None,
            app_id: Some("some_id".to_string()),
            expected_response: Some(ExtnResponse::Boolean(true)),
        });

        match response {
            Ok(_) => {
                // nothing to assert here
            }
            Err(_) => {
                panic!("Received an unexpected error");
            }
        }

        // how to verify the event response in other sender?
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(
            extn_client.event_processors.read().unwrap().len(),
            0,
            "Assertion failed: event_processors map should be empty after cleanup"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_initialize() {
        let (mock_sender, receiver) = ExtnSender::mock();
        let mut extn_client = ExtnClient::new(receiver, mock_sender.clone());
        let extn_client_thread = extn_client.clone();
        let processor =
            MockRequestProcessor::new_v1(extn_client.clone(), vec![RippleContract::Internal]);

        extn_client.add_request_processor(processor);

        tokio::spawn(async move {
            extn_client_thread.initialize().await;
        });

        let result = extn_client
            .request(MockRequest {
                app_id: "test_app_id".to_string(),
                contract: RippleContract::Internal,
                expected_response: Some(ExtnResponse::Boolean(true)),
            })
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_context_update() {
        let extn_client = ExtnClient::mock();
        let test_string = "TestString".to_string();
        let request = RippleContextUpdateRequest::TimeZone(TimeZone {
            time_zone: test_string.clone(),
            offset: 1,
        });

        extn_client.context_update(request);
        let ripple_context = extn_client.ripple_context.read().unwrap();

        assert!(
            matches!(&ripple_context.time_zone, Some(time_zone) if time_zone.time_zone == test_string && time_zone.offset == 1)
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_handle_no_processor_error() {
        let (mock_sender, mock_rx) = ExtnSender::mock();
        let mut extn_client = ExtnClient::new(mock_rx.clone(), mock_sender.clone());
        let extn_client_for_thread = extn_client.clone();

        tokio::spawn(async move {
            extn_client_for_thread.initialize().await;
        });

        let response = extn_client
            .request(MockRequest {
                app_id: "test_app_id".to_string(),
                contract: RippleContract::Internal,
                expected_response: Some(ExtnResponse::Boolean(true)),
            })
            .await;

        match response {
            Ok(actual_response) => {
                let expected_message = ExtnMessage {
                    id: "some-uuid".to_string(),
                    requestor: ExtnId::get_main_target("main".into()),
                    target: RippleContract::Internal,
                    target_id: None,
                    payload: ExtnPayload::Response(ExtnResponse::Error(
                        RippleError::ProcessorError,
                    )),
                    callback: None,
                    ts: Some(Utc::now().timestamp_millis()),
                };

                assert!(Uuid::parse_str(&actual_response.id).is_ok());
                assert_eq!(actual_response.requestor, expected_message.requestor);
                assert_eq!(actual_response.target, expected_message.target);
                assert_eq!(actual_response.target_id, expected_message.target_id);

                assert_eq!(
                    actual_response.callback.is_some(),
                    expected_message.callback.is_some()
                );
                assert!(actual_response.ts.is_some());
            }
            Err(_) => {
                panic!("Received an unexpected error");
            }
        }
    }

    #[rstest(
        tc,
        exp_resp,
        case("success", ExtnResponse::Boolean(true)),
        case("success", ExtnResponse::Error(RippleError::ProcessorError)),
        case("failure - send resp err", ExtnResponse::Boolean(true)),
        case("failure - resp processor err", ExtnResponse::Boolean(true))
    )]
    #[tokio::test]
    async fn test_handle_single(tc: String, exp_resp: ExtnResponse) {
        testing_logger::setup();
        let extn_client = ExtnClient::mock();
        let id = uuid::Uuid::new_v4().to_string();
        let (tx, rx): (oneshot::Sender<ExtnMessage>, _) = oneshot::channel();

        if tc.contains("send resp err") {
            drop(rx);
        }

        if !tc.contains("resp processor err") {
            add_single_processor(
                id.clone(),
                Some(tx),
                extn_client.response_processors.clone(),
            );
            assert!(extn_client.response_processors.read().unwrap().len() == 1);
        } else {
            extn_client.response_processors.write().unwrap().clear();
            assert!(extn_client.response_processors.read().unwrap().len() == 0);
        }

        let msg = ExtnMessage {
            id,
            requestor: ExtnId::get_main_target("main".into()),
            target: RippleContract::Internal,
            target_id: None,
            payload: ExtnPayload::Response(exp_resp.clone()),
            callback: None,
            ts: Some(Utc::now().timestamp_millis()),
        };

        ExtnClient::handle_single(msg, extn_client.response_processors);
        tokio::time::sleep(Duration::from_millis(10)).await;

        validate(|captured_logs| {
            for log in captured_logs {
                if tc.contains("send resp err") {
                    assert!(log.body.contains("Error sending the response back"));
                } else if tc.contains("resp processor err") {
                    assert!(log.body.contains("No response processor for"));
                } else {
                    assert_ne!(log.body, "Error sending the response back");
                    assert_ne!(log.body, "No response processor for");
                }
            }
        });
    }

    #[rstest(
        tc,
        exp_resp,
        case("success", ExtnResponse::Boolean(true)),
        case("success", ExtnResponse::Error(RippleError::ProcessorError)),
        case("failure - send resp err", ExtnResponse::Boolean(true)),
        case("failure - req processor err", ExtnResponse::Boolean(true))
    )]
    #[tokio::test]
    async fn test_handle_stream(tc: String, exp_resp: ExtnResponse) {
        testing_logger::setup();
        let extn_client = ExtnClient::mock();
        let processor =
            MockRequestProcessor::new_v1(extn_client.clone(), vec![RippleContract::Internal]);

        if tc.contains("req processor err") {
            extn_client.request_processors.write().unwrap().clear();
            assert!(extn_client.request_processors.read().unwrap().len() == 0);
        } else {
            add_stream_processor(
                processor.contract().as_clear_string(),
                processor.sender(),
                extn_client.request_processors.clone(),
            );
            assert!(extn_client.request_processors.read().unwrap().len() == 1);
        }

        add_stream_processor(
            processor.contract().as_clear_string(),
            processor.sender(),
            extn_client.request_processors.clone(),
        );

        assert!(extn_client.request_processors.read().unwrap().len() == 1);

        let msg = ExtnMessage {
            id: "some-id".to_string(),
            requestor: ExtnId::get_main_target("main".into()),
            target: RippleContract::Internal,
            target_id: None,
            payload: ExtnPayload::Response(exp_resp),
            callback: None,
            ts: Some(Utc::now().timestamp_millis()),
        };

        let result = ExtnClient::handle_stream(msg.clone(), extn_client.request_processors);
        tokio::time::sleep(Duration::from_millis(10)).await;

        // TODO - update based on tc
        assert!(result);

        validate(|captured_logs| {
            for log in captured_logs {
                if result {
                    assert_ne!(log.body, "No Request Processor for");
                    assert_ne!(log.body, "Error sending the response back");
                } else {
                    assert!(log.body.contains("No Request Processor for"));
                    assert_ne!(log.body, "Error sending the response back");
                }
            }
        });
    }

    #[rstest(
        _tc,
        exp_resp,
        case("success", ExtnResponse::Boolean(true)),
        case("success", ExtnResponse::Error(RippleError::ProcessorError)),
        case("failure - send resp err", ExtnResponse::Boolean(true)),
        case("failure - req processor err", ExtnResponse::Boolean(true))
    )]
    #[tokio::test]
    async fn test_handle_vec_stream(_tc: String, exp_resp: ExtnResponse) {
        testing_logger::setup();
        let extn_client = ExtnClient::mock();
        let processor =
            MockEventProcessor::new_v1(extn_client.clone(), vec![RippleContract::Internal]);
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

        let msg = ExtnMessage {
            id: "some-id".to_string(),
            requestor: ExtnId::get_main_target("main".into()),
            target: RippleContract::Internal,
            target_id: None,
            payload: ExtnPayload::Response(exp_resp),
            callback: None,
            ts: Some(Utc::now().timestamp_millis()),
        };

        ExtnClient::handle_vec_stream(msg.clone(), extn_client.event_processors.clone());
        tokio::time::sleep(Duration::from_millis(10)).await;

        validate(|captured_logs| {
            for log in captured_logs {
                assert_ne!(log.body, "No Event Processor for");
                assert_ne!(log.body, "Error sending the response back");
            }
        });
    }

    #[tokio::test]
    async fn test_respond() {
        let (mock_sender, mock_rx) = ExtnSender::mock();
        let mut extn_client = ExtnClient::new(mock_rx.clone(), mock_sender.clone());
        let id = uuid::Uuid::new_v4().to_string();

        let (tx, _rx): (oneshot::Sender<ExtnMessage>, _) = oneshot::channel();
        add_single_processor(
            id.clone(),
            Some(tx),
            extn_client.response_processors.clone(),
        );
        assert!(extn_client.response_processors.read().unwrap().len() == 1);

        let req = ExtnMessage {
            id: id.clone(),
            requestor: ExtnId::get_main_target("main".into()),
            target: RippleContract::Internal,
            target_id: None,
            payload: ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::DeviceInfo(
                DeviceInfoRequest::Make,
            ))),
            callback: None,
            ts: Some(Utc::now().timestamp_millis()),
        };

        let response = ExtnResponse::String("test_make".to_string());
        let result = extn_client.respond(req.clone(), response.clone()).await;
        assert!(result.is_ok());

        if let Ok(received_msg) = mock_rx.recv().await {
            assert_eq!(received_msg.id, req.id);
            assert_eq!(received_msg.requestor, "ripple:main:internal:main");
            assert_eq!(received_msg.target, "\"internal\"");
            assert_eq!(received_msg.target_id, "");
            assert_eq!(
                received_msg.payload,
                "{\"Response\":{\"String\":\"test_make\"}}"
            );
        } else {
            panic!("Expected a message to be received");
        }
    }

    #[tokio::test]
    async fn test_send_message() {
        let (mock_sender, mock_rx) = ExtnSender::mock();

        let mut extn_client = ExtnClient::new(mock_rx.clone(), mock_sender.clone());
        let id = uuid::Uuid::new_v4().to_string();

        let (tx, _rx): (oneshot::Sender<ExtnMessage>, _) = oneshot::channel();
        add_single_processor(
            id.clone(),
            Some(tx),
            extn_client.response_processors.clone(),
        );
        assert!(extn_client.response_processors.read().unwrap().len() == 1);

        let msg = ExtnMessage {
            id: id.clone(),
            requestor: ExtnId::get_main_target("main".into()),
            target: RippleContract::Internal,
            target_id: None,
            payload: ExtnPayload::Response(ExtnResponse::String("test_make".to_string())),
            callback: None,
            ts: Some(Utc::now().timestamp_millis()),
        };

        let result = extn_client.send_message(msg.clone()).await;
        assert!(result.is_ok());

        if let Ok(received_msg) = mock_rx.recv().await {
            assert_eq!(received_msg.id, msg.id);
            assert_eq!(received_msg.requestor, "ripple:main:internal:main");
            assert_eq!(received_msg.target, "\"internal\"");
            assert_eq!(received_msg.target_id, "");
            assert_eq!(
                received_msg.payload,
                "{\"Response\":{\"String\":\"test_make\"}}"
            );
        } else {
            panic!("Expected a message to be received");
        }
    }

    #[tokio::test] // TODO: fix the dummy test
    async fn test_standalone_request() {
        let (mock_sender, mock_rx) = ExtnSender::mock();
        let extn_client = ExtnClient::new(mock_rx.clone(), mock_sender.clone());

        // TODO - this is a dummy test, need to add a real test
        if let Ok(ExtnResponse::Value(_v)) = extn_client
            .standalone_request(Config::DistributorServices, 2000)
            .await
        {
            println!("**** Got some successful response from standalone_request");
        } else {
            println!("**** not the expected respone from standalone_request");
        }
    }

    #[tokio::test]
    async fn test_request_transient() {
        let (mock_sender, mock_rx) = ExtnSender::mock();
        let mut extn_client = ExtnClient::new(mock_rx, mock_sender.clone());

        extn_client.add_sender(
            ExtnId::get_main_target("main".into()),
            ExtnSymbol {
                id: "id".to_string(),
                uses: vec!["config".to_string()],
                fulfills: vec!["permissions".to_string()],
                config: None,
            },
            mock_sender.tx,
        );

        assert_eq!(
            extn_client.get_other_senders().len(),
            1,
            "Assertion failed: extn_client.get_other_senders() does not have the expected length"
        );

        let result = extn_client.request_transient(MockRequest {
            app_id: "test_app_id".to_string(),
            contract: RippleContract::Internal,
            expected_response: None,
        });

        match result {
            Ok(response) => {
                println!("test_request_transient: Success: {:?}", response);
            }
            Err(err) => {
                println!("test_request_transient: err: {:?}", err);
                panic!("Received an unexpected error");
            }
        }
    }

    #[rstest(
            config,
            exp_size,
            case([("stack_size".to_string(), "1024".to_string())].iter().cloned().collect::<HashMap<_, _>>(), Some(ExtnStackSize::Min)),
            case(HashMap::new(), None),
        )]
    fn test_get_stack_size(config: HashMap<String, String>, exp_size: Option<ExtnStackSize>) {
        let (mock_sender, mock_rx) = ExtnSender::mock_with_params(
            ExtnId::get_main_target("main".into()),
            Vec::new(),
            Vec::new(),
            Some(config),
        );

        let extn_client = ExtnClient::new(mock_rx, mock_sender);
        let result = extn_client.get_stack_size();

        match result {
            Some(stack_size) => {
                let exp_size_clone = exp_size.clone();
                assert_eq!(stack_size, exp_size_clone.unwrap());
                if exp_size.is_none() {
                    panic!("Expected Some(stack_size), but got None");
                }
            }
            None => {
                if exp_size.is_some() {
                    panic!("Expected Some(stack_size), but got None");
                }
            }
        }
    }

    #[rstest(
        config, expected_value,
        case(Some([("key".to_string(), "val".to_string())].iter().cloned().collect::<HashMap<_, _>>()), Some("val".to_string())),
        case(Some(HashMap::new()), None),
        case(None, None),
    )]
    fn test_get_config(config: Option<HashMap<String, String>>, expected_value: Option<String>) {
        let (mock_sender, _mock_rx) = ExtnSender::mock_with_params(
            ExtnId::get_main_target("main".into()),
            Vec::new(),
            Vec::new(),
            config,
        );

        assert_eq!(mock_sender.get_config("key"), expected_value);
    }

    #[rstest(
        config, expected_value,
        case(Some([("key".to_string(), "true".to_string())].iter().cloned().collect::<HashMap<_, _>>()), true),
        case(Some([("key".to_string(), "false".to_string())].iter().cloned().collect::<HashMap<_, _>>()), false),
        case(Some(HashMap::new()), false),
        case(None, false),
    )]
    fn test_get_bool_config(config: Option<HashMap<String, String>>, expected_value: bool) {
        let (mock_sender, mock_rx) = ExtnSender::mock_with_params(
            ExtnId::get_main_target("main".into()),
            Vec::new(),
            Vec::new(),
            config,
        );
        let extn_client = ExtnClient::new(mock_rx, mock_sender);
        assert_eq!(extn_client.get_bool_config("key"), expected_value);
    }

    #[rstest(id, extn_id, permitted,fulfills, exp_resp,
        case("ext_id", ExtnId::get_main_target("main".into()), vec!["context".to_string()], Vec::new(),  Ok(())),
        case("non_ext_id", ExtnId::get_main_target("main".into()), vec!["context".to_string()], Vec::new(), Ok(())),    
        case("non_ext_id", ExtnId::new_channel(ExtnClassId::Device, "info".to_string()),
        vec!["config".to_string()],
        vec!["device_info".to_string()], Err(RippleError::InvalidAccess))
    )]
    fn test_send_event_with_id(
        id: &str,
        extn_id: ExtnId,
        permitted: Vec<String>,
        fulfills: Vec<String>,
        exp_resp: RippleResponse,
    ) {
        let (mock_sender, mock_rx) =
            ExtnSender::mock_with_params(extn_id, permitted, fulfills, Some(HashMap::new()));
        let extn_client = ExtnClient::new(mock_rx, mock_sender.clone());

        if id != "non_existent_id" {
            extn_client.clone().add_sender(
                ExtnId::get_main_target("main".into()),
                ExtnSymbol {
                    id: id.to_string(),
                    uses: vec!["config".to_string()],
                    fulfills: vec!["permissions".to_string()],
                    config: None,
                },
                mock_sender.tx,
            );

            assert_eq!(
            extn_client.get_other_senders().len(),
            1,
            "Assertion failed: extn_client.get_other_senders() does not have the expected length"
        );
        }

        let actual_response =
            extn_client.send_event_with_id(id, crate::utils::mock_utils::get_mock_event());
        assert_eq!(actual_response, exp_resp);
    }

    #[rstest(id, permitted,fulfills, exp_resp, error_msg,
        case(ExtnId::get_main_target("main".into()), vec!["context".to_string()], Vec::new(), true, "Expected true for the given main target"),    
        case(ExtnId::new_channel(ExtnClassId::Device, "info".to_string()),
        vec!["config".to_string(), "device_info".to_string()],
        vec!["device_info".to_string()], true, "Expected true for the given permitted contract"),
        case(ExtnId::new_channel(ExtnClassId::Device, "info".to_string()),
        vec!["config".to_string()],
        vec!["device_info".to_string()], false, "Expected false for the given non permitted contract")
    )]
    fn test_check_contract_permission(
        id: ExtnId,
        permitted: Vec<String>,
        fulfills: Vec<String>,
        exp_resp: bool,
        error_msg: &str,
    ) {
        let (mock_sender, mock_rx) =
            ExtnSender::mock_with_params(id, permitted, fulfills, Some(HashMap::new()));
        let extn_client = ExtnClient::new(mock_rx, mock_sender);
        let cp = extn_client.check_contract_permitted(RippleContract::DeviceInfo);
        assert_eq!(cp, exp_resp, "{}", error_msg);
    }

    #[rstest(
        id,
        fulfills,
        exp_resp,
        error_msg,
        case(
            ExtnId::get_main_target("main".into()),
            vec!["context".to_string()],
            true,
            "Expected true for the given main target"
        ),
        case(
            ExtnId::new_channel(ExtnClassId::Device, "info".to_string()),
            vec!["config".to_string(), "device_info".to_string()],
            true,
            "Expected true for the given fulfilled contract"
        ),
        case(
            ExtnId::new_channel(ExtnClassId::Device, "info".to_string()),
            vec!["config".to_string()],
            false,
            "Expected false for the given non-fulfilled contract"
        )
    )]
    fn test_contract_fulfillment(
        id: ExtnId,
        fulfills: Vec<String>,
        exp_resp: bool,
        error_msg: &str,
    ) {
        let (mock_sender, mock_rx) =
            ExtnSender::mock_with_params(id, Vec::new(), fulfills, Some(HashMap::new()));
        let extn_client = ExtnClient::new(mock_rx, mock_sender);
        let cp = extn_client.check_contract_fulfillment(RippleContract::DeviceInfo);
        assert_eq!(cp, exp_resp, "{}", error_msg);
    }

    #[tokio::test]
    async fn test_has_token() {
        let (mock_sender, mock_rx) = ExtnSender::mock();
        let extn_client = ExtnClient::new(mock_rx, mock_sender);

        // Set activation status to AccountToken
        {
            let mut ripple_context = extn_client.ripple_context.write().unwrap();
            ripple_context.activation_status = Some(ActivationStatus::AccountToken(AccountToken {
                token: "some_token".to_string(),
                expires: 123,
            }));
        }

        // Check if has_token returns true
        let has_token = extn_client.has_token();
        assert!(has_token);

        // Reset activation status to None
        {
            let mut ripple_context = extn_client.ripple_context.write().unwrap();
            ripple_context.activation_status = None;
        }

        // Check if has_token returns false after resetting activation status
        let has_token_after_reset = extn_client.has_token();
        assert!(!has_token_after_reset);
    }

    #[tokio::test]
    async fn test_get_activation_status() {
        let extn_client = ExtnClient::mock();

        // Set activation status to AccountToken
        {
            let mut ripple_context = extn_client.ripple_context.write().unwrap();
            ripple_context.activation_status = Some(ActivationStatus::AccountToken(AccountToken {
                token: "some_token".to_string(),
                expires: 123,
            }));
        }

        // Check if get_activation_status returns AccountToken
        let activation_status = extn_client.get_activation_status();
        assert_eq!(
            activation_status,
            Some(ActivationStatus::AccountToken(AccountToken {
                token: "some_token".to_string(),
                expires: 123,
            }))
        );

        // Reset activation status to None
        {
            let mut ripple_context = extn_client.ripple_context.write().unwrap();
            ripple_context.activation_status = None;
        }

        // Check if get_activation_status returns None after resetting activation status
        let activation_status_after_reset = extn_client.get_activation_status();
        assert_eq!(activation_status_after_reset, None);
    }

    #[rstest(
        connectivity,
        expected_result,
        case(InternetConnectionStatus::FullyConnected, true),
        case(InternetConnectionStatus::LimitedInternet, true),
        case(InternetConnectionStatus::NoInternet, false),
        case(InternetConnectionStatus::CaptivePortal, false)
    )]
    #[tokio::test]
    async fn test_has_internet(connectivity: InternetConnectionStatus, expected_result: bool) {
        let extn_client = ExtnClient::mock();
        extn_client
            .ripple_context
            .write()
            .unwrap()
            .internet_connectivity = Some(connectivity);

        let has_internet = extn_client.has_internet();
        assert_eq!(has_internet, expected_result);
    }

    #[tokio::test]
    async fn test_get_timezone() {
        let extn_client = ExtnClient::mock();
        let test_timezone = TimeZone {
            time_zone: "America/New_York".to_string(),
            offset: -5,
        };

        extn_client.ripple_context.write().unwrap().time_zone = Some(test_timezone.clone());
        let result = extn_client.get_timezone();
        assert_eq!(result, Some(test_timezone));
    }
}
