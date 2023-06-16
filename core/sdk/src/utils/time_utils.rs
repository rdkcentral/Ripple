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

use chrono::{DateTime, NaiveDateTime, Utc};

/*
Handle various types of timestamp conversion from magnitude -> String
for ISO8601 format, e.g: 2022-06-23T16:16:10Z

*/

pub fn timestamp_2_iso8601(timestamp: i64) -> String {
    if let Some(t) = NaiveDateTime::from_timestamp_opt(timestamp, 0) {
        DateTime::<Utc>::from_utc(t, Utc).to_rfc3339().to_string()
    } else {
        timestamp.to_string()
    }
}
