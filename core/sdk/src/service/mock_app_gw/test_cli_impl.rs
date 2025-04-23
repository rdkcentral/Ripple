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
use std::io::{self, Write};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use std::sync::atomic::{AtomicU64, Ordering};

static REQ_ID_COUNTER: AtomicU64 = AtomicU64::new(5000);

pub async fn start_tester() {
    let (ws_stream, _) = connect_async("ws://127.0.0.1:1234")
        .await
        .expect("Failed to connect to AppGW");
    let (mut write, mut read) = ws_stream.split();

    //let (_tx, _rx) = mpsc::channel::<(String, String)>(32);
    //let pending: HashMap<String, String> = HashMap::new();

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
                        println!("[Success][{}]: {}", id, result);
                    }
                } else if let Some(error) = v.get("error") {
                    println!("[Error][{}]: {}", id, error);
                } else {
                    println!("[Unknown][{}]: {}", id, resp);
                }
            } else {
                println!("[Test CLI] Non-JSON response: {}", resp);
            }
        }
    });

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();

    loop {
        println!();
        println!("=== Test CLI Menu ===");
        println!("1. Call service1.get_status");
        println!("2. Call service2.compute");
        println!("3. Send aggregate.device_status");
        println!("4. Send invalid method");
        println!("5. Send request to unregistered service");
        println!("q. Quit");
        print!("Choose an option: ");
        io::stdout().flush().unwrap();

        line.clear();
        if reader.read_line(&mut line).await.unwrap() == 0 {
            break;
        }

        let id = REQ_ID_COUNTER.fetch_add(1, Ordering::Relaxed);

        let choice = line.trim();

        let msg = match choice {
            "1" => json!({ "jsonrpc": "2.0", "id": id, "method": "service1.get_status" }),
            "2" => json!({ "jsonrpc": "2.0", "id": id, "method": "service2.compute" }),
            "3" => json!({ "jsonrpc": "2.0", "id": id, "method": "aggregate.device_status" }),
            "4" => json!({ "jsonrpc": "2.0", "id": id, "method": "unknown.method" }),
            "5" => json!({ "jsonrpc": "2.0", "id": id, "method": "nonexistent.do_something" }),
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

    println!("Tester exiting...");
}
