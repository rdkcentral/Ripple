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
enum Patterns {
    Language,
    Timezone,
}

fn pattern_matches(pattern: Patterns, str: &String) -> bool {
    Regex::new(pattern.as_str()).unwrap().is_match(str.as_str())
}

impl Patterns {
    fn as_str(&self) -> &'static str {
        match self {
            Patterns::Language => "^[A-Za-z]{2}$",
            Patterns::Timezone => "^[-+_/ A-Za-z 0-9]*$",
        }
    }
}

pub mod opacity_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S>(value: &u32, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if *value > 100 {
            Err(serde::ser::Error::custom(
                "Invalid value for Opacity. Value should be between 0 and 100 inclusive",
            ))
        } else {
            serializer.serialize_u32(*value)
        }
    }
    pub fn deserialize<'de, D>(deserializer: D) -> Result<u32, D::Error>
    where
        D: Deserializer<'de>,
    {
        let num = u32::deserialize(deserializer)?;
        if num > 100 {
            Err(serde::de::Error::custom(
                "Invalid value for Opacity. Value should be between 0 and 100 inclusive",
            ))
        } else {
            Ok(num)
        }
    }
}

pub mod language_code_serde {
    use crate::utils::serde_utils::{pattern_matches, Patterns};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(str: &String, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if pattern_matches(Patterns::Language, str) {
            serializer.serialize_str(&str)
        } else {
            Err(serde::ser::Error::custom(
                "Language code is not of the format specified in ISO 639",
            ))
        }
    }
    pub fn deserialize<'de, D>(deserializer: D) -> Result<String, D::Error>
    where
        D: Deserializer<'de>,
    {
        let str = String::deserialize(deserializer)?;
        if pattern_matches(Patterns::Language, &str) {
            Ok(str)
        } else {
            Err(serde::de::Error::custom(
                "Language code is not of the format specified in ISO 639",
            ))
        }
    }
}

pub mod timezone_serde {
    use crate::utils::serde_utils::{pattern_matches, Patterns};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(str: &String, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if pattern_matches(Patterns::Timezone, str) {
            serializer.serialize_str(&str)
        } else {
            Err(serde::ser::Error::custom(
                "Timezone is not in a format supported by the IANA TZ database",
            ))
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<String, D::Error>
    where
        D: Deserializer<'de>,
    {
        let str = String::deserialize(deserializer)?;
        if pattern_matches(Patterns::Timezone, &str) {
            Ok(str)
        } else {
            Err(serde::de::Error::custom(
                "Timezone is not in a format supported by the IANA TZ database",
            ))
        }
    }
}
