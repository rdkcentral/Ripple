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

use serde_json::json;

pub trait Mockable {
    fn mock() -> Self;
}

pub fn cap_jsonrpc_payload_granted(cap: String) -> serde_json::Value {
    json!({"id":1,"jsonrpc":"2.0","result":{"available":true,"capability":cap,"manage":{"granted":true,"permitted":true},"provide":{"granted":true,"permitted":true},"supported":true,"use":{"granted":true,"permitted":true}}})
}

pub fn cap_jsonrpc_payload_revoked(cap: String) -> serde_json::Value {
    json!({"id":1,"jsonrpc":"2.0","result":{"available":true,"capability":cap,"details":["unpermitted"],"manage":{"granted":false,"permitted":false},"provide":{"granted":false,"permitted":false},"supported":true,"use":{"granted":false,"permitted":false}}})
}
