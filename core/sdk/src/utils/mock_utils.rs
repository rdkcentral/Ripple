use crate::api::session::AccountSession;
use crate::extn::client::extn_client::ExtnClient;
use crate::extn::client::extn_processor::{
    DefaultExtnStreamer, ExtnEventProcessor, ExtnRequestProcessor, ExtnStreamProcessor,
    ExtnStreamer,
};
use crate::extn::client::extn_sender::ExtnSender;
use crate::extn::extn_client_message::{
    ExtnEvent, ExtnMessage, ExtnPayload, ExtnPayloadProvider, ExtnRequest, ExtnResponse,
};
use crate::extn::extn_id::{ExtnClassId, ExtnId};
use crate::extn::ffi::ffi_message::CExtnMessage;
use crate::framework::ripple_contract::RippleContract;

use async_channel::{unbounded, Receiver as CReceiver, Sender as CSender};
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockEvent {
    pub event_name: String,
    pub result: Value,
    pub context: Option<Value>,
    pub app_id: Option<String>,
}

impl ExtnPayloadProvider for MockEvent {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Event(ExtnEvent::Value(
            serde_json::to_value(self.clone()).unwrap(),
        ))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Event(ExtnEvent::Value(v)) = payload {
            if let Ok(v) = serde_json::from_value(v) {
                return Some(v);
            }
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::Internal
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockRequest {
    pub context: MockContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockContext {
    pub app_id: String,
    pub dist_session: AccountSession,
}

impl ExtnPayloadProvider for MockRequest {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::Extn(mock_request)) = payload {
            return Some(serde_json::from_value(mock_request).unwrap());
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Extn(
            serde_json::to_value(self.clone()).unwrap(),
        ))
    }

    // TODO - customize contract from test ?
    fn contract() -> RippleContract {
        RippleContract::Internal
    }
}

// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct MockCustomRequest {
//     pub context: MockContext,
//     pub contract: RippleContract, // Add a contract field to MockRequest
// }

// impl ExtnPayloadProvider for MockCustomRequest {
//     fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
//         if let ExtnPayload::Request(ExtnRequest::Extn(mock_request)) = payload {
//             return Some(serde_json::from_value(mock_request).unwrap());
//         }

//         None
//     }

//     fn get_extn_payload(&self) -> ExtnPayload {
//         ExtnPayload::Request(ExtnRequest::Extn(
//             serde_json::to_value(self.clone()).unwrap(),
//         ))
//     }

//     // Customize contract from the test
//     fn contract(&self) -> RippleContract {
//         self.contract
//     }
// }

pub fn get_mock_extn_client(id: ExtnId) -> ExtnClient {
    let (s, receiver) = unbounded();
    let mock_sender = ExtnSender::new(
        s,
        id,
        vec!["context".to_string()],
        vec!["fulfills".to_string()],
        Some(HashMap::new()),
    );

    ExtnClient::new(receiver, mock_sender)
}

pub fn get_mock_message(payload_type: PayloadType) -> ExtnMessage {
    ExtnMessage {
        id: "test_id".to_string(),
        requestor: ExtnId::get_main_target("main".into()),
        target: RippleContract::Internal,
        payload: match payload_type {
            PayloadType::Event => get_mock_event_payload(),
            PayloadType::Request => get_mock_request_payload(),
        },
        callback: None,
        ts: Some(Utc::now().timestamp_millis()),
    }
}

pub fn get_mock_event_payload() -> ExtnPayload {
    ExtnPayload::Event(ExtnEvent::Value(serde_json::to_value(true).unwrap()))
}

pub fn get_mock_request_payload() -> ExtnPayload {
    ExtnPayload::Request(ExtnRequest::Extn(
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
    ))
}

pub enum PayloadType {
    Event,
    Request,
}

pub fn get_mock_event() -> MockEvent {
    MockEvent {
        event_name: "test_event".to_string(),
        result: serde_json::json!({"result": "result"}),
        context: None,
        app_id: Some("some_id".to_string()),
    }
}

pub fn get_mock_request() -> MockRequest {
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
    }
}
