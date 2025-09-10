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

use serde::{Deserialize, Serialize};

use crate::api::gateway::rpc_gateway_api::CallContext;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct ListenRequest {
    pub listen: bool,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct ListenRequestWithEvent {
    pub event: String,
    pub request: ListenRequest,
    pub context: CallContext,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ListenerResponse {
    pub listening: bool,
    pub event: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Comprehensive JSON serialization/deserialization tests for Firebolt OpenRPC compliance

    #[test]
    fn test_listen_request_json_serialization() {
        let request = ListenRequest { listen: true };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: ListenRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(request, deserialized);
    }

    #[test]
    fn test_listen_request_json_deserialization() {
        let json = r#"{"listen": false}"#;
        let deserialized: ListenRequest = serde_json::from_str(json).unwrap();
        assert!(!deserialized.listen);
    }

    #[test]
    fn test_listen_request_true_value() {
        let json = r#"{"listen": true}"#;
        let deserialized: ListenRequest = serde_json::from_str(json).unwrap();
        assert!(deserialized.listen);
    }

    #[test]
    fn test_listen_request_invalid_json() {
        let invalid_json = r#"{"listen": "not_boolean"}"#;
        let result: Result<ListenRequest, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_listen_request_with_event_json_serialization() {
        let context = CallContext {
            session_id: "test_session".to_string(),
            request_id: "test_request".to_string(),
            app_id: "test_app".to_string(),
            call_id: 123,
            protocol: crate::api::gateway::rpc_gateway_api::ApiProtocol::Bridge,
            method: "test_method".to_string(),
            cid: Some("test_cid".to_string()),
            gateway_secure: false,
            context: vec!["test_context".to_string()],
        };

        let request_with_event = ListenRequestWithEvent {
            event: "test.event".to_string(),
            request: ListenRequest { listen: true },
            context,
        };

        let json = serde_json::to_string(&request_with_event).unwrap();
        let deserialized: ListenRequestWithEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(request_with_event, deserialized);
    }

    #[test]
    fn test_listen_request_with_event_json_deserialization() {
        let json = r#"{
            "event": "device.power",
            "request": {"listen": false},
            "context": {
                "session_id": "session123",
                "request_id": "request456",
                "app_id": "app789",
                "call_id": 789,
                "protocol": "Bridge",
                "method": "power.status",
                "cid": "call123",
                "gateway_secure": true,
                "context": ["test_context"]
            }
        }"#;

        let deserialized: ListenRequestWithEvent = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized.event, "device.power");
        assert!(!deserialized.request.listen);
        assert_eq!(deserialized.context.session_id, "session123");
        assert_eq!(deserialized.context.call_id, 789);
    }

    #[test]
    fn test_listen_request_with_event_invalid_json() {
        let invalid_json = r#"{"event": "test", "request": "invalid"}"#;
        let result: Result<ListenRequestWithEvent, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_listener_response_json_serialization() {
        let response = ListenerResponse {
            listening: true,
            event: "device.test".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: ListenerResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(response.listening, deserialized.listening);
        assert_eq!(response.event, deserialized.event);
    }

    #[test]
    fn test_listener_response_json_deserialization() {
        let json = r#"{
            "listening": false,
            "event": "lifecycle.background"
        }"#;

        let deserialized: ListenerResponse = serde_json::from_str(json).unwrap();
        assert!(!deserialized.listening);
        assert_eq!(deserialized.event, "lifecycle.background");
    }

    #[test]
    fn test_listener_response_invalid_json() {
        let invalid_json = r#"{"listening": 123}"#;
        let result: Result<ListenerResponse, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());
    }
    #[test]
    fn test_complex_listen_request_with_event_roundtrip() {
        let context = CallContext {
            session_id: "complex_session_123".to_string(),
            request_id: "complex_request_456".to_string(),
            app_id: "com.complex.app".to_string(),
            call_id: 9999,
            protocol: crate::api::gateway::rpc_gateway_api::ApiProtocol::Bridge,
            method: "complex.method.test".to_string(),
            cid: Some("complex_cid_789".to_string()),
            gateway_secure: true,
            context: vec!["complex_context".to_string(), "another_context".to_string()],
        };

        let complex_request = ListenRequestWithEvent {
            event: "lifecycle.foreground.background.transition".to_string(),
            request: ListenRequest { listen: false },
            context,
        };

        let json = serde_json::to_string(&complex_request).unwrap();
        let deserialized: ListenRequestWithEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(complex_request, deserialized);
        assert_eq!(
            deserialized.event,
            "lifecycle.foreground.background.transition"
        );
        assert!(!deserialized.request.listen);
        assert_eq!(deserialized.context.app_id, "com.complex.app");
        assert_eq!(deserialized.context.call_id, 9999);
        assert!(deserialized.context.gateway_secure);
        assert_eq!(deserialized.context.context.len(), 2);
    }
}
