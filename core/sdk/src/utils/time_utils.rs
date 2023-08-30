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

use chrono::{LocalResult, TimeZone, Utc};
use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use futures::Future;
use tokio::task::JoinHandle;

/*
Handle various types of timestamp conversion from magnitude -> String
for ISO8601 format, e.g: 2022-06-23T16:16:10Z

*/

pub fn timestamp_2_iso8601(timestamp: i64) -> String {
    if let LocalResult::Single(t) = chrono::Local.timestamp_opt(timestamp, 0) {
        t.to_rfc3339()
    } else {
        timestamp.to_string()
    }
}

/*
Handle various types of timestamp conversion from magnitude -> String
*/
const TIME_STAMP_CHECK: i64 = 100000000000;

pub fn convert_timestamp_to_iso8601(timestamp: i64) -> String {
    let timestamp_in_secs = if timestamp >= TIME_STAMP_CHECK {
        // the given time stamp is in milliseconds.
        timestamp / 1000
    } else {
        timestamp
    };

    Utc.timestamp_opt(timestamp_in_secs, 0)
        .unwrap()
        .to_rfc3339()
}

#[derive(Debug)]
pub struct Timer {
    handle: JoinHandle<()>,
    cancelled: Arc<Mutex<bool>>,
}

impl Timer {
    pub fn start<T>(delay_ms: u64, callback: T) -> Timer
    where
        T: Future + Send + 'static,
    {
        let cancelled_flag_mutex = Arc::new(Mutex::new(false));
        let cancelled = cancelled_flag_mutex.clone();
        let handle = tokio::spawn(async move {
            thread::sleep(Duration::from_millis(delay_ms));
            let cancelled = { *cancelled_flag_mutex.lock().unwrap() };
            if !cancelled {
                callback.await;
            }
        });
        Timer { handle, cancelled }
    }

    pub fn cancel(self) {
        let mut cancelled_flag = self.cancelled.lock().unwrap();
        *cancelled_flag = true;
        self.handle.abort();
    }
}
