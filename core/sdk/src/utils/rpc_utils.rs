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

use jsonrpsee::core::Error;

pub fn rpc_err(msg: impl Into<String>) -> Error {
    Error::Custom(msg.into())
}

pub fn extract_tcp_port(url: &str) -> Result<String, Error> {
    let url_split: Vec<&str> = url.split("://").collect();
    if let Some(domain) = url_split.get(1) {
        let domain_split: Vec<&str> = domain.split('/').collect();
        if let Some(first_part) = domain_split.first() {
            Ok(first_part.to_string())
        } else {
            Err(Error::Custom("Invalid domain format".to_string()))
        }
    } else {
        Err(Error::Custom("Invalid URL format".to_string()))
    }
}
