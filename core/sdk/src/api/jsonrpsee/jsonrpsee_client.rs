use crossbeam::channel::Sender as CSender;
use log::error;
use tokio::sync::{
    mpsc::Sender as MSender,
    oneshot::{self, Sender as OSender},
};

use crate::{
    extn::{
        client::extn_client_message::{ExtnMessage, ExtnPayloadProvider},
        manager::types::{ExtnCapability, ExtnClass, ExtnClassType},
    },
    utils::error::RippleError,
};

#[derive(Debug,Clone)]
pub struct JsonRpseeClient {
    sender: CSender<JsonRpseeMessage>,
    cap: ExtnCapability,
}

pub struct JsonRpseeMessage {
    pub msg: ExtnMessage,
    pub callback: OSender<ExtnMessage>,
    pub stream: Option<MSender<ExtnMessage>>,
}

impl JsonRpseeClient {
    fn get_message(&self, request: impl ExtnPayloadProvider) -> ExtnMessage {
        ExtnMessage {
            id: uuid::Uuid::new_v4().to_string(),
            requestor: self.cap.clone(),
            target: request.get_capability(),
            payload: request.get_extn_payload(),
            callback: None,
        }
    }

    pub async fn call(
        &self,
        request: impl ExtnPayloadProvider,
    ) -> Result<ExtnMessage, RippleError> {
        let (otx, otr) = oneshot::channel();
        let msg = JsonRpseeMessage {
            msg: self.get_message(request),
            callback: otx,
            stream: None,
        };

        if let Err(e) = self.sender.send(msg) {
            error!("Error sending {:?}", e);
            return Err(RippleError::ExtnError);
        }

        if let Ok(v) = otr.await {
            return Ok(v);
        }

        Err(RippleError::InvalidOutput)
    }
}

#[derive(Debug,Clone)]
pub struct JsonRpseeClientBuilder {
    pub sender: CSender<JsonRpseeMessage>,
}

impl JsonRpseeClientBuilder {
    pub fn build(&self, service: String) -> JsonRpseeClient {
        JsonRpseeClient {
            sender: self.sender.clone(),
            cap: ExtnClassType::new(
                crate::extn::manager::types::ExtnType::Extn,
                ExtnClass::Jsonrpsee,
            )
            .get_cap(service),
        }
    }
}
