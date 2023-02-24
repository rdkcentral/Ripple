use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crossbeam::channel::{Receiver as CReceiver, Sender as CSender, TryRecvError};
use log::{debug, error, info, trace};
use tokio::sync::{
    mpsc::Sender as MSender,
    oneshot::{self, Sender as OSender},
};

use crate::{
    extn::{
        extn_capability::ExtnCapability,
        extn_client_message::{ExtnMessage, ExtnPayloadProvider},
        ffi::ffi_message::CExtnMessage,
    },
    utils::error::RippleError,
};

use super::{
    extn_processor::{ExtnEventProcessor, ExtnRequestProcessor},
    extn_sender::ExtnSender,
};

/// Defines the SDK Client implementation of the Inter Extension communication.
/// # Overview
/// Core objective for the Extn client is to provide a reliable and robust communication channel between the  `Main` and its extensions. There are challenges when using Dynamic Linked libraries which needs to be carefully handled for memory, security and reliability. `Client` is built into the `core/sdk` for a better Software Delivery and Operational(SDO) performance.
/// Each client within an extension contains the below fields
/// 1. `reciever` - Crossbeam Receiver which is connected to the processors for handling incoming messages
/// 2. `sender` - Crossbeam Sender to send the request back to `Main` application
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
    response_processors: Arc<RwLock<HashMap<String, OSender<ExtnMessage>>>>,
    request_processors: Arc<RwLock<HashMap<String, MSender<ExtnMessage>>>>,
    event_processors: Arc<RwLock<HashMap<String, Vec<MSender<ExtnMessage>>>>>,
}

fn add_stream_processor<P>(id: String, context: P, map: Arc<RwLock<HashMap<String, P>>>) {
    let mut processor_state = map.write().unwrap();
    processor_state.insert(id, context);
}

fn add_vec_stream_processor<P>(id: String, context: P, map: Arc<RwLock<HashMap<String, Vec<P>>>>) {
    let mut processor_state = map.write().unwrap();
    if processor_state.contains_key(&id) {
        processor_state.get_mut(&id).unwrap().push(context)
    } else {
        processor_state.insert(id, vec![context]);
    }
}

fn add_single_processor<P>(id: String, processor: Option<P>, map: Arc<RwLock<HashMap<String, P>>>) {
    if processor.is_some() {
        let mut processor_state = map.write().unwrap();
        processor_state.insert(id, processor.unwrap());
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
    /// `receiver` - Crossbeam Receiver provided by the `Main` Application for IEC
    ///
    /// `sender` - [ExtnSender] object provided by `Main` Application with a unique [ExtnCapability]
    pub fn new(receiver: CReceiver<CExtnMessage>, sender: ExtnSender) -> ExtnClient {
        ExtnClient {
            receiver,
            sender,
            extn_sender_map: Arc::new(RwLock::new(HashMap::new())),
            response_processors: Arc::new(RwLock::new(HashMap::new())),
            request_processors: Arc::new(RwLock::new(HashMap::new())),
            event_processors: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Adds a new request processor reference to the internal map of processors
    ///
    /// Uses the capability provided by the Processor for registration
    ///
    /// Also starts the thread in the processor to accept incoming requests.
    pub fn add_request_processor(&mut self, mut processor: impl ExtnRequestProcessor) {
        let processor_string = processor.capability().to_string();
        info!("adding request processor {}", processor_string);
        add_stream_processor(
            processor_string.clone(),
            processor.sender(),
            self.request_processors.clone(),
        );
        tokio::spawn(async move {
            trace!("starting request processor thread for {}", processor_string);
            processor.run().await
        });
    }

    /// Removes a request processor reference on the internal map of processors
    pub fn remove_request_processor(&mut self, capability: ExtnCapability) {
        remove_processor(capability.to_string(), self.request_processors.clone());
    }

    /// Adds a new event processor reference to the internal map of processors
    ///
    /// Uses the capability provided by the Processor for registration
    ///
    /// Also starts the thread in the processor to accept incoming events.
    pub fn add_event_processor(&mut self, mut processor: impl ExtnEventProcessor) {
        add_vec_stream_processor(
            processor.capability().to_string(),
            processor.sender(),
            self.event_processors.clone(),
        );
        tokio::spawn(async move { processor.run().await });
    }

    /// Removes an event processor reference on the internal map of processors
    pub fn cleanup_event_stream(&mut self, capability: ExtnCapability) {
        Self::cleanup_vec_stream(capability.to_string(), None, self.event_processors.clone());
    }

    /// Used mainly by `Main` application to add senders of the extensions for IEC
    pub fn add_sender(&mut self, capability: ExtnCapability, sender: CSender<CExtnMessage>) {
        let mut sender_map = self.extn_sender_map.write().unwrap();
        sender_map.insert(capability.get_short(), sender);
    }

    /// Called once per client initialization this is a blocking method. Use a spawned thread to call this method
    pub async fn initialize(&self) {
        debug!("Starting initialize");
        let receiver = self.receiver.clone();
        let mut index: u32 = 0;
        loop {
            index = index + 1;
            match receiver.try_recv() {
                Ok(c_message) => {
                    trace!("** receiving message {:?}", c_message);
                    let message_result: Result<ExtnMessage, RippleError> =
                        c_message.clone().try_into();
                    if message_result.is_err() {
                        error!("invalid message {:?}", c_message);
                        continue;
                    }
                    let message = message_result.unwrap();
                    if message.is_response() {
                        Self::handle_single(message, self.response_processors.clone());
                    } else if message.is_event() {
                        Self::handle_vec_stream(message, self.event_processors.clone());
                    } else {
                        let current_cap = self.sender.get_cap();
                        let message_target_cap = message.clone().target;
                        if current_cap.match_layer(message_target_cap.clone()) {
                            Self::handle_stream(message, self.request_processors.clone());
                        } else {
                            // Forward the message to an extn sender or return error
                            if let Some(sender) = self.get_extn_sender(message_target_cap) {
                                let mut new_message = message.clone();
                                if new_message.callback.is_none() {
                                    // before forwarding check if the requestor needs to be added as callback
                                    let req_sender = if let Some(requestor_sender) =
                                        self.get_extn_sender(message.clone().requestor)
                                    {
                                        Some(requestor_sender)
                                    } else {
                                        None
                                    };
                                    if req_sender.is_some() {
                                        let _ = new_message.callback.insert(req_sender.unwrap());
                                    }
                                }

                                tokio::spawn(async move {
                                    if let Err(e) = sender.send(new_message.into()) {
                                        error!("Error forwarding request {:?}", e)
                                    }
                                });
                            } else {
                                error!("No Request handler for {:?}", message);
                            }
                        }
                    }
                }
                Err(e) => match e {
                    TryRecvError::Disconnected => break,
                    _ => {}
                },
            }
            if index % 200 == 0 {
                index = 0;
                debug!("Receiver still running");
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        debug!("Initialize Ended Abruptly");
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

        if processor_result.is_some() {
            tokio::spawn(async move {
                if let Err(e) = processor_result.unwrap().send(msg) {
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
    ) {
        let id_c = msg.clone().target.to_string();

        let v = {
            let processors = processor.read().unwrap();
            processors.get(&id_c).cloned()
        };
        if v.is_some() {
            let sender = v.clone().unwrap();
            tokio::spawn(async move {
                if let Err(e) = sender.send(msg.clone()).await {
                    error!("Error sending the response back {:?}", e);
                }
            });
        } else {
            error!("No Request Processor for {:?}", msg);
        }
    }

    fn handle_vec_stream(
        msg: ExtnMessage,
        processor: Arc<RwLock<HashMap<String, Vec<MSender<ExtnMessage>>>>>,
    ) {
        let id_c = msg.clone().target.to_string();
        let read_processor = processor.clone();
        let processors = read_processor.read().unwrap();
        let v = processors.get(&id_c).cloned();
        let mut gc_sender_indexes: Vec<usize> = Vec::new();
        if v.is_some() {
            let v = v.clone().unwrap();
            for (index, s) in v.iter().enumerate() {
                let sender = s.clone();
                let msg_c = msg.clone();
                if sender.is_closed() {
                    gc_sender_indexes.push(index);
                } else {
                    tokio::spawn(async move {
                        if let Err(e) = sender.try_send(msg_c) {
                            error!("Error sending the response back {:?}", e);
                        }
                    });
                }
            }
        } else {
            error!("No Event Processor for {:?}", msg);
        }

        Self::cleanup_vec_stream(id_c, Some(gc_sender_indexes), processor);
    }

    fn cleanup_vec_stream(
        id_c: String,
        gc_sender_indexes: Option<Vec<usize>>,
        processor: Arc<RwLock<HashMap<String, Vec<MSender<ExtnMessage>>>>>,
    ) {
        let indices = match gc_sender_indexes {
            Some(i) => Some(i),
            None => match processor.read().unwrap().get(&id_c.clone()) {
                Some(v) => Some(
                    v.iter()
                        .filter(|x| x.is_closed())
                        .enumerate()
                        .map(|(i, _)| i)
                        .collect(),
                ),
                None => None,
            },
        };
        if indices.is_some() {
            let indices = indices.unwrap();
            if indices.len() > 0 {
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

    fn get_extn_sender(&self, cap: ExtnCapability) -> Option<CSender<CExtnMessage>> {
        let key = cap.get_short();
        self.extn_sender_map.read().unwrap().get(&key).cloned()
    }

    /// Critical method used by request processors to send response message back to the requestor
    /// # Arguments
    /// `msg` - [ExtnMessage] response object
    pub async fn respond(&mut self, msg: ExtnMessage) -> Result<(), RippleError> {
        self.sender
            .respond(msg.clone().into(), self.get_extn_sender(msg.clone().target))
    }

    /// Critical method used by event processors to emit event back to the requestor
    /// # Arguments
    /// `msg` - [ExtnMessage] event object
    pub async fn event(&mut self, event: impl ExtnPayloadProvider) -> Result<(), RippleError> {
        self.sender.send_event(event).await
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
        let other_sender = self.get_extn_sender(payload.get_capability());

        if let Err(e) = self.sender.send_request(id, payload, other_sender, None) {
            return Err(e);
        }
        if let Ok(r) = rx.await {
            return Ok(r);
        }

        Err(RippleError::ExtnError)
    }
}
