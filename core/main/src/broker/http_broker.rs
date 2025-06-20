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

use std::vec;

use hyper::{client::HttpConnector, Body, Client, Method, Request, Response, Uri};
use ripple_sdk::{
    api::{
        gateway::rpc_gateway_api::JsonRpcApiError,
        observability::log_signal::LogSignal,
        rules_engine::{jq_compile, RuleTransformType},
    },
    log::{debug, error},
    tokio::{self, sync::mpsc},
    utils::error::RippleError,
};
use serde_json::Value;

use super::endpoint_broker::{
    BrokerCallback, BrokerCleaner, BrokerConnectRequest, BrokerOutputForwarder, BrokerRequest,
    BrokerSender, EndpointBroker, EndpointBrokerState, BROKER_CHANNEL_BUFFER_SIZE,
};

use crate::state::platform_state::PlatformState;

use ripple_sdk::tokio_tungstenite::tungstenite::http::uri::InvalidUri;

pub struct HttpBroker {
    sender: BrokerSender,
    cleaner: BrokerCleaner,
}

async fn send_http_request(
    client: &Client<HttpConnector>,
    uri: &Uri,
    broker_request: BrokerRequest,
) -> Result<Response<Body>, RippleError> {
    let mut method = Method::GET;
    let mut body = Body::empty();

    // A rule with a request transform defined indicates that the request is a POST, where
    // the request transform is the body of the request. Otherwise, it is a GET request.

    if let Some(request_transform) = broker_request
        .rule
        .transform
        .get_transform_data(RuleTransformType::Request)
    {
        method = Method::POST;

        let transform_params =
            match serde_json::from_str::<Vec<Value>>(&broker_request.rpc.params_json) {
                Ok(mut params) => params.pop().unwrap_or(Value::Null),
                Err(e) => {
                    error!(
                        "send_http_request: Error in http broker parsing request params: e={:?}",
                        e
                    );
                    Value::Null
                }
            };

        let body_val = jq_compile(
            transform_params,
            &request_transform,
            format!("{}_http_post", broker_request.rpc.ctx.method),
        )?;

        body = Body::from(body_val.to_string());
    }

    let uri: Uri = format!("{}{}", uri, broker_request.rule.alias)
        .parse()
        .map_err(|e: InvalidUri| RippleError::BrokerError(e.to_string()))?;

    debug!("http_broker sending {} request={}", method, uri,);

    let http_request = Request::builder()
        .uri(uri)
        .method(method)
        .body(body)
        .map_err(|e| RippleError::BrokerError(e.to_string()))?;

    match client.request(http_request).await {
        Ok(v) => Ok(v),
        Err(e) => {
            error!("Error in server");
            Err(RippleError::BrokerError(e.to_string()))
        }
    }
}

async fn send_broker_response(
    callback: &BrokerCallback,
    request: &BrokerRequest,
    body: &[u8],
) -> Result<(), RippleError> {
    match BrokerOutputForwarder::handle_non_jsonrpc_response(
        body,
        callback.clone(),
        request.clone(),
    ) {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}
fn error_string_to_json(msg: &str) -> serde_json::Value {
    serde_json::json!({
        "error": msg
    })
}
async fn body_to_bytes(body: Body) -> Vec<u8> {
    match hyper::body::to_bytes(body).await {
        Ok(bytes) => {
            let value: Vec<u8> = bytes.into();
            value.as_slice().to_vec()
        }
        Err(e) => format!("error in http broker transforming body to bytes {}", e)
            .to_string()
            .as_bytes()
            .to_vec(),
    }
}

impl EndpointBroker for HttpBroker {
    fn get_broker(
        _ps: Option<PlatformState>,
        request: BrokerConnectRequest,
        callback: BrokerCallback,
        _broker_state: &mut EndpointBrokerState,
    ) -> Self {
        let endpoint = request.endpoint.clone();
        let (tx, mut tr) = mpsc::channel(BROKER_CHANNEL_BUFFER_SIZE);
        let broker = BrokerSender { sender: tx };
        let client = Client::new();

        let _ =  endpoint.get_url().parse().map_err(|e| error!("broker url {:?} in endpoint is invalid, cannot start http broker. error={}",endpoint,e) ).map(|uri| tokio::spawn(async move {
            while let Some(mut request) = tr.recv().await {
                LogSignal::new("http_broker".to_string(), format!("received request - start processing request={:?}", request), request.rpc.ctx.clone())
                    .with_diagnostic_context_item("rule_alias", request.rule.alias.as_str()).emit_debug();

                match send_http_request(&client, &uri, request.clone())
                    .await
                {
                    Ok(response) => {
                        LogSignal::new(
                            "http_broker".to_string(),
                            format!("received response={:?} to request: {:?} using rule={:?}", response, request, request.rule),
                            request.rpc.ctx.clone(),
                        )
                        .emit_debug();

                        let (parts, body) = response.into_parts();
                        let body = body_to_bytes(body).await;

                        if !body.is_empty() {
                            if let Ok(json_str) = serde_json::from_slice::<serde_json::Value>(&body).map(|v| vec![v])
                            .and_then(|v| serde_json::to_string(&v))
                            {
                                request.rpc.params_json = json_str;
                                send_broker_response(&callback, &request, &body).await.ok();
                            } else {
                                let msg = format!("Error in http broker parsing response from http service at {}. status={:?}",uri, parts.status);
                                LogSignal::new("http_broker".to_string(), "Prepare request failed".to_string(), request.rpc.ctx.clone())
                                    .with_diagnostic_context_item("error", &msg)
                                    .emit_error();
                                Self::send_broker_failure_response(&callback,
                                    JsonRpcApiError::default()
                                    .with_id(request.rpc.ctx.call_id)
                                    .with_message(msg.to_string()).into());
                            }
                        } else {
                            send_broker_response(&callback, &request, &body).await.ok();
                        }

                        if !parts.status.is_success() {
                            LogSignal::new("http_broker".to_string(), "Prepare request failed".to_string(), request.rpc.ctx.clone())
                            .with_diagnostic_context_item("error", &format!("http error {} returned from http service in http broker {:?}",
                                parts.status, body))
                            .emit_error();
                        }
                    }
                    Err(err) => {
                        let msg = format!("An error message from calling the downstream http service={} in http broker {:?}", uri, err);
                        LogSignal::new("http_broker".to_string(), "Prepare request failed".to_string(), request.rpc.ctx.clone())
                                .with_diagnostic_context_item("error", &msg)
                                .emit_error();
                        let _ = send_broker_response(&callback, &request,  error_string_to_json(msg.as_str()).to_string().as_bytes()).await;
                    }
                }
            }
        }));

        Self {
            sender: broker,
            cleaner: BrokerCleaner { cleaner: None },
        }
    }

    fn get_sender(&self) -> BrokerSender {
        self.sender.clone()
    }

    fn get_cleaner(&self) -> BrokerCleaner {
        self.cleaner.clone()
    }
}
#[cfg(test)]
mod tests {
    use httpmock::prelude::*;
    use hyper::{Client, StatusCode, Uri};
    use serde_json::{json, Value};
    use std::time::Duration;

    use crate::broker::endpoint_broker::BrokerOutput;

    use super::*;

    use ripple_sdk::{
        api::{
            gateway::rpc_gateway_api::{JsonRpcApiResponse, RpcRequest},
            rules_engine::{Rule, RuleEndpoint, RuleEndpointProtocol},
        },
        tokio::{runtime::Runtime, task::JoinHandle, time::timeout},
        Mockable,
    };

    //helper functions

    pub fn get_base_uri_from_mock_server() -> Uri {
        // Start a mock HTTP server.
        let mock_server = MockServer::start();

        mock_server.mock(|when, then| {
            when.method(GET) // Use http::Method::GET
                .path("/test_rule");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(serde_json::json!({"data": "success"}));
        });

        mock_server.base_url().parse::<Uri>().unwrap()
    }

    pub fn result_contains_string(resp: &JsonRpcApiResponse, search_string: &str) -> bool {
        if let Some(Value::Object(obj)) = &resp.result {
            obj.iter().any(|(_, value)| {
                if let Value::String(s) = value {
                    s.contains(search_string)
                } else {
                    false
                }
            })
        } else if let Some(Value::Object(obj)) = &resp.error {
            obj.iter().any(|(_, value)| {
                if let Value::String(s) = value {
                    s.contains(search_string)
                } else {
                    false
                }
            })
        } else {
            false
        }
    }

    async fn send_and_receive_broker_output(
        base_uri: Uri,
        rule_alias: &str,
    ) -> Result<Option<BrokerOutput>, mpsc::Receiver<BrokerOutput>> {
        let endpoint = RuleEndpoint {
            url: base_uri.to_string(),
            protocol: RuleEndpointProtocol::Http,
            jsonrpc: false,
        };

        let (tx, _) = mpsc::channel(BROKER_CHANNEL_BUFFER_SIZE);
        let (btx, mut brx) = mpsc::channel::<BrokerOutput>(BROKER_CHANNEL_BUFFER_SIZE);
        let request = BrokerConnectRequest::new("somekey".to_owned(), endpoint, tx);
        let callback = BrokerCallback { sender: btx };
        let mut broker_state = EndpointBrokerState::default();

        let broker = HttpBroker::get_broker(None, request, callback.clone(), &mut broker_state);
        let sender = broker.get_sender();

        let mut rpc_request = RpcRequest::mock();
        rpc_request.ctx.call_id = 11;

        let broker_request = BrokerRequest {
            rpc: rpc_request.clone(),
            rule: Rule {
                alias: rule_alias.to_string(),
                ..Default::default()
            },
            subscription_processed: None,
            workflow_callback: Some(callback.clone()),
            telemetry_response_listeners: vec![],
        };

        sender.sender.send(broker_request).await.unwrap();

        Ok(timeout(Duration::from_secs(5), brx.recv())
            .await
            .unwrap_or(None))
    }

    fn get_mock_broker_request(path: &str) -> BrokerRequest {
        BrokerRequest {
            rpc: RpcRequest::mock(),
            rule: Rule {
                alias: path.to_string(),
                endpoint: Some("http".to_string()),
                ..Default::default()
            },
            subscription_processed: None,
            workflow_callback: None,
            telemetry_response_listeners: vec![],
        }
    }

    #[test]
    fn test_send_broker_response_json_body() {
        let rt = Runtime::new().unwrap();
        let callback = BrokerCallback::default();
        let request = BrokerRequest::default();
        let body = b"{\"key\": \"value\"}";

        rt.block_on(async {
            let response = send_broker_response(&callback, &request, body).await;
            assert!(response.is_ok(), "send_broker_response return Ok(())");
        });
    }

    #[test]
    fn test_send_broker_response_with_non_json_body() {
        let rt = Runtime::new().unwrap();
        let callback = BrokerCallback::default();
        let request = BrokerRequest::default();
        let body = b"test response";

        rt.block_on(async {
            let response = send_broker_response(&callback, &request, body).await;
            assert_eq!(Err(RippleError::ParseError), response);
        });
    }

    #[test]
    fn test_error_string_to_json() {
        let msg = "test error";
        let json = error_string_to_json(msg);
        assert_eq!(json["error"], msg);
    }

    #[test]
    fn test_body_to_bytes() {
        let rt = Runtime::new().unwrap();
        let body = Body::from("test body");

        rt.block_on(async {
            let bytes = body_to_bytes(body).await;
            assert_eq!(bytes, b"test body");
        });
    }

    #[test]
    fn test_get_broker() {
        let request = BrokerConnectRequest::default();
        let callback = BrokerCallback::default();
        let mut broker_state = EndpointBrokerState::default();

        let broker = HttpBroker::get_broker(None, request, callback, &mut broker_state);
        assert!(broker.get_sender().sender.is_closed());
        assert!(broker.get_cleaner().cleaner.is_none());
    }

    #[tokio::test]
    async fn test_http_broker_get_broker_success() {
        let base_uri = get_base_uri_from_mock_server();
        let output_result = send_and_receive_broker_output(base_uri, "test_rule").await;

        if let Ok(Some(output)) = output_result {
            assert!(output.data.is_success());
        } else {
            panic!("Timeout or channel closed without receiving data");
        }
    }

    #[tokio::test]
    async fn test_http_broker_get_broker_error_check_invalid_uri() {
        let invalid_uri: Uri = "http://127.0.0.1:1234/".parse().unwrap();
        let output_result = send_and_receive_broker_output(invalid_uri, "test_rule").await;

        if let Ok(Some(output)) = output_result {
            let has_error_in_result = if let Some(Value::Object(obj)) = &output.data.result {
                obj.contains_key("error")
            } else {
                false
            };
            assert!(has_error_in_result);
            assert!(result_contains_string(
                &output.data,
                "An error message from calling the downstream http service"
            ));
        } else {
            panic!("Timeout or channel closed without receiving data");
        }
    }

    #[tokio::test]
    async fn test_http_broker_get_broker_error_check_invalid_rule() {
        let base_uri = get_base_uri_from_mock_server();
        let output_result = send_and_receive_broker_output(base_uri, "test").await;

        if let Ok(Some(output)) = output_result {
            assert!(result_contains_string(
                &output.data,
                "Request did not match any route or mock"
            ));
        } else {
            panic!("Timeout or channel closed without receiving data");
        }
    }

    #[tokio::test]
    async fn test_http_broker_get_broker_error_check_invalid_json_body() {
        let mock_server = MockServer::start();

        mock_server.mock(|when, then| {
            when.method(GET) // Use http::Method::GET
                .path("/test_rule");
            then.status(200)
                .header("Content-Type", "application/json")
                .body("hai"); //Invalid json body
        });

        let base_uri = mock_server.base_url().parse::<Uri>().unwrap();
        let output_result = send_and_receive_broker_output(base_uri, "test_rule").await;

        if let Ok(Some(output)) = output_result {
            assert!(output.data.error.is_some());
            assert!(result_contains_string(
                &output.data,
                "Error in http broker parsing response from http service at"
            ));
        } else {
            panic!("Timeout or channel closed without receiving data");
        }
    }

    #[tokio::test]
    async fn test_send_http_request_multiple_concurrent() {
        let mock_server = MockServer::start();

        mock_server.mock(|when, then| {
            when.method(GET) // Use http::Method::GET for clarity
                .path("/test_rule1");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(serde_json::json!({"data": "response1"}));
        });

        mock_server.mock(|when, then| {
            when.method(GET) // Use http::Method::GET for clarity
                .path("/test_rule2");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(serde_json::json!({"data": "response2"}));
        });

        mock_server.mock(|when, then| {
            when.method(GET) // Use http::Method::GET for clarity
                .path("/test_rule3");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(serde_json::json!({"data": "response3"}));
        });

        let base_uri = mock_server.base_url().parse::<Uri>().unwrap();
        let client = Client::new();

        let broker_requests = vec![
            get_mock_broker_request("test_rule1"),
            get_mock_broker_request("test_rule2"),
            get_mock_broker_request("test_rule3"),
        ];

        let mut handles = vec![];

        for broker_request in broker_requests {
            let client_clone = client.clone();
            let uri_clone = base_uri.clone();
            let broker_request_clone = broker_request.clone();
            let handle: JoinHandle<Result<Value, String>> = tokio::spawn(async move {
                let response_result =
                    send_http_request(&client_clone, &uri_clone, broker_request).await;
                match response_result {
                    Ok(response) => {
                        if response.status() != StatusCode::OK {
                            return Err(format!(
                                "Request to {} failed with status: {}",
                                broker_request_clone.rule.alias,
                                response.status()
                            ));
                        }
                        let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
                        let body_string = String::from_utf8(body_bytes.to_vec()).unwrap();
                        let body_json: Value = serde_json::from_str(&body_string).unwrap();
                        Ok(body_json)
                    }
                    Err(e) => Err(format!(
                        "Request to {} failed: {}",
                        broker_request_clone.rule.alias, e
                    )),
                }
            });
            handles.push(handle);
        }

        let mut expected_results = vec![
            json!({"data": "response1"}),
            json!({"data": "response2"}),
            json!({"data": "response3"}),
        ];
        for handle in handles {
            let result = handle.await.unwrap();
            match result {
                Ok(body_json) => {
                    if let Some(index) = expected_results.iter().position(|x| *x == body_json) {
                        expected_results.remove(index);
                    } else {
                        panic!("Unexpected response: {:?}", body_json);
                    }
                }
                Err(e) => {
                    panic!("One of the concurrent requests failed: {}", e);
                }
            }
        }
        assert!(
            expected_results.is_empty(),
            "Not all expected responses were received"
        );
    }

    #[tokio::test]
    async fn test_send_http_request_invalid_uri() {
        let client = Client::new();
        let rule = Rule {
            alias: "invalid uri %".to_string(), // Invalid URI character
            ..Default::default()
        };

        let broker_request = BrokerRequest {
            rpc: RpcRequest::mock(),
            rule,
            subscription_processed: None,
            workflow_callback: None,
            telemetry_response_listeners: vec![],
        };

        let base_uri: Uri = "http://localhost:1234/".parse().unwrap();
        let result = send_http_request(&client, &base_uri, broker_request).await;
        assert!(matches!(result, Err(RippleError::BrokerError(_))));
    }

    #[tokio::test]
    async fn test_send_http_request_server_error() {
        let mock_server = MockServer::start();
        mock_server.mock(|when, then| {
            when.method(GET).path("/test_rule_error");
            then.status(500)
                .header("Content-Type", "application/json")
                .json_body(json!({"error": "server error"}));
        });

        let base_uri = mock_server.base_url().parse::<Uri>().unwrap();

        let rule = Rule {
            alias: "test_rule_error".to_string(),
            ..Default::default()
        };

        let broker_request = BrokerRequest {
            rpc: RpcRequest::mock(),
            rule,
            subscription_processed: None,
            workflow_callback: None,
            telemetry_response_listeners: vec![],
        };

        let client = Client::new();
        let response = send_http_request(&client, &base_uri, broker_request)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let body_json: Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(body_json, json!({"error": "server error"}));
    }

    #[tokio::test]
    async fn test_send_http_request_empty_params_json() {
        let mock_server = MockServer::start();
        mock_server.mock(|when, then| {
            when.method(GET).path("/test_rule_empty");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(json!({"ok": true}));
        });

        let base_uri = mock_server.base_url().parse::<Uri>().unwrap();

        let rule = Rule {
            alias: "test_rule_empty".to_string(),
            ..Default::default()
        };

        let mut rpc = RpcRequest::mock();
        rpc.params_json = "".to_string();

        let broker_request = BrokerRequest {
            rpc,
            rule,
            subscription_processed: None,
            workflow_callback: None,
            telemetry_response_listeners: vec![],
        };

        let client = Client::new();
        let response = send_http_request(&client, &base_uri, broker_request)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let body_json: Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(body_json, json!({"ok": true}));
    }
}
