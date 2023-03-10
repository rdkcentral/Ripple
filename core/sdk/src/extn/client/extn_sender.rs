use crossbeam::channel::Sender as CSender;
use log::{debug, error, trace};

use crate::{
    extn::{
        extn_capability::ExtnCapability, extn_client_message::ExtnPayloadProvider,
        ffi::ffi_message::CExtnMessage,
    },
    framework::RippleResponse,
    utils::error::RippleError,
};

/// ExtensionRequestSender will contain a struct with Sender Implementation for the FFI friendly
/// Message channel.
/// Internal Implementation of the managers within an accessor will be exposed with methods
/// that obfuscate the FFI implementation and take in more generic Rust objects
/// Uses FFI converters to convert the data from the Rust Structure to a C Friendly Api
/// Each program boundary will get one callsign which denotes the extension or gateway
/// This callsign is a capability which will be implemented for the Permissions Logic
/// Sender also creates unique uuid to mark each requests
///

#[repr(C)]
#[derive(Clone, Debug)]
pub struct ExtnSender {
    tx: CSender<CExtnMessage>,
    cap: ExtnCapability,
}

impl ExtnSender {
    pub fn get_cap(&self) -> ExtnCapability {
        self.cap.clone()
    }

    pub fn new(tx: CSender<CExtnMessage>, cap: ExtnCapability) -> Self {
        ExtnSender { tx, cap }
    }

    pub fn send_request(
        &self,
        id: String,
        payload: impl ExtnPayloadProvider,
        other_sender: Option<CSender<CExtnMessage>>,
        callback: Option<CSender<CExtnMessage>>,
    ) -> Result<(), RippleError> {
        let p = payload.get_extn_payload();
        let c_request = p.into();
        let msg = CExtnMessage {
            requestor: self.cap.to_string(),
            callback,
            payload: c_request,
            id,
            target: payload.get_capability().to_string(),
        };
        self.send(msg, other_sender)
    }

    pub async fn send_event(
        &self,
        payload: impl ExtnPayloadProvider,
        other_sender: Option<CSender<CExtnMessage>>,
    ) -> Result<(), RippleError> {
        let id = uuid::Uuid::new_v4().to_string();
        let p = payload.get_extn_payload();
        let c_event = p.into();
        let msg = CExtnMessage {
            requestor: self.cap.to_string(),
            callback: None,
            payload: c_event,
            id,
            target: payload.get_capability().to_string(),
        };
        self.respond(msg, other_sender)
    }

    pub fn send(
        &self,
        msg: CExtnMessage,
        other_sender: Option<CSender<CExtnMessage>>,
    ) -> Result<(), RippleError> {
        if other_sender.is_some() {
            debug!("Sending message on the other sender");
            if let Err(e) = other_sender.unwrap().send(msg) {
                error!("send error for message {:?}", e);
                return Err(RippleError::SendFailure);
            }
            return Ok(());
        } else {
            let tx = self.tx.clone();
            //tokio::spawn(async move {
            trace!("sending to main channel");
            if let Err(e) = tx.send(msg) {
                error!("send error for message {:?}", e);
                return Err(RippleError::SendFailure);
            }
            return Ok(());
        }
    }

    pub fn respond(
        &self,
        msg: CExtnMessage,
        other_sender: Option<CSender<CExtnMessage>>,
    ) -> RippleResponse {
        if msg.callback.is_some() {
            debug!("Sending message on the callback sender");
            if let Err(e) = msg.clone().callback.unwrap().send(msg) {
                error!("send error for message {:?}", e);
                return Err(RippleError::SendFailure);
            }
            return Ok(());
        } else {
            return self.send(msg, other_sender);
        }
    }
}
