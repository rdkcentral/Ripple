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

pub mod api;
pub mod extn;
pub mod framework;
pub mod manifest;
pub mod processor;
pub mod service;
pub mod utils;

// Externalize the reusable crates to avoid version
// mismatch and standardization of these libraries
// across extensions
pub extern crate async_trait;
pub extern crate chrono;
pub extern crate futures;
pub extern crate libloading;
pub extern crate log;
pub extern crate semver;
pub extern crate serde;
pub extern crate serde_json;
pub extern crate serde_yaml;
pub extern crate tokio;
pub extern crate tokio_tungstenite;
pub extern crate uuid;

// Optional tracing support - reexport tracing crates when feature is enabled
#[cfg(feature = "tracing")]
pub extern crate opentelemetry;
#[cfg(feature = "tracing")]
pub extern crate opentelemetry_jaeger;
#[cfg(feature = "tracing")]
pub extern crate tracing;
#[cfg(feature = "tracing")]
pub extern crate tracing_opentelemetry;
#[cfg(feature = "tracing")]
pub extern crate tracing_subscriber;

// Macro for conditional tracing imports - use throughout the project
#[macro_export]
macro_rules! use_tracing {
    () => {
        #[cfg(feature = "tracing")]
        use $crate::tracing::{
            debug as tracing_debug, error as tracing_error, info as tracing_info, instrument,
        };
    };
}

// Optional tracing initialization function - available project-wide
#[cfg(feature = "tracing")]
pub fn init_tracing() {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    // Check if JAEGER_ENDPOINT is set for visualization
    let jaeger_endpoint = std::env::var("JAEGER_ENDPOINT").ok();
    println!(
        "🔍 DEBUG: Checking JAEGER_ENDPOINT environment variable: {:?}",
        jaeger_endpoint
    );

    // Debug: print all environment variables containing "JAEGER"
    for (key, value) in std::env::vars() {
        if key.contains("JAEGER") {
            println!("🔍 DEBUG: Found env var: {}={}", key, value);
        }
    }

    let registry = tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug")));

    if let Some(endpoint) = jaeger_endpoint {
        println!(
            "🔍 Jaeger endpoint detected: {}, initializing OpenTelemetry with HTTP collector",
            endpoint
        );

        // Try to initialize Jaeger tracing with HTTP collector
        match init_jaeger_tracing(&endpoint) {
            Ok(_) => {
                println!("✅ Jaeger tracing initialized! Traces will appear in Jaeger UI at http://localhost:16686");

                // Also add console layer for immediate feedback
                registry
                    .with(
                        tracing_subscriber::fmt::layer()
                            .with_target(true)
                            .with_thread_ids(true)
                            .compact(),
                    )
                    .with(tracing_opentelemetry::layer())
                    .init();
            }
            Err(e) => {
                println!(
                    "⚠️ Failed to initialize Jaeger: {}. Falling back to console output.",
                    e
                );
                registry
                    .with(
                        tracing_subscriber::fmt::layer()
                            .with_target(true)
                            .with_thread_ids(true)
                            .with_file(true)
                            .with_line_number(true)
                            .with_span_events(
                                tracing_subscriber::fmt::format::FmtSpan::ENTER
                                    | tracing_subscriber::fmt::format::FmtSpan::CLOSE,
                            )
                            .pretty(),
                    )
                    .init();
            }
        }

        // Create a test span to show it's working
        tracing::info_span!(
            "startup_demo",
            service = "ripple",
            demo = "manager_presentation"
        )
        .in_scope(|| {
            tracing::info!(
                "🚀 Manager demo: Tracing is active! All instrumented functions will be logged."
            );

            // Show what traces look like
            tracing::debug_span!("example_span", operation = "demo").in_scope(|| {
                tracing::info!("This is what a nested trace looks like");
            });
        });
    } else {
        // Initialize with console output only
        registry
            .with(
                tracing_subscriber::fmt::layer()
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_span_events(
                        tracing_subscriber::fmt::format::FmtSpan::ENTER
                            | tracing_subscriber::fmt::format::FmtSpan::CLOSE,
                    )
                    .pretty(),
            )
            .init();

        log::info!("Tracing initialized for console output with span events (set JAEGER_ENDPOINT for visualization)");

        // Create a test span to verify tracing is working
        tracing::info_span!("startup_demo", service = "ripple").in_scope(|| {
            tracing::info!("Console tracing initialized - spans will show in logs");
        });
    }
}

#[cfg(feature = "tracing")]
fn init_jaeger_tracing(endpoint: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use opentelemetry::sdk::Resource;
    use opentelemetry::KeyValue;

    // Create HTTP client for Jaeger
    let http_client = reqwest::Client::new();

    // Create Jaeger tracer with HTTP collector and explicit client
    let _tracer = opentelemetry_jaeger::new_collector_pipeline()
        .with_endpoint(format!("http://{}/api/traces", endpoint))
        .with_timeout(std::time::Duration::from_secs(3))
        .with_service_name("ripple")
        .with_http_client(http_client)
        .with_trace_config(
            opentelemetry::sdk::trace::config().with_resource(Resource::new(vec![
                KeyValue::new("service.name", "ripple"),
                KeyValue::new("service.version", "1.0.0"),
                KeyValue::new("demo.type", "manager_presentation"),
            ])),
        )
        .install_batch(opentelemetry::runtime::Tokio)?;

    // The tracer provider is automatically set globally by the install_batch call
    Ok(())
}

pub trait Mockable {
    fn mock() -> Self;
}

pub type JsonRpcErrorType = jsonrpsee::core::error::Error;
pub type JsonRpcErrorCode = jsonrpsee::types::error::ErrorCode;
