// Copyright 2025 Comcast Cable Communications Management, LLC
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
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use std::sync::atomic::{AtomicU64, Ordering};

static REQ_ID_COUNTER: AtomicU64 = AtomicU64::new(5000);
const APPGW_WS_URL: &str = "ws://127.0.0.1:1234";

pub async fn start_test_cli() {
    let (ws_stream, _) = connect_async(APPGW_WS_URL)
        .await
        .expect("Failed to connect to AppGW");
    let (mut write, mut read) = ws_stream.split();

    // Spawn receiver task
    tokio::spawn(async move {
        while let Some(Ok(Message::Text(resp))) = read.next().await {
            if let Ok(v) = serde_json::from_str::<Value>(&resp) {
                let id = v["id"].as_u64().unwrap_or(0);
                if let Some(result) = v.get("result") {
                    if let Some(service) = result.get("from_service") {
                        println!(
                            "[Success][{}] From {}: {}",
                            id,
                            service,
                            result.get("response").unwrap_or(result)
                        );
                    } else {
                        println!(
                            "[Success][{}]: {}",
                            id,
                            serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string())
                        );
                    }
                } else if let Some(_error) = v.get("error") {
                    println!(
                        "[Error][{}]: {}",
                        id,
                        serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string())
                    );
                } else {
                    println!("[Unknown][{}]: {}", id, resp);
                }
            } else {
                println!("[Test CLI] Non-JSON response: {}", resp);
            }
        }
    });

    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut line = String::new();

    loop {
        println!();
        println!("=== Test CLI Menu ===");
        println!("1. Call service1.get_status");
        println!("2. Call service2.compute");
        println!("3. Send aggregate.service_status");
        println!("4. Send invalid method");
        println!("5. Send request to unregistered service");
        println!("6. Call service2.check");
        println!("7. Call service1.info");
        println!("q. Quit");
        print!("Choose an option: ");
        io::stdout().flush().unwrap();

        line.clear();
        if reader.read_line(&mut line).unwrap() == 0 {
            break;
        }

        let id = REQ_ID_COUNTER.fetch_add(1, Ordering::Relaxed);

        let choice = line.trim();

        let msg = match choice {
            "1" => json!({ "jsonrpc": "2.0", "id": id, "method": "service1.get_status" }),
            "2" => json!({ "jsonrpc": "2.0", "id": id, "method": "service2.compute" }),
            "3" => json!({ "jsonrpc": "2.0", "id": id, "method": "aggregate.service_status" }),
            "4" => json!({ "jsonrpc": "2.0", "id": id, "method": "unknown.method" }),
            "5" => json!({ "jsonrpc": "2.0", "id": id, "method": "service1.good_bye" }),
            "6" => json!({ "jsonrpc": "2.0", "id": id, "method": "service2.check" }),
            "7" => json!({ "jsonrpc": "2.0", "id": id, "method": "service1.info" }),
            "q" | "Q" => break,
            _ => {
                println!("Invalid choice");
                continue;
            }
        };

        if let Err(e) = write.send(Message::Text(msg.to_string())).await {
            eprintln!("Failed to send message: {}", e);
            break;
        }
        println!("Sent request [{}] for option '{}'", id, choice);
    }

    println!("Test CLI exiting...");
}
