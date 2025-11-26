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

use std::time::Duration;

use futures::stream::{SplitSink, SplitStream};
use futures_util::StreamExt;
use log::{error, info, warn};
use tokio::net::TcpStream;
use tokio_tungstenite::{client_async, tungstenite::Message, WebSocketStream};

use super::error::RippleError;

const DEFAULT_RETRY_INTERVAL: u64 = 50;

pub struct WebSocketConfig {
    pub alias: Option<String>,
    pub retry: Option<u64>,
    pub fail_after: Option<i32>,
}

pub struct WebSocketConfigBuilder {
    alias: Option<String>,
    retry: Option<u64>,
    fail_after: Option<i32>,
}

impl Default for WebSocketConfigBuilder {
    fn default() -> Self {
        Self {
            alias: None,
            retry: Some(DEFAULT_RETRY_INTERVAL),
            fail_after: None,
        }
    }
}

impl WebSocketConfigBuilder {
    pub fn alias(mut self, alias: String) -> Self {
        self.alias = Some(alias);
        self
    }

    pub fn retry(mut self, retry: u64) -> Self {
        self.retry = Some(retry);
        self
    }

    pub fn fail_after(mut self, fail_after: i32) -> Self {
        self.fail_after = Some(fail_after);
        self
    }

    pub fn build(self) -> WebSocketConfig {
        WebSocketConfig {
            alias: self.alias,
            retry: self.retry,
            fail_after: self.fail_after,
        }
    }
}

pub struct WebSocketUtils;

impl WebSocketUtils {
    /// Attempts to establish a WebSocket connection to the given endpoint.
    ///
    /// # Parameters
    /// - `endpoint`: The base URL of the WebSocket server.
    /// - `config`: Configuration options for the WebSocket connection, including alias, retry interval, and fail-after duration.
    ///
    /// # Returns
    /// - On success, returns a tuple containing the WebSocket sink and stream.
    /// - On failure, returns a `RippleError`.
    ///
    /// # Errors
    /// - `RippleError::InvalidInput` if the URL is invalid.
    /// - `RippleError::NotAvailable` if the connection fails after the specified retries.
    ///
    /// # Example
    /// ```
    /// #[tokio::main]
    /// async fn main() {
    ///     let config = ripple_sdk::utils::ws_utils::WebSocketConfigBuilder::default()
    ///         .alias("my_alias".to_string())
    ///         .retry(100)
    ///         .fail_after(5)
    ///         .build();
    ///     let result = ripple_sdk::utils::ws_utils::WebSocketUtils::get_ws_stream("ws://127.0.0.1:0", Some(config)).await;
    /// }
    /// ```
    pub async fn get_ws_stream(
        endpoint: &str,
        inital_config: Option<WebSocketConfig>,
    ) -> Result<
        (
            SplitSink<WebSocketStream<TcpStream>, Message>,
            SplitStream<WebSocketStream<TcpStream>>,
        ),
        RippleError,
    > {
        info!("Broker Endpoint url {}", endpoint);
        let config = inital_config.unwrap_or_else(|| {
            WebSocketConfigBuilder::default()
                .retry(DEFAULT_RETRY_INTERVAL)
                .build()
        });
        let retry_every = config.retry.unwrap_or(DEFAULT_RETRY_INTERVAL);
        let url_path = if let Some(ref a) = config.alias {
            format!("{}{}", endpoint, a)
        } else {
            endpoint.to_owned()
        };
        if cfg!(not(feature = "local_dev")) {
            // Only support local ws connections
            if !url_path.starts_with("ws://127.0.0.1") && !url_path.starts_with("ws://localhost") {
                return Err(RippleError::InvalidInput);
            }
        }
        let url = match url::Url::parse(&url_path) {
            Ok(parsed_url) => parsed_url,
            Err(_) => return Err(RippleError::InvalidInput),
        };
        let tcp_port = Self::extract_tcp_port(endpoint)?;

        info!("Url host str {}", url.host_str().unwrap());

        let timeout_duration = config.fail_after.map(|f| Duration::from_secs(f as u64));
        if let Some(duration) = timeout_duration {
            tokio::time::timeout(duration, async {
                Self::handshake(config, retry_every, url_path, tcp_port).await
            })
            .await
            .map_err(|_| RippleError::NotAvailable)?
        } else {
            Self::handshake(config, retry_every, url_path, tcp_port).await
        }
    }

    async fn connect_tcp_port(
        tcp_port: &str,
        url_path: &str,
    ) -> Result<
        (
            SplitSink<WebSocketStream<TcpStream>, Message>,
            SplitStream<WebSocketStream<TcpStream>>,
        ),
        RippleError,
    > {
        match TcpStream::connect(&tcp_port).await {
            Ok(v) => {
                // Setup handshake for websocket with the tcp port
                // Some WS servers lock on to the Port but not setup handshake till they are fully setup
                if let Ok((stream, _)) = client_async(url_path, v).await {
                    return Ok(stream.split());
                }
            }
            Err(e) => {
                if !e.to_string().to_lowercase().contains("connection refused") {
                    error!("Failed to connect to TCP port {}: {}", tcp_port, e);
                }
            }
        }
        Err(RippleError::NotAvailable)
    }

    fn extract_tcp_port(url: &str) -> Result<String, RippleError> {
        let parsed_url = url::Url::parse(url).map_err(|_| RippleError::InvalidInput)?;
        if let Some(host) = parsed_url.host_str() {
            let port = parsed_url.port_or_known_default().unwrap_or(80);
            let host = format!("{}:{}", host, port);
            Ok(host.to_string())
        } else {
            Err(RippleError::InvalidInput)
        }
    }

    async fn handshake(
        config: WebSocketConfig,
        retry_every: u64,
        url_path: String,
        tcp_port: String,
    ) -> Result<
        (
            SplitSink<WebSocketStream<TcpStream>, Message>,
            SplitStream<WebSocketStream<TcpStream>>,
        ),
        RippleError,
    > {
        let mut index: i32 = 0;
        let mut retry_count: u32 = 0;
        let mut delay_duration = tokio::time::Duration::from_millis(retry_every);

        loop {
            match Self::connect_tcp_port(&tcp_port, &url_path).await {
                Ok(v) => {
                    if retry_count > 0 {
                        info!(
                            "Websocket TCP Connection with {} succeeded after {} retries",
                            url_path, retry_count
                        );
                    } else {
                        info!("Websocket TCP Connection with {} succeeded", url_path);
                    }
                    break Ok(v);
                }
                Err(e) => {
                    if let RippleError::Permission(
                        crate::api::firebolt::fb_capabilities::DenyReason::Unpermitted,
                    ) = e
                    {
                        break Err(RippleError::Permission(
                            crate::api::firebolt::fb_capabilities::DenyReason::Unpermitted,
                        ));
                    }
                }
            }

            if let Some(fail) = &config.fail_after {
                if fail.eq(&index) {
                    warn!(
                        "Websocket TCP Connection with {} failed after {} retries",
                        url_path, retry_count
                    );
                    break Err(RippleError::NotAvailable);
                }
            }

            index += 1;
            retry_count += 1;

            warn!(
                "Websocket TCP Connection with {} failed (retry #{}) - retrying after {} ms on {}",
                url_path,
                retry_count,
                delay_duration.as_millis(),
                tcp_port
            );

            if delay_duration < tokio::time::Duration::from_secs(3) {
                delay_duration *= 2;
            }
            tokio::time::sleep(delay_duration).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_tcp_port_valid_url() {
        let url = "ws://example.com:8080";
        assert_eq!(
            WebSocketUtils::extract_tcp_port(url).unwrap(),
            "example.com:8080"
        );
    }

    #[test]
    fn test_extract_tcp_port_invalid_url() {
        let url = "invalid_url";
        assert!(WebSocketUtils::extract_tcp_port(url).is_err());
    }

    #[tokio::test]
    async fn test_get_ws_stream_invalid_url() {
        let config = WebSocketConfig {
            alias: None,
            retry: Some(100),
            fail_after: Some(5),
        };
        let result = WebSocketUtils::get_ws_stream("invalid_url", Some(config)).await;
        assert!(matches!(result, Err(RippleError::InvalidInput)));
    }

    #[tokio::test]
    async fn test_get_ws_stream_with_retry() {
        let config = WebSocketConfigBuilder::default()
            .retry(50)
            .fail_after(3)
            .build();
        let result = WebSocketUtils::get_ws_stream("ws://127.0.0.1:0", Some(config)).await;
        assert!(matches!(result, Err(RippleError::NotAvailable)));
    }
}
