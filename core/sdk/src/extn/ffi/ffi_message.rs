use crossbeam::channel::Sender as CSender;

use crate::{
    extn::{
        extn_capability::ExtnCapability,
        extn_client_message::{ExtnMessage, ExtnPayload},
    },
    utils::error::RippleError,
};

/// Contains C Alternates for
/// CExtnRequest
/// CExtnResponse
/// From<CExtnResponse> for ExtnResponse
/// From<ExtnRequest> for CExtnRequest
///
#[repr(C)]
#[derive(Clone, Debug)]
pub struct CExtnMessage {
    pub id: String,
    pub requestor: String,
    pub target: String,
    pub payload: String,
    pub callback: Option<CSender<CExtnMessage>>,
}

impl From<ExtnMessage> for CExtnMessage {
    fn from(value: ExtnMessage) -> Self {
        let payload: String = value.payload.into();
        CExtnMessage {
            callback: value.callback,
            id: value.id,
            payload,
            requestor: value.requestor.to_string(),
            target: value.target.to_string(),
        }
    }
}

impl TryInto<ExtnMessage> for CExtnMessage {
    type Error = RippleError;

    fn try_into(self) -> Result<ExtnMessage, Self::Error> {
        let requestor_capability: Result<ExtnCapability, RippleError> = self.requestor.try_into();
        if requestor_capability.is_err() {
            return Err(RippleError::ParseError);
        }
        let requestor = requestor_capability.unwrap();

        let target_capability: Result<ExtnCapability, RippleError> = self.target.try_into();
        if target_capability.is_err() {
            return Err(RippleError::ParseError);
        }
        let target = target_capability.unwrap();

        let payload: Result<ExtnPayload, RippleError> = self.payload.try_into();
        if payload.is_err() {
            return Err(RippleError::ParseError);
        }
        let payload = payload.unwrap();

        Ok(ExtnMessage {
            callback: self.callback,
            id: self.id,
            requestor,
            target,
            payload,
        })
    }
}
