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
    api::gateway::rpc_gateway_api::JsonRpcApiError,
    log::{debug, error, trace},
    tokio::{self, sync::mpsc},
    utils::error::RippleError,
};

use tokio_tungstenite::tungstenite::http::uri::InvalidUri;

use super::endpoint_broker::{
    BrokerCallback, BrokerCleaner, BrokerConnectRequest, BrokerOutputForwarder, BrokerRequest,
    BrokerSender, EndpointBroker, EndpointBrokerState,
};

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
async fn send_broker_response(callback: &BrokerCallback, request: &BrokerRequest, body: &[u8]) {
    match BrokerOutputForwarder::handle_non_jsonrpc_response(
        body,
        callback.clone(),
        request.clone(),
    ) {
        Ok(_) => {}
        Err(e) => {
            error!("Error message from http broker {:?}", e)
        }
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
        request: BrokerConnectRequest,
        callback: BrokerCallback,
        _broker_state: &mut EndpointBrokerState,
    ) -> Self {
        let endpoint = request.endpoint.clone();
        let (tx, mut tr) = mpsc::channel(10);
        let broker = BrokerSender { sender: tx };
        let client = Client::new();
        let _ =  endpoint.get_url().parse().map_err(|e| error!("broker url {:?} in endpoint is invalid, cannot start http broker. error={}",endpoint,e) ).map(|uri| tokio::spawn(async move {
            while let Some(request) = tr.recv().await {
                debug!("http broker received request={:?}", request);
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
                            trace!(
                                "http broker response={:?} to request: {:?} using rule={:?}",
                                response, request, request.rule
                            );

                            send_broker_response(&callback, &request, &body).await;
                            if !parts.status.is_success() {
                                error!(
                                    "http error {} returned from http service in http broker {:?}",
                                    parts.status, body
                                );
                            }
                        } else {
                            let msg = format!("Error in http broker parsing response from http service at {}. status={:?}",uri, parts.status);
                            error!("{}",msg);
                            Self::send_broker_failure_response(&callback,
                                JsonRpcApiError::default()
                                .with_id(request.rpc.ctx.call_id)
                                .with_message(msg.to_string()).into());
                        }
                    }
                    Err(err) => {
                        let msg = format!("An error message from calling the downstream http service={} in http broker {:?}", uri, err);
                        error!("{}",msg);
                        send_broker_response(&callback, &request,  error_string_to_json(msg.as_str()).to_string().as_bytes()).await;
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
