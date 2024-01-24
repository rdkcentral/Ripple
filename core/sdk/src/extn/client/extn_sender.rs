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

use std::collections::HashMap;

use async_channel::Sender as CSender;
use chrono::Utc;
use log::{error, trace};

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
    pub tx: CSender<CExtnMessage>,
    pub id: ExtnId,
    pub permitted: Vec<String>,
    pub fulfills: Vec<String>,
    pub config: Option<HashMap<String, String>>,
}

impl ExtnSender {
    pub fn get_cap(&self) -> ExtnId {
        self.id.clone()
    }

    pub fn new(
        tx: CSender<CExtnMessage>,
        id: ExtnId,
        context: Vec<String>,
        fulfills: Vec<String>,
        config: Option<HashMap<String, String>>,
    ) -> Self {
        println!("**** ExtnSender: new() called");
        ExtnSender {
            tx,
            id,
            permitted: context,
            fulfills,
            config,
        }
    }
    pub fn check_contract_permission(&self, contract: RippleContract) -> bool {
        if self.id.is_main() {
            true
        } else {
            self.permitted.contains(&contract.as_clear_string())
        }
    }

    pub fn check_contract_fulfillment(&self, contract: RippleContract) -> bool {
        if self.id.is_main() {
            true
        } else {
            self.fulfills.contains(&contract.as_clear_string())
        }
    }

    pub fn get_config(&self, key: &str) -> Option<String> {
        if let Some(c) = &self.config {
            if let Some(v) = c.get(key) {
                return Some(v.clone());
            }
        }
        None
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
            ts: Utc::now().timestamp_millis(),
        };
        self.send(msg, other_sender)
    }

    pub fn send_event(
        &self,
        payload: impl ExtnPayloadProvider,
        other_sender: Option<CSender<CExtnMessage>>,
    ) -> Result<(), RippleError> {
        let id = uuid::Uuid::new_v4().to_string();
        let p = payload.get_extn_payload();
        println!(
            "**** send_event(): get_extn_payload() msg:  {:?}",
            serde_json::to_value(payload.get_extn_payload().clone()).unwrap()
        );
        println!(
            "**** send_event(): other_sender:  {:?}",
            other_sender.clone()
        );
        let c_event = p.into();
        let msg = CExtnMessage {
            requestor: self.id.to_string(),
            callback: None,
            payload: c_event,
            id,
            target: payload.get_contract().into(),
            ts: Utc::now().timestamp_millis(),
        };
        self.respond(msg, other_sender)
    }

    pub fn send(
        &self,
        msg: CExtnMessage,
        other_sender: Option<CSender<CExtnMessage>>,
    ) -> Result<(), RippleError> {
        if let Some(other_sender) = other_sender {
            trace!("Sending message on the other sender");
            if let Err(e) = other_sender.try_send(msg) {
                error!("send() error for message in other sender {}", e.to_string());
                return Err(RippleError::SendFailure);
            }
            Ok(())
        } else {
            let tx = self.tx.clone();
            //tokio::spawn(async move {
            trace!("sending to main channel");
            if let Err(e) = tx.try_send(msg) {
                error!("send() error for message in main sender {}", e.to_string());
                return Err(RippleError::SendFailure);
            }
            Ok(())
        }
    }

    pub fn respond(
        &self,
        msg: CExtnMessage,
        other_sender: Option<CSender<CExtnMessage>>,
    ) -> RippleResponse {
        if msg.callback.is_some() {
            trace!("Sending message on the callback sender");
            if let Err(e) = msg.clone().callback.unwrap().try_send(msg) {
                error!(
                    "respond() error for message in callback sender {}",
                    e.to_string()
                );
                return Err(RippleError::SendFailure);
            }
            Ok(())
        } else {
            self.send(msg, other_sender)
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    pub use crate::{
        api::device::device_info_request::{self, DeviceInfoRequest},
        extn::extn_id::ExtnClassId,
    };
    use async_channel::Receiver as CReceiver;
    use mockall::automock;
    use rstest::rstest;
    use std::collections::HashMap;

    // Mockable trait with generic mock method
    #[automock]
    pub trait Mockable {
        fn mock() -> (Self, CReceiver<CExtnMessage>)
        where
            Self: Sized;
        fn mock_with_params(
            id: ExtnId,
            context: Vec<String>,
            fulfills: Vec<String>,
            config: Option<HashMap<String, String>>,
        ) -> (Self, CReceiver<CExtnMessage>)
        where
            Self: Sized;
    }

    // Implement ExtnSenderMockable for ExtnSender
    impl Mockable for ExtnSender {
        fn mock() -> (Self, CReceiver<CExtnMessage>) {
            ExtnSenderBuilder::new().build()
        }

        fn mock_with_params(
            id: ExtnId,
            context: Vec<String>,
            fulfills: Vec<String>,
            config: Option<HashMap<String, String>>,
        ) -> (Self, CReceiver<CExtnMessage>) {
            ExtnSenderBuilder::new()
                .id(id)
                .context(context)
                .fulfills(fulfills)
                .config(config)
                .build()
        }
    }

    struct ExtnSenderBuilder {
        id: ExtnId,
        context: Vec<String>,
        fulfills: Vec<String>,
        config: Option<HashMap<String, String>>,
    }

    impl ExtnSenderBuilder {
        fn new() -> Self {
            ExtnSenderBuilder {
                id: ExtnId::get_main_target("main".into()),
                context: vec!["context".to_string()],
                fulfills: vec!["fulfills".to_string()],
                config: Some(HashMap::new()),
            }
        }

        fn id(mut self, id: ExtnId) -> Self {
            self.id = id;
            self
        }

        fn context(mut self, context: Vec<String>) -> Self {
            self.context = context;
            self
        }

        fn fulfills(mut self, fulfills: Vec<String>) -> Self {
            self.fulfills = fulfills;
            self
        }

        fn config(mut self, config: Option<HashMap<String, String>>) -> Self {
            self.config = config;
            self
        }

        fn build(self) -> (ExtnSender, CReceiver<CExtnMessage>) {
            let (tx, _rx) = async_channel::unbounded();
            (
                ExtnSender {
                    tx,
                    id: self.id,
                    permitted: self.context,
                    fulfills: self.fulfills,
                    config: self.config,
                },
                _rx,
            )
        }
    }

    #[test]
    fn test_get_cap() {
        // Mock ExtnSender
        let (sender, _receiver) = ExtnSender::mock();

        // Get the capability
        let cap = sender.get_cap();

        // Check if it's expected to be main
        let is_main = cap.is_main();

        // case 1
        assert!(is_main, "Expected cap to be main");

        // case 2  custom mock
        // ExtnSender::mock_with_params(id, custom_context, custom_fulfills, custom_config)
        // assert!(!is_main, "Expected cap not to be main");
    }

    #[test]
    fn test_contract_permission() {
        let (sender, _receiver) = ExtnSender::mock();
        let cp = sender.check_contract_permission(RippleContract::DeviceInfo);
        //case 1 - main
        assert!(cp, "Expected cap to be main");

        // case 2 - custom mock - permitted contract
        // assert!(!cp, "Expected cap to be permitted contract");

        // case 3 - custom mock - non permitted contract
        // assert!(!cp, "Expected cap to be non permitted contract");
    }

    #[test]
    fn test_contract_fulfillment() {
        let (sender, _receiver) = ExtnSender::mock();
        let cp = sender.check_contract_fulfillment(RippleContract::DeviceInfo);
        //case 1 - main
        assert!(cp, "Expected cap to be main");

        // case 2 - custom mock - permitted contract
        // assert!(!cp, "Expected cap to be permitted contract");

        // case 3 - custom mock - non permitted contract
        // assert!(!cp, "Expected cap to be non permitted contract");
    }

    // // { id: "22e045eb-d99d-4fec-8763-022d40e90dd9", requestor: ExtnId { _type: Main, class: Internal, service: "main" }, target: Storage(Local), payload: Request(Device(Storage(Get(GetStorageProperty { namespace: "DeviceName", key: "name" })))), callback: None, ts: Some(1702486068197) }
    // // { id: "22e045eb-d99d-4fec-8763-022d40e90dd9", requestor: ExtnId { _type: Main, class: Internal, service: "main" }, target: Storage(Local), payload: Response(StorageData(StorageData { value: String("work"), update_time: "2023-12-13T16:46:16.473509+00:00" })), callback: None, ts: Some(1702486068214) }
    // // { id: "36483c85-bab0-4939-966c-43185a4e9f1d", requestor: ExtnId { _type: Main, class: Internal, service: "main" }, target: OperationalMetricListener, payload: Event(OperationalMetrics(FireboltInteraction(FireboltInteraction { app_id: "com.bskyb.epgui", method: "device.name", params: Some("[{}]"), tt: 23, success: true, ripple_session_id: "40ca4ca5-00a8-4b73-905f-40c2d39f9579", app_session_id: Some("bb2e1eb6-f14f-40b2-bf19-a9c1760d366a") }))), callback: None, ts: Some(1702486068220) }

    #[rstest]
    #[case("key1", Some("value1".to_string()))] // key exists in config
    #[case("key2", None)] // key does not exist in config
    #[case("key3", None)] // no config
    fn test_get_config(#[case] key: &str, #[case] expected: Option<String>) {
        // Create a mock ExtnSender with the specified config
        let (sender, _receiver) = match key {
            "key1" => ExtnSender::mock_with_params(
                ExtnId::get_main_target("main".into()),
                vec!["context".to_string()],
                vec!["fulfills".to_string()],
                Some(
                    vec![("key1".to_string(), "value1".to_string())]
                        .into_iter()
                        .collect(),
                ),
            ),
            "key2" | "key3" => ExtnSender::mock_with_params(
                ExtnId::get_main_target("main".into()),
                Vec::new(),
                Vec::new(), // Provide an empty Vec<String> for fulfills
                None,
            ),
            _ => ExtnSender::mock(),
        };

        // Call the get_config method and assert the result
        let result = sender.get_config(key);
        assert_eq!(result, expected);
    }

    #[rstest]
    #[case(&DeviceInfoRequest::Make)]
    #[case(&DeviceInfoRequest::Name)]
    async fn test_send_request_with_device_info_request(
        #[case] device_info_request: &DeviceInfoRequest,
    ) {
        let config: Option<HashMap<String, String>> = Some(
            [("rdk_telemetry", "true")]
                .iter()
                .cloned()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        );
        let (sender, receiver) = ExtnSender::mock_with_params(
            ExtnId::new_channel(ExtnClassId::Device, "thunder_comcast".into()),
            vec![
                "config".to_string(),
                "app_events".to_string(),
                "rpc".to_string(),
                "ripple_context".to_string(),
                "operational_metric_listener".to_string(),
                RippleContract::DeviceInfo.as_clear_string(),
            ],
            vec![
                "root.session".to_string(),
                "device.session".to_string(),
                "bridge_protocol".to_string(),
                // Add more fulfills as needed
            ],
            config,
        );

        // Perform the send_request operation with DeviceInfoRequest
        let result = sender.send_request(
            "some_id".to_string(),
            device_info_request.clone(),
            Some(sender.tx.clone()),
            None,
        );
        println!(
            "**** test_send_request_with_device_info_request() result: {:?}",
            result
        );

        // Assertions
        assert!(result.is_ok());
        if let Ok(r) = receiver.recv().await {
            // Print received message for debugging
            println!("Received message: {:?}", r);

            // Assertions based on the received message
            assert_eq!(r.requestor, sender.id.to_string());
            assert_eq!(
                r.target,
                format!("\"{}\"", RippleContract::DeviceInfo.as_clear_string())
            );

            // Generate the ExtnPayload using get_extn_payload
            let extn_payload = device_info_request.get_extn_payload();

            // Convert the ExtnPayload to a JSON string
            let exp_payload_str = serde_json::to_string(&extn_payload).unwrap();

            // Assert the payload matches the expected payload string
            assert_eq!(r.payload, exp_payload_str);
        } else {
            // Handle the case when no message is received
            panic!("Expected a message to be received");
        }
    }

    #[rstest]
    #[case(&DeviceInfoRequest::Make)]
    async fn test_send_event_with_device_info_request(
        #[case] device_info_event: &DeviceInfoRequest,
    ) {
        let config: Option<HashMap<String, String>> = Some(
            [("rdk_telemetry", "true")]
                .iter()
                .cloned()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        );
        let (sender, receiver) = ExtnSender::mock_with_params(
            ExtnId::new_channel(ExtnClassId::Device, "thunder_comcast".into()),
            vec![
                "config".to_string(),
                "app_events".to_string(),
                "rpc".to_string(),
                "ripple_context".to_string(),
                "operational_metric_listener".to_string(),
                RippleContract::DeviceInfo.as_clear_string(),
            ],
            vec![
                "root.session".to_string(),
                "device.session".to_string(),
                "bridge_protocol".to_string(),
                // Add more fulfills as needed
            ],
            config,
        );

        // Perform the send_request operation with DeviceInfoRequest
        let result = sender.send_event(device_info_event.clone(), Some(sender.tx.clone()));
        println!(
            "**** test_send_event_with_device_info_request() result: {:?}",
            result
        );

        // Assertions
        assert!(result.is_ok());
        if let Ok(r) = receiver.recv().await {
            println!("**** test_send_event_with_device_info_request() r: {:?}", r);
            assert_eq!(r.requestor, sender.id.to_string());
            assert_eq!(
                r.target.clone(),
                format!("\"{}\"", RippleContract::DeviceInfo.as_clear_string())
            );

            // Generate the ExtnPayload using get_extn_payload
            let extn_payload = device_info_event.get_extn_payload();

            // Convert the ExtnPayload to a JSON string
            let exp_payload_str = serde_json::to_string(&extn_payload).unwrap();

            assert_eq!(r.payload, exp_payload_str);
        } else {
            panic!("Expected a message to be received");
        };
    }

    #[rstest]
    #[case(&DeviceInfoRequest::Make)]
    async fn test_send_with_device_info_request(#[case] device_info_request: &DeviceInfoRequest) {
        let config: Option<HashMap<String, String>> = Some(
            [("rdk_telemetry", "true")]
                .iter()
                .cloned()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        );
        let (sender, receiver) = ExtnSender::mock_with_params(
            ExtnId::new_channel(ExtnClassId::Device, "thunder_comcast".into()),
            vec![
                "config".to_string(),
                "app_events".to_string(),
                "rpc".to_string(),
                "ripple_context".to_string(),
                "operational_metric_listener".to_string(),
                RippleContract::DeviceInfo.as_clear_string(),
            ],
            vec![
                "root.session".to_string(),
                "device.session".to_string(),
                "bridge_protocol".to_string(),
                // Add more fulfills as needed
            ],
            config,
        );

        let msg = CExtnMessage {
            requestor: sender.id.to_string(),
            callback: None,
            payload: device_info_request.get_extn_payload().into(),
            id: "some_id".to_string(),
            target: RippleContract::DeviceInfo.as_clear_string(),
            ts: Utc::now().timestamp_millis(),
        };

        // Perform the send_request operation with DeviceInfoRequest
        let result = sender.send(msg, Some(sender.tx.clone()));
        println!(
            "**** test_send_with_device_info_request() result: {:?}",
            result
        );

        // Assertions
        assert!(result.is_ok());
        if let Ok(r) = receiver.recv().await {
            println!("**** test_send_with_device_info_request() r: {:?}", r);
            assert_eq!(r.requestor, sender.id.to_string());
            assert_eq!(
                r.target.clone(),
                RippleContract::DeviceInfo.as_clear_string()
            );

            // Generate the ExtnPayload using get_extn_payload
            let extn_payload = device_info_request.get_extn_payload();

            // Convert the ExtnPayload to a JSON string
            let exp_payload_str = serde_json::to_string(&extn_payload).unwrap();

            assert_eq!(r.payload, exp_payload_str);
        } else {
            panic!("Expected a message to be received");
        };
    }

    #[rstest]
    #[case(&DeviceInfoRequest::Make)]
    async fn test_respond_with_device_info_request(
        #[case] device_info_request: &DeviceInfoRequest,
    ) {
        let config: Option<HashMap<String, String>> = Some(
            [("rdk_telemetry", "true")]
                .iter()
                .cloned()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        );
        let (sender, receiver) = ExtnSender::mock_with_params(
            ExtnId::new_channel(ExtnClassId::Device, "thunder_comcast".into()),
            vec![
                "config".to_string(),
                "app_events".to_string(),
                "rpc".to_string(),
                "ripple_context".to_string(),
                "operational_metric_listener".to_string(),
                RippleContract::DeviceInfo.as_clear_string(),
            ],
            vec![
                "root.session".to_string(),
                "device.session".to_string(),
                "bridge_protocol".to_string(),
                // Add more fulfills as needed
            ],
            config,
        );

        let msg = CExtnMessage {
            requestor: sender.id.to_string(),
            callback: None,
            payload: device_info_request.get_extn_payload().into(),
            id: "some_id".to_string(),
            target: RippleContract::DeviceInfo.as_clear_string(),
            ts: Utc::now().timestamp_millis(),
        };

        // Perform the send_request operation with DeviceInfoRequest
        let result = sender.respond(msg, Some(sender.tx.clone()));
        println!(
            "**** test_respond_with_device_info_request() result: {:?}",
            result
        );

        // Assertions
        assert!(result.is_ok());
        if let Ok(r) = receiver.recv().await {
            println!("**** test_respond_with_device_info_request() r: {:?}", r);
            assert_eq!(r.requestor, sender.id.to_string());
            assert_eq!(
                r.target.clone(),
                RippleContract::DeviceInfo.as_clear_string()
            );

            // Generate the ExtnPayload using get_extn_payload
            let extn_payload = device_info_request.get_extn_payload();

            // Convert the ExtnPayload to a JSON string
            let exp_payload_str = serde_json::to_string(&extn_payload).unwrap();

            assert_eq!(r.payload, exp_payload_str);
        } else {
            panic!("Expected a message to be received");
        };
    }
}
