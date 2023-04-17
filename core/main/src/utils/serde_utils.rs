use crate::api::handlers::discovery::{
    BADGER_ENTITLEMENTS_ACCOUNTLINK, BADGER_ENTITLEMENTS_UPDATE, BADGER_ENTITLEMENT_APPLAUNCH,
    BADGER_ENTITLEMENT_SIGNIN, BADGER_ENTITLEMENT_SIGNOUT, BADGER_MEDIA_EVENT_PERCENT,
    BADGER_MEDIA_EVENT_SECONDS,
};
use regex::Regex;
use serde::{Deserialize, Deserializer};
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
    use crate::helpers::serde_utils::{pattern_matches, Patterns};
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

pub mod optional_language_code_serde {
    use crate::helpers::serde_utils::language_code_serde;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(data: &Option<String>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if let Some(str) = data {
            language_code_serde::serialize(str, serializer)
        } else {
            serializer.serialize_none()
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        language_code_serde::deserialize(deserializer).map(|res| Some(res))
    }
}

pub mod optional_language_code_list_serde {
    use super::{pattern_matches, Patterns};
    use serde::{ser::SerializeSeq, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(data: &Option<Vec<String>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if let Some(list_of_lang) = data {
            let mut seq = serializer.serialize_seq(Some(list_of_lang.len()))?;
            for str in list_of_lang {
                seq.serialize_element(str)?;
            }
            seq.end()
        } else {
            serializer.serialize_none()
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: Vec<String> = Vec::deserialize(deserializer)?;
        for elem in &s {
            if !pattern_matches(Patterns::Language, elem) {
                return Err(serde::de::Error::custom(
                    "One or more language is not of the ISO 639 format",
                ));
            }
        }
        Ok(Some(s))
    }
}

pub mod date_time_str_serde {
    use chrono::{TimeZone, Utc};
    use serde::{self, Deserialize, Deserializer, Serializer};

    const FORMAT: &'static str = "%Y-%m-%dT%H:%M:%S.%3fZ";

    pub fn serialize<S>(data: &String, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let formed_date_res = Utc.datetime_from_str(&data, FORMAT);
        if let Ok(_) = formed_date_res {
            serializer.serialize_str(&data)
        } else {
            Err(serde::ser::Error::custom(
                "String not convertible to date-time",
            ))
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<String, D::Error>
    where
        D: Deserializer<'de>,
    {
        let str = String::deserialize(deserializer)?;
        let formed_date_res = Utc.datetime_from_str(&str, FORMAT);
        if let Ok(_) = formed_date_res {
            Ok(str)
        } else {
            Err(serde::de::Error::custom(
                "Field not in expected Date-Time (YYYY-MM-DDTHH:mm:SS.xxxZ) format",
            ))
        }
    }
}

pub mod optional_date_time_str_serde {
    use crate::helpers::serde_utils::date_time_str_serde;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(date: &Option<String>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if let Some(data) = date {
            date_time_str_serde::serialize(data, serializer)
        } else {
            serializer.serialize_none()
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        date_time_str_serde::deserialize(deserializer).map(|data| Some(data))
    }
}

pub mod timezone_serde {
    use crate::helpers::serde_utils::{pattern_matches, Patterns};
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

pub mod settings_serde {
    use serde::{Deserialize, Deserializer, Serializer};

    use crate::api::handlers::settings::SettingKey;

    pub fn serialize<S>(data: &Vec<String>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_seq(data.iter().map(|s| s.to_owned()))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let keys: Vec<String> = Vec::deserialize(deserializer)?;
        for key in &keys {
            if SettingKey::from_key(key).is_none() {
                return Err(serde::de::Error::custom(format!("Invalid key {}", key)));
            }
        }
        Ok(keys)
    }
}

enum DeserializeEnum {
    Action,
    Type,
    ProgressUnit,
}

fn create_valid_list(enum_value: DeserializeEnum) -> Vec<&'static str> {
    let mut valid_entry = Vec::new();
    match enum_value {
        DeserializeEnum::Action => {
            valid_entry.push(BADGER_ENTITLEMENT_SIGNIN);
            valid_entry.push(BADGER_ENTITLEMENT_SIGNOUT);
            valid_entry.push(BADGER_ENTITLEMENT_APPLAUNCH);
        }
        DeserializeEnum::Type => {
            valid_entry.push(BADGER_ENTITLEMENTS_UPDATE);
            valid_entry.push(BADGER_ENTITLEMENTS_ACCOUNTLINK);
        }
        DeserializeEnum::ProgressUnit => {
            valid_entry.push(BADGER_MEDIA_EVENT_PERCENT);
            valid_entry.push(BADGER_MEDIA_EVENT_SECONDS);
        }
    }
    valid_entry
}

fn deserialize_option<'de, D>(
    deserializer: D,
    enum_value: DeserializeEnum,
) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    let valid_entries = create_valid_list(enum_value);
    if valid_entries.contains(&value.as_str()) {
        Ok(Some(value))
    } else {
        Err(serde::de::Error::custom(format!(
            "Invalid value ({})",
            value
        )))
    }
}

pub fn link_action_deserialize<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_option(deserializer, DeserializeEnum::Action)
}

pub fn link_type_deserialize<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_option(deserializer, DeserializeEnum::Type)
}

pub fn progress_unit_deserialize<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_option(deserializer, DeserializeEnum::ProgressUnit)
}

pub fn progress_value_deserialize<'de, D>(deserializer: D) -> Result<f32, D::Error>
where
    D: Deserializer<'de>,
{
    let value: f32 = f32::deserialize(deserializer)?;
    if value < 0.0 {
        Err(serde::de::Error::custom(
            "Invalid value for progress. Minimum value should be 0.0",
        ))
    } else {
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::*;
    #[tokio::test]
    async fn test_language_code_serde_unit_struct() {
        #[derive(Serialize, Deserialize, Debug)]
        struct TestStruct {
            #[serde(with = "language_code_serde")]
            pub val: String,
        }
        let json_str = r#"{}"#;
        let test_struct = serde_json::from_str::<TestStruct>(&json_str);
        assert!(test_struct.is_err());
    }
    #[tokio::test]
    async fn test_language_code_serde_with_wrong_format() {
        #[derive(Serialize, Deserialize, Debug)]
        struct TestStruct {
            #[serde(with = "language_code_serde")]
            pub val: String,
        }
        let json_str = r#"{"val": "eng"}"#;
        let test_struct = serde_json::from_str::<TestStruct>(&json_str);
        assert!(test_struct.is_err());
    }
    #[tokio::test]
    async fn test_language_code_serde_with_proper_format() {
        #[derive(Serialize, Deserialize, Debug)]
        struct TestStruct {
            #[serde(with = "language_code_serde")]
            pub val: String,
        }
        let json_str = r#"{"val": "en"}"#;
        let test_struct = serde_json::from_str::<TestStruct>(&json_str);
        assert!(test_struct.is_ok());
        assert!(serde_json::to_string(&(test_struct.unwrap())).is_ok());
    }

    #[tokio::test]
    async fn test_optional_language_code_serde_unit_struct() {
        #[derive(Serialize, Deserialize, Debug)]
        struct TestStruct {
            #[serde(default, with = "optional_language_code_serde")]
            pub val: Option<String>,
        }
        let json_str = r#"{}"#;
        let test_struct = serde_json::from_str::<TestStruct>(&json_str);
        assert!(test_struct.is_ok());
        assert!(serde_json::to_string(&(test_struct.unwrap())).is_ok());
    }
    #[tokio::test]
    async fn test_optional_language_code_serde_with_wrong_format() {
        #[derive(Serialize, Deserialize, Debug)]
        struct TestStruct {
            #[serde(with = "optional_language_code_serde")]
            pub val: Option<String>,
        }
        let json_str = r#"{"val": "eng"}"#;
        let test_struct = serde_json::from_str::<TestStruct>(&json_str);
        assert!(test_struct.is_err());
    }
    #[tokio::test]
    async fn test_optional_language_code_serde_with_proper_format() {
        #[derive(Serialize, Deserialize, Debug)]
        struct TestStruct {
            #[serde(with = "optional_language_code_serde")]
            pub val: Option<String>,
        }
        let json_str = r#"{"val": "en"}"#;
        let test_struct = serde_json::from_str::<TestStruct>(&json_str);
        assert!(test_struct.is_ok());
        assert!(serde_json::to_string(&(test_struct.unwrap())).is_ok());
    }

    #[tokio::test]
    async fn test_optional_language_code_list_serde_unit_struct() {
        #[derive(Serialize, Deserialize, Debug)]
        struct TestStruct {
            #[serde(default, with = "optional_language_code_list_serde")]
            pub val: Option<Vec<String>>,
        }
        let json_str = r#"{}"#;
        let test_struct = serde_json::from_str::<TestStruct>(&json_str);
        assert!(test_struct.is_ok());
        assert!(serde_json::to_string(&(test_struct.unwrap())).is_ok());
    }
    #[tokio::test]
    async fn test_optional_language_code_list_serde_with_wrong_format() {
        #[derive(Serialize, Deserialize, Debug)]
        struct TestStruct {
            #[serde(default, with = "optional_language_code_list_serde")]
            pub val: Option<Vec<String>>,
        }
        let json_str = r#"{"val": ["eng", "fra"]}"#;
        let test_struct = serde_json::from_str::<TestStruct>(&json_str);
        assert!(test_struct.is_err());
    }
    #[tokio::test]
    async fn test_optional_language_code_list_serde_with_proper_format() {
        #[derive(Serialize, Deserialize, Debug)]
        struct TestStruct {
            #[serde(with = "optional_language_code_list_serde")]
            pub val: Option<Vec<String>>,
        }
        let json_str = r#"{"val": ["en", "fr"]}"#;
        let test_struct = serde_json::from_str::<TestStruct>(&json_str);
        assert!(test_struct.is_ok());
        assert!(serde_json::to_string(&(test_struct.unwrap())).is_ok());
    }

    #[tokio::test]
    async fn test_date_time_str_serde_unit_struct() {
        #[derive(Serialize, Deserialize, Debug)]
        struct TestStruct {
            #[serde(with = "date_time_str_serde")]
            pub val: String,
        }
        let json_str = r#"{}"#;
        let test_struct = serde_json::from_str::<TestStruct>(&json_str);
        assert!(test_struct.is_err());
    }
    #[tokio::test]
    async fn test_date_time_str_serde_with_wrong_format() {
        #[derive(Serialize, Deserialize, Debug)]
        struct TestStruct {
            #[serde(with = "date_time_str_serde")]
            pub val: String,
        }
        let json_str = r#"{"val": "eng"}"#;
        let test_struct = serde_json::from_str::<TestStruct>(&json_str);
        assert!(test_struct.is_err());
    }
    #[tokio::test]
    async fn test_date_time_str_serde_with_proper_format() {
        #[derive(Serialize, Deserialize, Debug)]
        struct TestStruct {
            #[serde(with = "date_time_str_serde")]
            pub val: String,
        }
        let json_str = r#"{"val": "2022-01-14T11:18:26.849Z"}"#;
        let test_struct = serde_json::from_str::<TestStruct>(&json_str);
        assert!(test_struct.is_ok());
        assert!(serde_json::to_string(&(test_struct.unwrap())).is_ok());
    }

    #[tokio::test]
    async fn test_optional_date_time_str_serde_unit_struct() {
        #[derive(Serialize, Deserialize, Debug)]
        struct TestStruct {
            #[serde(default, with = "optional_date_time_str_serde")]
            pub val: Option<String>,
        }
        let json_str = r#"{}"#;
        let test_struct = serde_json::from_str::<TestStruct>(&json_str);
        assert!(test_struct.is_ok());
        assert!(serde_json::to_string(&(test_struct.unwrap())).is_ok());
    }
    #[tokio::test]
    async fn test_optional_date_time_str_serde_with_wrong_format() {
        #[derive(Serialize, Deserialize, Debug)]
        struct TestStruct {
            #[serde(with = "optional_date_time_str_serde")]
            pub val: Option<String>,
        }
        let json_str = r#"{"val": "eng"}"#;
        let test_struct = serde_json::from_str::<TestStruct>(&json_str);
        assert!(test_struct.is_err());
    }
    #[tokio::test]
    async fn test_optional_date_time_str_serde_with_proper_format() {
        #[derive(Serialize, Deserialize, Debug)]
        struct TestStruct {
            #[serde(with = "optional_date_time_str_serde")]
            pub val: Option<String>,
        }
        let json_str = r#"{"val": "2022-01-14T11:18:26.849Z"}"#;
        let test_struct = serde_json::from_str::<TestStruct>(&json_str);
        assert!(test_struct.is_ok());
        assert!(serde_json::to_string(&(test_struct.unwrap())).is_ok());
    }

    #[tokio::test]
    async fn test_opacity_data_within_bounds() {
        #[derive(Serialize, Deserialize, Debug)]
        struct TestStruct {
            #[serde(with = "opacity_serde")]
            pub val: u32,
        }
        let json_str = r#"{"val": 23}"#;
        let test_struct = serde_json::from_str::<TestStruct>(&json_str);
        assert!(test_struct.is_ok());
        assert!(serde_json::to_string(&(test_struct.unwrap())).is_ok());
    }
    #[tokio::test]
    async fn test_opacity_data_out_of_bounds() {
        #[derive(Serialize, Deserialize, Debug)]
        struct TestStruct {
            #[serde(with = "opacity_serde")]
            pub val: u32,
        }
        let json_str = r#"{"val": 123}"#;
        let test_struct = serde_json::from_str::<TestStruct>(&json_str);
        assert!(test_struct.is_err());
    }

    #[test]
    fn test_pattern_match_success() {
        let valid_str = String::from("en");
        assert!(pattern_matches(Patterns::Language, &valid_str))
    }

    #[test]
    fn test_pattern_match_fail() {
        let valid_str = String::from("3ng");
        assert!(!pattern_matches(Patterns::Language, &valid_str))
    }

    #[test]
    fn test_settings_key_valid() {
        #[derive(Serialize, Deserialize, Debug)]
        struct TestStruct {
            #[serde(with = "settings_serde")]
            pub keys: Vec<String>,
        }
        let json_str = r#"{"keys": ["CC_STATE", "VOICE_GUIDANCE_STATE", 
        "DisplayPersonalizedRecommendations", "RememberWatchedPrograms", "friendly_name",
        "legacyMiniGuide", "power_save_status", "ShareWatchHistoryStatus", "ShowClosedCapture",
        "TextToSpeechEnabled2"]}"#;
        let test_struct = serde_json::from_str::<TestStruct>(&json_str);
        assert!(test_struct.is_ok());
        assert!(serde_json::to_string(&(test_struct.unwrap())).is_ok());
    }

    fn test_settings_key_invalid() {
        #[derive(Serialize, Deserialize, Debug)]
        struct TestStruct {
            #[serde(with = "settings_serde")]
            pub keys: Vec<String>,
        }
        let json_str = r#"{"keys": ["CC_STATE", "VOICE_GUIDANCE_STATE", 
        "DisplayPersonalizedRecommendations", "UnRememberWatchedPrograms"]}"#;
        let test_struct = serde_json::from_str::<TestStruct>(&json_str);
        assert!(test_struct.is_err());
    }
    #[test]
    fn test_progress_unit_valid_value() {
        #[derive(Serialize, Deserialize, Debug)]
        struct TestStruct {
            #[serde(
                default,
                skip_serializing_if = "Option::is_none",
                deserialize_with = "progress_unit_deserialize"
            )]
            pub progress_units: Option<String>,
        }
        let json_str = r#"{"progress_units": "seconds"}"#;
        let test_struct = serde_json::from_str::<TestStruct>(&json_str);
        assert!(test_struct.is_ok());
    }

    #[test]
    fn test_progress_unit_invalid_value() {
        #[derive(Serialize, Deserialize, Debug)]
        struct TestStruct {
            #[serde(
                default,
                skip_serializing_if = "Option::is_none",
                deserialize_with = "progress_unit_deserialize"
            )]
            pub progress_units: Option<String>,
        }
        let json_str = r#"{"progress_units": "hour"}"#;
        let test_struct = serde_json::from_str::<TestStruct>(&json_str);
        assert!(test_struct.is_err());
    }
    #[test]
    fn test_link_action_valid_value() {
        #[derive(Serialize, Deserialize, Debug)]
        struct TestStruct {
            #[serde(
                default,
                skip_serializing_if = "Option::is_none",
                deserialize_with = "link_action_deserialize"
            )]
            pub action: Option<String>, /*signIn, signOut, appLaunch,  */
        }
        let json_str = r#"{"action": "appLaunch"}"#;
        let test_struct = serde_json::from_str::<TestStruct>(&json_str);
        assert!(test_struct.is_ok());
    }

    #[test]
    fn test_link_action_invalid_value() {
        #[derive(Serialize, Deserialize, Debug)]
        struct TestStruct {
            #[serde(
                default,
                skip_serializing_if = "Option::is_none",
                deserialize_with = "link_action_deserialize"
            )]
            pub action: Option<String>, /*signIn, signOut, appLaunch,  */
        }
        let json_str = r#"{"action": "appLauncher"}"#;
        let test_struct = serde_json::from_str::<TestStruct>(&json_str);
        assert!(test_struct.is_err());
    }

    #[test]
    fn test_link_type_valid_value() {
        #[derive(Serialize, Deserialize, Debug)]
        struct TestStruct {
            #[serde(
                default,
                rename = "type",
                skip_serializing_if = "Option::is_none",
                deserialize_with = "link_type_deserialize"
            )]
            pub link_type: Option<String>,
        }
        let json_str = r#"{"type": "entitlementsUpdate"}"#;
        let test_struct = serde_json::from_str::<TestStruct>(&json_str);
        assert!(test_struct.is_ok());
    }

    #[test]
    fn test_link_type_invalid_value() {
        #[derive(Serialize, Deserialize, Debug)]
        struct TestStruct {
            #[serde(
                default,
                rename = "type",
                skip_serializing_if = "Option::is_none",
                deserialize_with = "link_type_deserialize"
            )]
            pub link_type: Option<String>,
        }
        let json_str = r#"{"type": "entitlements"}"#;
        let test_struct = serde_json::from_str::<TestStruct>(&json_str);
        assert!(test_struct.is_err());
    }

    #[test]
    fn test_progress_deserialize_positive_value() {
        #[derive(Deserialize, Debug)]
        struct TestStruct {
            #[serde(default, deserialize_with = "progress_value_deserialize")]
            pub progress_value: f32,
        }
        let json_str = r#"{"progress_value": 0.95}"#;
        let test_struct = serde_json::from_str::<TestStruct>(&json_str);
        assert!(test_struct.is_ok());
        let result = test_struct.unwrap();
        assert_eq!(result.progress_value, 0.95);
    }

    #[test]
    fn test_progress_deserialize_negative_value() {
        #[derive(Deserialize, Debug)]
        struct TestStruct {
            #[serde(default, deserialize_with = "progress_value_deserialize")]
            pub progress_value: f32,
        }
        let json_str = r#"{"progress_value": -0.95}"#;
        let test_struct = serde_json::from_str::<TestStruct>(&json_str);
        assert!(test_struct.is_err());
        let result = test_struct.unwrap_err();
        assert!(result.to_string().starts_with(&String::from(
            "Invalid value for progress. Minimum value should be 0.0"
        )));
    }

    #[test]
    fn test_progress_deserialize_no_value() {
        #[derive(Deserialize, Debug)]
        struct TestStruct {
            #[serde(default, deserialize_with = "progress_value_deserialize")]
            pub progress_value: f32,
        }
        let json_str = r#"{}"#;
        let test_struct = serde_json::from_str::<TestStruct>(&json_str);

        assert!(test_struct.is_ok());

        let result = test_struct.unwrap();
        assert_eq!(result.progress_value, 0.0);
    }
}
