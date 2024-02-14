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
        if self.id.is_main() || self.fulfills.contains(&contract.as_clear_string()) {
            true
        } else if let Ok(extn_id) = ExtnId::try_from(contract.as_clear_string()) {
            self.id.eq(&extn_id)
        } else {
            false
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
            debug!(
                "id {:?} not having permission to send contract: {:?}",
                self.id.to_string(),
                payload.get_contract().as_clear_string(),
            );
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
            target_id: "".to_owned(),
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
        let c_event = p.into();
        let msg = CExtnMessage {
            requestor: self.id.to_string(),
            callback: None,
            payload: c_event,
            id,
            target: payload.get_contract().into(),
            target_id: "".to_owned(),
            ts: Utc::now().timestamp_millis(),
        };
        self.respond(msg, other_sender)
    }

    pub fn forward_event(
        &self,
        target_id: &str,
        payload: impl ExtnPayloadProvider,
    ) -> Result<(), RippleError> {
        // Check if sender has permission to forward the payload
        let permitted = self.check_contract_permission(payload.get_contract());
        if permitted || self.get_cap().is_main() {
            let id = uuid::Uuid::new_v4().to_string();
            let p = payload.get_extn_payload();
            let c_event = p.into();
            let msg = CExtnMessage {
                requestor: self.id.to_string(),
                callback: None,
                payload: c_event,
                id,
                target: payload.get_contract().into(),
                target_id: target_id.to_owned(),
                ts: Utc::now().timestamp_millis(),
            };
            self.respond(msg, None)
        } else {
            Err(RippleError::InvalidAccess)
        }
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
    use crate::{api::device::device_info_request::DeviceInfoRequest, extn::extn_id::ExtnClassId};
    use async_channel::Receiver as CReceiver;
    use rstest::rstest;
    use std::collections::HashMap;

    #[cfg(test)]
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

    #[cfg(test)]
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
    #[cfg(test)]
    pub struct ExtnSenderBuilder {
        id: ExtnId,
        context: Vec<String>,
        fulfills: Vec<String>,
        config: Option<HashMap<String, String>>,
    }

    #[cfg(test)]
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
        let (sender, _receiver) = ExtnSender::mock();
        let cap = sender.get_cap();
        let is_main = cap.is_main();
        assert!(is_main, "Expected cap to be main");
    }

    #[rstest(id, permitted,fulfills, exp_resp, error_msg,
        case(ExtnId::get_main_target("main".into()), vec!["context".to_string()], vec!["fulfills".to_string()], true, "Expected true for the given main target"),    
        case(ExtnId::new_channel(ExtnClassId::Device, "info".to_string()),
        vec!["config".to_string(), "device_info".to_string()],
        vec!["device_info".to_string()], true, "Expected true for the given permitted contract"),
        case(ExtnId::new_channel(ExtnClassId::Device, "info".to_string()),
        vec!["config".to_string()],
        vec!["device_info".to_string()], false, "Expected false for the given non permitted contract")
    )]
    fn test_contract_permission(
        id: ExtnId,
        permitted: Vec<String>,
        fulfills: Vec<String>,
        exp_resp: bool,
        error_msg: &str,
    ) {
        let (sender, _receiver) =
            ExtnSender::mock_with_params(id, permitted, fulfills, Some(HashMap::new()));

        let cp = sender.check_contract_permission(RippleContract::DeviceInfo);
        assert_eq!(cp, exp_resp, "{}", error_msg);
    }

    #[rstest]
    #[case("key1", Some("value1".to_string()))] // key exists in config
    #[case("key2", None)] // key does not exist in config
    #[case("key3", None)] // no config
    fn test_get_config(#[case] key: &str, #[case] expected: Option<String>) {
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
                Vec::new(),
                None,
            ),
            _ => ExtnSender::mock(),
        };

        let result = sender.get_config(key);
        assert_eq!(result, expected);
    }

    #[rstest(
        id,
        fulfills,
        contract,
        exp_resp,
        error_msg,
        case(
            ExtnId::get_main_target("main".into()),
            vec!["context".to_string()],
            RippleContract::DeviceInfo,
            true,
            "Expected true for the given main target"
        ),
        case(
            ExtnId::new_channel(ExtnClassId::Device, "info".to_string()),
            vec!["config".to_string(), "device_info".to_string()],
            RippleContract::DeviceInfo,
            true,
            "Expected true for the given fulfilled contract"
        ),
        case(
            ExtnId::new_channel(ExtnClassId::Device, "info".to_string()),
            vec!["config".to_string()],
            RippleContract::DeviceInfo,
            false,
            "Expected false for the given non-fulfilled contract"
        )
    )]
    fn test_contract_fulfillment(
        id: ExtnId,
        fulfills: Vec<String>,
        contract: RippleContract,
        exp_resp: bool,
        error_msg: &str,
    ) {
        let (sender, _receiver) =
            ExtnSender::mock_with_params(id, vec![], fulfills, Some(HashMap::new()));

        let cf = sender.check_contract_fulfillment(contract);
        assert_eq!(cf, exp_resp, "{}", error_msg);
    }

    #[rstest(id, permitted,fulfills, exp_resp, error_msg,
        case(ExtnId::get_main_target("main".into()), vec!["context".to_string()], vec!["fulfills".to_string()], true, "Expected true for the given main target"),    
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
        let (sender, _receiver) =
            ExtnSender::mock_with_params(id, permitted, fulfills, Some(HashMap::new()));

        let cf = sender.check_contract_permission(RippleContract::DeviceInfo);
        assert_eq!(cf, exp_resp, "{}", error_msg);
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

        let result = sender.send_request(
            "some_id".to_string(),
            device_info_request.clone(),
            Some(sender.tx.clone()),
            None,
        );
        assert!(result.is_ok());

        if let Ok(r) = receiver.recv().await {
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

        let result = sender.send_event(device_info_event.clone(), Some(sender.tx.clone()));

        assert!(result.is_ok());
        if let Ok(r) = receiver.recv().await {
            assert_eq!(r.requestor, sender.id.to_string());
            assert_eq!(
                r.target,
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

    #[rstest(id, extn_id, permitted,fulfills, exp_resp,
        case("ext_id", ExtnId::get_main_target("main".into()), vec!["context".to_string()], vec!["fulfills".to_string()],  Ok(())),
        case("non_ext_id", ExtnId::get_main_target("main".into()), vec!["context".to_string()], vec!["fulfills".to_string()], Ok(())),    
        case("non_ext_id", ExtnId::new_channel(ExtnClassId::Device, "info".to_string()),
        vec!["config".to_string()],
        vec!["device_info".to_string()], Err(RippleError::InvalidAccess))
    )]
    async fn test_forward_event(
        id: &str,
        extn_id: ExtnId,
        permitted: Vec<String>,
        fulfills: Vec<String>,
        exp_resp: RippleResponse,
    ) {
        let (sender, _receiver) =
            ExtnSender::mock_with_params(extn_id, permitted, fulfills, Some(HashMap::new()));
        let actual_response = sender.forward_event(id, DeviceInfoRequest::Make);
        assert_eq!(actual_response, exp_resp);
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
            target_id: RippleContract::DeviceInfo.as_clear_string(),
            ts: Utc::now().timestamp_millis(),
        };

        let result = sender.send(msg, Some(sender.tx.clone()));

        assert!(result.is_ok());
        if let Ok(r) = receiver.recv().await {
            assert_eq!(r.requestor, sender.id.to_string());
            assert_eq!(r.target, RippleContract::DeviceInfo.as_clear_string());

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
            ],
            config,
        );

        let msg = CExtnMessage {
            requestor: sender.id.to_string(),
            callback: None,
            payload: device_info_request.get_extn_payload().into(),
            id: "some_id".to_string(),
            target: RippleContract::DeviceInfo.as_clear_string(),
            target_id: RippleContract::DeviceInfo.as_clear_string(),
            ts: Utc::now().timestamp_millis(),
        };

        let result = sender.respond(msg, Some(sender.tx.clone()));
        assert!(result.is_ok());

        if let Ok(r) = receiver.recv().await {
            assert_eq!(r.requestor, sender.id.to_string());
            assert_eq!(r.target, RippleContract::DeviceInfo.as_clear_string());

            let extn_payload = device_info_request.get_extn_payload();
            let exp_payload_str = serde_json::to_string(&extn_payload).unwrap();

            assert_eq!(r.payload, exp_payload_str);
        } else {
            panic!("Expected a message to be received");
        };
    }
}
