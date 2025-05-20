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
    api::{gateway::rpc_gateway_api::JsonRpcApiError, observability::log_signal::LogSignal},
    log::{debug, error},
    tokio::{self, sync::mpsc},
    utils::error::RippleError,
};

use super::endpoint_broker::{
    BrokerCallback, BrokerCleaner, BrokerConnectRequest, BrokerOutputForwarder, BrokerRequest,
    BrokerSender, EndpointBroker, EndpointBrokerState, BROKER_CHANNEL_BUFFER_SIZE,
};
use crate::state::platform_state::PlatformState;
use tokio_tungstenite::tungstenite::http::uri::InvalidUri;

pub struct HttpBroker {
    sender: BrokerSender,
    cleaner: BrokerCleaner,
}
/*

*/

async fn send_http_request(
    client: &Client<HttpConnector>,
    method: Method,
    uri: &Uri,
    path: &str,
) -> Result<Response<Body>, RippleError> {
    /*
    TODO? we may need to support body for POST request in the future
    */
    let http_request = Request::new(Body::empty());
    let (mut parts, _) = http_request.into_parts();
    //TODO, need to refactor to support other methods
    parts.method = method.clone();
    /*
    mix endpoint url with method
    */

    let uri: Uri = format!("{}{}", uri, path)
        .parse()
        .map_err(|e: InvalidUri| RippleError::BrokerError(e.to_string()))?;
    let new_request = Request::builder()
        .uri(uri)
        .body(Body::empty())
        .map_err(|e| RippleError::BrokerError(e.to_string()))?;
    let (uri_parts, _) = new_request.into_parts();

    parts.uri = uri_parts.uri;

    let http_request = Request::from_parts(parts, Body::empty());

    debug!(
        "http_broker sending {} request={}",
        method,
        http_request.uri(),
    );
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
            while let Some(request) = tr.recv().await {
                LogSignal::new("http_broker".to_string(), format!("received request - start processing request={:?}", request), request.rpc.ctx.clone())
                    .with_diagnostic_context_item("rule_alias", request.rule.alias.as_str()).emit_debug();
                match send_http_request(&client, Method::GET, &uri, &request.clone().rule.alias)
                    .await
                {
                    Ok(response) => {
                        let (parts, body) = response.into_parts();
                        let body = body_to_bytes(body).await;
                        let mut request = request;
                        if let Ok(json_str) = serde_json::from_slice::<serde_json::Value>(&body).map(|v| vec![v])
                            .and_then(|v| serde_json::to_string(&v))
                        {
                            request.rpc.params_json = json_str;
                            let response = Self::update_request(&request);
                            LogSignal::new(
                                "http_broker".to_string(),
                                format!("received response={:?} to request: {:?} using rule={:?}", response, request, request.rule),
                                request.rpc.ctx.clone(),
                            )
                            .emit_debug();


                            let _ = send_broker_response(&callback, &request, &body).await;
                            if !parts.status.is_success() {
                                LogSignal::new("http_broker".to_string(), "Prepare request failed".to_string(), request.rpc.ctx.clone())
                                .with_diagnostic_context_item("error", &format!("http error {} returned from http service in http broker {:?}",
                                    parts.status, body))
                                .emit_error();
                            }
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
    use hyper::{Client, Method, StatusCode, Uri};
    use serde_json::{json, Value};
    use std::time::Duration;

    use crate::broker::{
        endpoint_broker::BrokerOutput,
        rules::rules_engine::{Rule, RuleEndpoint, RuleEndpointProtocol},
    };

    use super::*;

    use ripple_sdk::{
        api::gateway::rpc_gateway_api::{JsonRpcApiResponse, RpcRequest},
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

    async fn assert_http_request(
        client: &Client<HttpConnector>,
        method: Method,
        base_uri: &Uri,
        path: &str,
        expected_status: StatusCode,
        expected_body: Option<Value>,
    ) {
        let response_result = send_http_request(client, method, base_uri, path).await;

        match response_result {
            Ok(response) => {
                assert_eq!(response.status(), expected_status);
                if let Some(expected) = expected_body {
                    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
                    let body_string = String::from_utf8(body_bytes.to_vec()).unwrap();
                    let body_json: Value = serde_json::from_str(&body_string).unwrap();
                    assert_eq!(body_json, expected);
                }
            }
            Err(e) => {
                panic!("send_http_request failed: {}", e);
            }
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
    async fn test_send_http_request_success() {
        let base_uri = get_base_uri_from_mock_server();
        let client = Client::new();
        assert_http_request(
            &client,
            Method::GET,
            &base_uri,
            "test_rule",
            StatusCode::OK,
            Some(json!({"data": "success"})),
        )
        .await;
    }

    #[tokio::test]
    async fn test_send_http_request_fail_invalid_path() {
        let base_uri = get_base_uri_from_mock_server();
        let client = Client::new();
        assert_http_request(
            &client,
            Method::GET,
            &base_uri,
            "test",
            StatusCode::NOT_FOUND,
            None,
        )
        .await;
    }

    #[tokio::test]
    async fn test_send_http_request_fail_invalid_path_with_spaces() {
        let base_uri = get_base_uri_from_mock_server();
        let client = Client::new();

        let path = " "; //This would fail Request::builder() spaces not allowed in URL
        let response_result = send_http_request(&client, Method::GET, &base_uri, path).await;
        assert!(response_result.is_err());

        if let Err(RippleError::BrokerError(err_msg)) = response_result {
            assert!(
                err_msg.contains("invalid uri character"),
                "Error message should indicate invalid URI character"
            );
        } else {
            panic!(
                "Expected RippleError::BrokerError, got {:?}",
                response_result
            );
        }
    }

    #[tokio::test]
    async fn test_send_http_request_fail_invalid_uri() {
        let _ = get_base_uri_from_mock_server();
        let client = Client::new();

        let invalid_uri: Uri = "http://127.0.0.1:1234/".parse().unwrap();
        let path = "test_rule";

        let response_result = send_http_request(&client, Method::GET, &invalid_uri, path).await;

        if let Err(e) = response_result {
            assert!(e.to_string().contains("Connection refused"));
        } else {
            panic!("Expected an error, but got success");
        }
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

        let paths = vec!["test_rule1", "test_rule2", "test_rule3"];
        let mut handles = vec![];

        for path in paths {
            let client_clone = client.clone();
            let uri_clone = base_uri.clone();
            let path_clone = path.to_string();
            let handle: JoinHandle<Result<Value, String>> = tokio::spawn(async move {
                let response_result =
                    send_http_request(&client_clone, Method::GET, &uri_clone, &path_clone).await;
                match response_result {
                    Ok(response) => {
                        if response.status() != StatusCode::OK {
                            return Err(format!(
                                "Request to {} failed with status: {}",
                                path_clone,
                                response.status()
                            ));
                        }
                        let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
                        let body_string = String::from_utf8(body_bytes.to_vec()).unwrap();
                        let body_json: Value = serde_json::from_str(&body_string).unwrap();
                        Ok(body_json)
                    }
                    Err(e) => Err(format!("Request to {} failed: {}", path_clone, e)),
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
}
