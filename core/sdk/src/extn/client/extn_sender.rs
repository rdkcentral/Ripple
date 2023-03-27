// If not stated otherwise in this file or this component's license file the
// following copyright and licenses apply:
//
// Copyright 2023 RDK Management
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
use crossbeam::channel::Sender as CSender;
use log::{debug, error, trace};

use crate::{
    extn::{
        extn_client_message::ExtnPayloadProvider, extn_id::ExtnId, ffi::ffi_message::CExtnMessage,
    },
    framework::{ripple_contract::RippleContract, RippleResponse},
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
    id: ExtnId,
    permitted: Vec<String>,
}

impl ExtnSender {
    pub fn get_cap(&self) -> ExtnId {
        self.id.clone()
    }

    pub fn new(tx: CSender<CExtnMessage>, id: ExtnId, context: Vec<String>) -> Self {
        ExtnSender {
            tx,
            id,
            permitted: context,
        }
    }
    fn check_contract_permission(&self, contract: RippleContract) -> bool {
        if self.id.is_main() {
            true
        } else {
            self.permitted.contains(&contract.into())
        }
    }

    pub fn send_request(
        &self,
        id: String,
        payload: impl ExtnPayloadProvider,
        other_sender: Option<CSender<CExtnMessage>>,
        callback: Option<CSender<CExtnMessage>>,
    ) -> Result<(), RippleError> {
        // Extns can only send request to which it has permissions through Extn manifest
        if !self.check_contract_permission(payload.get_contract()) {
            return Err(RippleError::InvalidAccess);
        }
        let p = payload.get_extn_payload();
        let c_request = p.into();
        let msg = CExtnMessage {
            requestor: self.id.to_string(),
            callback,
            payload: c_request,
            id,
            target: payload.get_contract().into(),
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
            requestor: self.id.to_string(),
            callback: None,
            payload: c_event,
            id,
            target: payload.get_contract().into(),
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
