// If not stated otherwise in this file or this component's license file the
// following copyright and licenses apply:
//
// Copyright 2023 RDK Management
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

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::api::device::device_request::AudioProfile;
use crate::utils::serde_utils::*;
#[derive(Debug, Deserialize, Default, Clone)]
pub struct ProviderRequest {}

#[derive(Debug, Serialize, Default, Clone)]
pub struct ContentProvider {
    pub id: String,
    pub apis: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum OfferingType {
    FREE,
    SUBSCRIBE,
    BUY,
    RENT,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FederationOptions {
    pub timeout: u32,
}

impl Default for FederationOptions {
    fn default() -> Self {
        FederationOptions { timeout: 3000 }
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContentIdentifiers {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub season_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_content_data: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContentRating {
    pub scheme: SchemeValue,
    pub rating: RatingValue,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advisories: Option<Vec<AdvisoriesValue>>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WaysToWatch {
    pub identifiers: ContentIdentifiers,
    #[serde(
        default,
        with = "optional_date_time_str_serde",
        skip_serializing_if = "Option::is_none"
    )]
    pub expires: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entitled: Option<bool>,
    #[serde(
        default,
        with = "optional_date_time_str_serde",
        skip_serializing_if = "Option::is_none"
    )]
    pub entitled_expires: Option<String>, //date-time format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offering_type: Option<OfferingType>, // One of valid values from OfferingTypeValues
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_ads: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<f32>, //TODO: need to convert to f32
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_quality: Option<Vec<VideoQuality>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_profile: Option<Vec<AudioProfile>>, //One of valid values from AudioProfileValues
    #[serde(
        default,
        with = "optional_language_code_list_serde",
        skip_serializing_if = "Option::is_none"
    )]
    pub audio_languages: Option<Vec<String>>,
    #[serde(
        default,
        with = "optional_language_code_list_serde",
        skip_serializing_if = "Option::is_none"
    )]
    pub closed_captions: Option<Vec<String>>,
    #[serde(
        default,
        with = "optional_language_code_list_serde",
        skip_serializing_if = "Option::is_none"
    )]
    pub subtitles: Option<Vec<String>>,
    #[serde(
        default,
        with = "optional_language_code_list_serde",
        skip_serializing_if = "Option::is_none"
    )]
    pub audio_descriptions: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EntityInfo {
    pub identifiers: ContentIdentifiers,
    pub title: String,
    pub entity_type: String,       //constant "program"
    pub program_type: ProgramType, // One of valid values from ProgramTypeValues
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synopsis: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub season_number: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episode_number: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>, // date-time format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_ratings: Option<Vec<ContentRating>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ways_to_watch: Option<Vec<WaysToWatch>>,
}

impl Default for EntityInfo {
    fn default() -> Self {
        EntityInfo {
            identifiers: Default::default(),
            title: Default::default(),
            entity_type: Default::default(),
            program_type: ProgramType::Other,
            synopsis: None,
            season_number: None,
            episode_number: None,
            release_date: None,
            content_ratings: None,
            ways_to_watch: Default::default(),
        }
    }
}

#[cfg(test)]
#[allow(dead_code)]
pub const SYNOPSIS: &'static str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Pulvinar sapien et ligula ullamcorper malesuada proin libero nunc.";

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum ProgramType {
    Movie,
    Episode,
    Season,
    Series,
    Other,
    Preview,
    Extra,
    Concert,
    SportingEvent,
    Advertisement,
    MusicVideo,
    Minisode,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub enum VideoQuality {
    Sd,
    Hd,
    Uhd,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SchemeValue {
    #[serde(rename = "CA-Movie")]
    CaMovie,
    #[serde(rename = "CA-TV")]
    CaTv,
    #[serde(rename = "CA-Movie-Fr")]
    CaMovieFr,
    #[serde(rename = "CA-TV-Fr")]
    CaTvFr,
    #[serde(rename = "US-Movie")]
    UsMovie,
    #[serde(rename = "US-TV")]
    UsTv,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RatingValue {
    NR,
    G,
    PG,
    PG13,
    R,
    NC17,
    TVY,
    TVY7,
    TVG,
    TVPG,
    TV14,
    TVMA,
    E,
    C,
    C8,
    #[serde(rename = "8+")]
    Plus8,
    #[serde(rename = "13+")]
    Plus13,
    #[serde(rename = "14+")]
    Plus14,
    #[serde(rename = "16+")]
    Plus16,
    #[serde(rename = "18+")]
    Plus18,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AdvisoriesValue {
    AT,
    BN,
    SL,
    SS,
    N,
    FV,
    D,
    L,
    S,
    V,
    C,
    C8,
    G,
    PG,
    #[serde(rename = "14+")]
    Plus14,
    #[serde(rename = "18+")]
    Plus18,
}

#[derive(Debug, Serialize, Default, Clone)]
pub struct ProviderResult {
    pub entries: HashMap<String, Vec<String>>,
}

// Adding impl with new function since object is created in provider broker
impl ProviderResult {
    pub fn new(entries: HashMap<String, Vec<String>>) -> Self {
        ProviderResult { entries }
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PurchasedContentResult {
    pub expires: String, //date-time Representation
    pub total_count: i32,
    pub entries: Vec<EntityInfo>,
}

//Struct to be used as the response from Apps to Ripple
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PurchasedContentParameters {
    pub limit: i32,
    pub offering_type: Option<OfferingType>, // One of valid values from OfferingType.
    pub program_type: Option<ProgramType>,   //One of valid values from ProgramTypeValue
}

// Struct to be used from AggExp to Ripple
#[derive(Debug, Deserialize, Default, Clone)]
pub struct ProvidedPurchaseContentRequest {
    pub provider: String,
    pub parameters: PurchasedContentParameters,
    pub options: Option<FederationOptions>,
}

// Struct to be used from Ripple to AggExp
#[derive(Debug, Serialize, Default, Clone)]
pub struct ProvidedPurchasedContentResult {
    pub provider: String,
    pub data: PurchasedContentResult,
}

// Struct used as parameter in entity request
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EntityInfoParameters {
    pub entity_id: String,
    pub asset_id: Option<String>,
}

//Struct to be used from AggExp to Ripple
#[derive(Debug, Deserialize, Default, Clone)]
pub struct ContentEntityRequest {
    pub provider: String,
    pub parameters: EntityInfoParameters,
    pub options: Option<FederationOptions>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct EntityInfoResult {
    pub expires: String,
    pub entity: EntityInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related: Option<EntityInfo>,
}

// Struct to be used from Ripple to AggExp
#[derive(Debug, Serialize, Default, Clone)]
pub struct ContentEntityResponse {
    pub provider: String,
    pub data: Option<EntityInfoResult>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ContentPolicy {
    pub enable_recommendations: bool,
    pub share_watch_history: bool,
    pub remember_watched_programs: bool,
}
