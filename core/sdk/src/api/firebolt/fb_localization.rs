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

use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Default, Serialize, Clone)]
pub struct PreferredLanguage(String);

impl<'de> Deserialize<'de> for PreferredLanguage {
    fn deserialize<D>(deserializer: D) -> Result<PreferredLanguage, D::Error>
    where
        D: Deserializer<'de>,
    {
        let text = String::deserialize(deserializer)?;
        if Regex::new("^[a-z]{3}$").unwrap().is_match(&text) {
            Ok(PreferredLanguage(text))
        } else {
            Err(serde::de::Error::custom("Invalid value for Language"))
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct SetPreferredAudioLanguage {
    pub value: Vec<PreferredLanguage>,
}

impl SetPreferredAudioLanguage {
    pub fn get_string(&self) -> Vec<String> {
        self.value.iter().map(|x| x.0.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_set_and_get() {
        let bad_language = json!({"value": ["English"]});
        assert!(serde_json::from_value::<SetPreferredAudioLanguage>(bad_language).is_err());
        let good_language = json!({"value": ["eng"]});
        if let Ok(l) = serde_json::from_value::<SetPreferredAudioLanguage>(good_language) {
            assert!(String::from("eng").eq(l.value.get(0).unwrap().0.as_str()));
        } else {
            panic!("bad language entry should not serialize")
        }
    }
}
