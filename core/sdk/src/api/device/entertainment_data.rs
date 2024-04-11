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

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::{collections::HashMap, fmt};

use crate::api::{device::device_request::AudioProfile, firebolt::fb_discovery::DiscoveryContext};
use crate::utils::serde_utils::*;
#[derive(Debug, Deserialize, Default, Clone)]
pub struct ProviderRequest {}

#[derive(Debug, Serialize, Default, Clone)]
pub struct ContentProvider {
    pub id: String,
    pub apis: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
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
#[cfg_attr(test, derive(PartialEq))]
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
#[cfg_attr(test, derive(PartialEq))]
pub struct ContentRating {
    pub scheme: SchemeValue,
    #[serde(deserialize_with = "rating_format_extender")]
    pub rating: RatingValue,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advisories: Option<Vec<AdvisoriesValue>>,
}

pub fn rating_format_extender<'de, D>(deserializer: D) -> Result<RatingValue, D::Error>
where
    D: Deserializer<'de>,
{
    let rating_string = String::deserialize(deserializer)?;
    match rating_string.as_str() {
        "PG-13" => Ok(RatingValue::PG13),
        "NC-17" => Ok(RatingValue::NC17),
        "TV-Y" => Ok(RatingValue::TVY),
        "TV-Y7" => Ok(RatingValue::TVY7),
        "TV-G" => Ok(RatingValue::TVG),
        "TV-PG" => Ok(RatingValue::TVG),
        "TV-14" => Ok(RatingValue::TV14),
        "TV-MA" => Ok(RatingValue::TVMA),
        _ => match serde_json::from_str::<RatingValue>(&rating_string) {
            Ok(v) => Ok(v),
            Err(e) => Err(serde::de::Error::custom(format!(
                "invalid Rating value {:?}",
                e
            ))),
        },
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
#[cfg_attr(test, derive(PartialEq))]
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
#[cfg_attr(test, derive(PartialEq))]
#[serde(rename_all = "camelCase")]
pub struct EntityInfo {
    pub identifiers: ContentIdentifiers,
    pub title: String,
    pub entity_type: EntityType, //constant "program"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program_type: Option<ProgramType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub music_type: Option<MusicType>, // One of valid values from ProgramTypeValues
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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(rename_all = "camelCase")]
pub enum EntityType {
    Program,
    Music,
}

impl Default for EntityInfo {
    fn default() -> Self {
        EntityInfo {
            identifiers: Default::default(),
            title: Default::default(),
            entity_type: EntityType::Program,
            program_type: None,
            music_type: None,
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
pub const SYNOPSIS: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Pulvinar sapien et ligula ullamcorper malesuada proin libero nunc.";

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
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

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum MusicType {
    Song,
    Album,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(rename_all = "UPPERCASE")]
pub enum VideoQuality {
    Sd,
    Hd,
    Uhd,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
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
#[cfg_attr(test, derive(PartialEq))]
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
#[cfg_attr(test, derive(PartialEq))]
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
#[cfg_attr(test, derive(PartialEq))]
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
#[cfg_attr(test, derive(PartialEq))]
pub struct EntityInfoResult {
    pub expires: String,
    #[serde(deserialize_with = "entity_info_deserialize")]
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

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum NavigationIntent {
    NavigationIntentStrict(NavigationIntentStrict),
    NavigationIntentLoose(NavigationIntentLoose),
}

// Original Navigation Intent is untagged meaning it cant be used  to serialize again when passed between extensions which also uses serde
// To avoid the data loss during IEC InternalNavigationIntent is created so the Firebolt specification is not affected
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum InternalNavigationIntent {
    NavigationIntentStrict(InternalNavigationIntentStrict),
    NavigationIntentLoose(NavigationIntentLoose),
}

impl From<NavigationIntent> for InternalNavigationIntent {
    fn from(value: NavigationIntent) -> Self {
        match value {
            NavigationIntent::NavigationIntentLoose(l) => Self::NavigationIntentLoose(l),
            NavigationIntent::NavigationIntentStrict(s) => Self::NavigationIntentStrict(s.into()),
        }
    }
}

// TODO: Compiler didnt accept the above From implementation to go both ways. Remove it in future
impl From<InternalNavigationIntent> for NavigationIntent {
    fn from(value: InternalNavigationIntent) -> Self {
        match value {
            InternalNavigationIntent::NavigationIntentLoose(l) => Self::NavigationIntentLoose(l),
            InternalNavigationIntent::NavigationIntentStrict(s) => {
                Self::NavigationIntentStrict(s.into())
            }
        }
    }
}

impl Default for NavigationIntent {
    fn default() -> Self {
        NavigationIntent::NavigationIntentStrict(
            NavigationIntentStrict::Home(HomeIntent::default()),
        )
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(tag = "action", rename_all = "kebab-case", deny_unknown_fields)]

pub enum NavigationIntentStrict {
    Home(HomeIntent),
    Launch(LaunchIntent),
    Entity(EntityIntent),
    Playback(PlaybackIntent),
    Search(SearchIntent),
    Section(SectionIntent),
    Tune(TuneIntent),
    ProviderRequest(ProviderRequestIntent),
    PlayEntity(PlayEntityIntent),
    PlayQuery(PlayQueryIntent),
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(tag = "action", rename_all = "kebab-case", deny_unknown_fields)]

pub enum InternalNavigationIntentStrict {
    Home(HomeIntent),
    Launch(LaunchIntent),
    Entity(InternalEntityIntent),
    Playback(PlaybackIntent),
    Search(SearchIntent),
    Section(SectionIntent),
    Tune(TuneIntent),
    ProviderRequest(ProviderRequestIntent),
    PlayEntity(PlayEntityIntent),
    PlayQuery(PlayQueryIntent),
}

impl From<InternalNavigationIntentStrict> for NavigationIntentStrict {
    fn from(value: InternalNavigationIntentStrict) -> Self {
        match value {
            InternalNavigationIntentStrict::Tune(t) => NavigationIntentStrict::Tune(t),
            InternalNavigationIntentStrict::Entity(e) => NavigationIntentStrict::Entity(e.into()),
            InternalNavigationIntentStrict::Home(h) => NavigationIntentStrict::Home(h),
            InternalNavigationIntentStrict::Launch(l) => NavigationIntentStrict::Launch(l),
            InternalNavigationIntentStrict::Playback(p) => NavigationIntentStrict::Playback(p),
            InternalNavigationIntentStrict::ProviderRequest(p) => {
                NavigationIntentStrict::ProviderRequest(p)
            }
            InternalNavigationIntentStrict::Search(s) => NavigationIntentStrict::Search(s),
            InternalNavigationIntentStrict::Section(s) => NavigationIntentStrict::Section(s),
            InternalNavigationIntentStrict::PlayEntity(p) => NavigationIntentStrict::PlayEntity(p),
            InternalNavigationIntentStrict::PlayQuery(p) => NavigationIntentStrict::PlayQuery(p),
        }
    }
}

impl From<NavigationIntentStrict> for InternalNavigationIntentStrict {
    fn from(value: NavigationIntentStrict) -> Self {
        match value {
            NavigationIntentStrict::Tune(t) => InternalNavigationIntentStrict::Tune(t),
            NavigationIntentStrict::Entity(e) => InternalNavigationIntentStrict::Entity(e.into()),
            NavigationIntentStrict::Home(h) => InternalNavigationIntentStrict::Home(h),
            NavigationIntentStrict::Launch(l) => InternalNavigationIntentStrict::Launch(l),
            NavigationIntentStrict::Playback(p) => InternalNavigationIntentStrict::Playback(p),
            NavigationIntentStrict::ProviderRequest(p) => {
                InternalNavigationIntentStrict::ProviderRequest(p)
            }
            NavigationIntentStrict::Search(s) => InternalNavigationIntentStrict::Search(s),
            NavigationIntentStrict::Section(s) => InternalNavigationIntentStrict::Section(s),
            NavigationIntentStrict::PlayEntity(p) => InternalNavigationIntentStrict::PlayEntity(p),
            NavigationIntentStrict::PlayQuery(p) => InternalNavigationIntentStrict::PlayQuery(p),
        }
    }
}

/*
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Context {
    pub source: String,
}
*/

#[derive(Serialize, PartialEq, Deserialize, Clone, Debug)]
pub struct NavigationIntentLoose {
    pub action: String,
    pub data: Option<Value>,
    pub context: DiscoveryContext,
}

impl Default for NavigationIntentStrict {
    fn default() -> Self {
        NavigationIntentStrict::Home(HomeIntent::default())
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct HomeIntent {
    pub context: DiscoveryContext,
}

impl Default for HomeIntent {
    fn default() -> HomeIntent {
        HomeIntent {
            context: DiscoveryContext {
                source: "device".to_string(),
            },
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct LaunchIntent {
    pub context: DiscoveryContext,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct EntityIntent {
    #[serde(deserialize_with = "entity_data_deserialize")]
    pub data: EntityIntentData,
    pub context: DiscoveryContext,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct InternalEntityIntent {
    pub data: EntityIntentData,
    pub context: DiscoveryContext,
}

impl From<InternalEntityIntent> for EntityIntent {
    fn from(value: InternalEntityIntent) -> Self {
        Self {
            data: value.data,
            context: value.context,
        }
    }
}

impl From<EntityIntent> for InternalEntityIntent {
    fn from(value: EntityIntent) -> Self {
        Self {
            data: value.data,
            context: value.context,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum EntityIntentData {
    Program(ProgramEntityIntentData),
    Untyped(UntypedEntity),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EntityIntentDataBase {
    entity_type: Option<String>,
}

pub fn entity_data_deserialize<'de, D>(deserializer: D) -> Result<EntityIntentData, D::Error>
where
    D: Deserializer<'de>,
{
    let val = Value::deserialize(deserializer)?;
    let base: EntityIntentDataBase =
        serde_json::from_value(val.clone()).map_err(serde::de::Error::custom)?;
    match base.entity_type {
        Some(et) if et == "program" => {
            let pgm = serde_json::from_value(val).map_err(serde::de::Error::custom)?;
            return Ok(EntityIntentData::Program(pgm));
        }
        _ => {
            let program_type = val.get("programType");
            if program_type.is_some() {
                let pgm = serde_json::from_value(val.clone()).map_err(serde::de::Error::custom)?;
                return Ok(EntityIntentData::Program(pgm));
            }
        }
    }
    let ut = serde_json::from_value(val).map_err(serde::de::Error::custom)?;

    Ok(EntityIntentData::Untyped(ut))
}

pub fn entity_info_deserialize<'de, D>(deserializer: D) -> Result<EntityInfo, D::Error>
where
    D: Deserializer<'de>,
{
    let val = EntityInfo::deserialize(deserializer)?;
    match val.entity_type {
        EntityType::Program => {
            if val.program_type.is_none() {
                return Err(serde::de::Error::custom(
                    "missing programType for entityType=program",
                ));
            }
        }
        EntityType::Music => {
            if val.music_type.is_none() {
                return Err(serde::de::Error::custom(
                    "missing musicType for entityType=music",
                ));
            }
        }
    }
    Ok(val)
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(tag = "programType", rename_all = "camelCase")]
pub enum ProgramEntityIntentData {
    #[serde(alias = "Movie", alias = "MOVIE")]
    Movie(MovieEntity),
    #[serde(alias = "Episode", alias = "EPISODE")]
    Episode(TVEpisodeEntity),
    #[serde(alias = "Season", alias = "SEASON")]
    Season(TVSeasonEntity),
    #[serde(alias = "Series", alias = "SERIES")]
    Series(TVSeriesEntity),
    #[serde(alias = "Concert", alias = "CONCERT")]
    Concert(AdditionalEntity),
    #[serde(alias = "SportingEvent", alias = "SPORTINGEVENT")]
    SportingEvent(AdditionalEntity),
    #[serde(alias = "Preview", alias = "PREVIEW")]
    Preview(AdditionalEntity),
    #[serde(alias = "Other", alias = "OTHER")]
    Other(AdditionalEntity),
    #[serde(alias = "Advertisement", alias = "ADVERTISEMENT")]
    Advertisement(AdditionalEntity),
    #[serde(alias = "MusicVideo", alias = "MUSICVIDEO")]
    MusicVideo(AdditionalEntity),
    #[serde(alias = "Minisode", alias = "MINISODE")]
    Minisode(AdditionalEntity),
    #[serde(alias = "Extra", alias = "EXTRA")]
    Extra(AdditionalEntity),
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BaseEntity {
    #[serde(default)]
    pub entity_type: ProgramEntityType,
    pub entity_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_content_data: Option<AppContentDataString>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct AppContentDataString(String);

impl fmt::Display for AppContentDataString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let max_len = 256;
        let truncated_str = if self.0.chars().count() > max_len {
            self.0.chars().take(max_len).collect::<String>()
        } else {
            self.0.clone()
        };
        write!(f, "{}", truncated_str)
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct ProgramEntityType(String);

impl Default for ProgramEntityType {
    fn default() -> Self {
        Self(String::from("program"))
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MovieEntity {
    #[serde(flatten)]
    pub base_entity: BaseEntity,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TVEpisodeEntity {
    #[serde(flatten)]
    pub base_entity: BaseEntity,
    pub series_id: String,
    pub season_id: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TVSeasonEntity {
    #[serde(flatten)]
    pub base_entity: BaseEntity,
    pub series_id: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TVSeriesEntity {
    #[serde(flatten)]
    pub base_entity: BaseEntity,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AdditionalEntity {
    #[serde(flatten)]
    pub base_entity: BaseEntity,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistEntity {
    #[serde(flatten)]
    pub base_entity: BaseEntity,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UntypedEntity {
    pub entity_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_content_data: Option<AppContentDataString>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct TuneIntent {
    pub data: TuneIntentData,
    pub context: DiscoveryContext,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct TuneIntentData {
    pub entity: ChannelEntity,
    pub options: TuneIntentDataOptions,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct ChannelEntityType(String);

impl Default for ChannelEntityType {
    fn default() -> Self {
        Self(String::from("channel"))
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChannelEntity {
    #[serde(default)]
    pub entity_type: ChannelEntityType,
    pub channel_type: ChannelType,
    pub entity_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_content_data: Option<AppContentDataString>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ChannelType {
    Streaming,
    OverTheAir,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TuneIntentDataOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart_current_program: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PlaybackIntent {
    pub data: ProgramTypeEntity,
    pub context: DiscoveryContext,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PlayEntityIntent {
    #[serde(deserialize_with = "play_entity_intent_validator")]
    pub data: PlayEntityIntentData,
    pub context: DiscoveryContext,
}

pub fn play_entity_intent_validator<'de, D>(
    deserializer: D,
) -> Result<PlayEntityIntentData, D::Error>
where
    D: Deserializer<'de>,
{
    let val = PlayEntityIntentData::deserialize(deserializer)?;
    let val_c = val.clone();
    if val_c.entity.entity_type.0.eq_ignore_ascii_case("playlist") {
        if let Some(options) = val_c.options {
            if !options.is_valid() {
                return Err(serde::de::Error::custom(
                    "invalid options for entityType=playlist",
                ));
            }
        }
    }

    Ok(val)
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PlayEntityIntentData {
    pub entity: BaseEntity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<PlayEntityIntentDataOptions>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlayEntityIntentDataOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub play_first_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub play_first_track: Option<u32>,
}

impl PlayEntityIntentDataOptions {
    fn is_valid(&self) -> bool {
        if let Some(id) = &self.play_first_id {
            if !id.is_empty() {
                return true;
            }
        } else if let Some(track) = &self.play_first_track {
            if track.ge(&1) {
                return true;
            }
        }
        false
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PlayQueryIntent {
    pub data: PlayQueryIntentData,
    pub context: DiscoveryContext,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PlayQueryIntentData {
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<PlayQueryIntentDataOptions>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlayQueryIntentDataOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program_types: Option<Vec<ProgramType>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub music_types: Option<Vec<MusicType>>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(tag = "programType", rename_all = "camelCase")]
pub enum ProgramTypeEntity {
    #[serde(alias = "Movie", alias = "MOVIE")]
    Movie(MovieEntity),
    #[serde(alias = "Episode", alias = "EPISODE")]
    Episode(TVEpisodeEntity),
    #[serde(alias = "Concert", alias = "CONCERT")]
    Concert(AdditionalEntity),
    #[serde(alias = "SportingEvent", alias = "SPORTINGEVENT")]
    SportingEvent(AdditionalEntity),
    #[serde(alias = "Preview", alias = "PREVIEW")]
    Preview(AdditionalEntity),
    #[serde(alias = "Other", alias = "OTHER")]
    Other(AdditionalEntity),
    #[serde(alias = "Advertisement", alias = "ADVERTISEMENT")]
    Advertisement(AdditionalEntity),
    #[serde(alias = "MusicVideo", alias = "MUSICVIDEO")]
    MusicVideo(AdditionalEntity),
    #[serde(alias = "Minisode", alias = "MINISODE")]
    Minisode(AdditionalEntity),
    #[serde(alias = "Extra", alias = "EXTRA")]
    Extra(AdditionalEntity),
    #[serde(alias = "Playlist", alias = "PLAYLIST")]
    Playlist(PlaylistEntity),
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct SearchIntent {
    pub data: SearchIntentData,
    pub context: DiscoveryContext,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct SearchIntentData {
    pub query: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct SectionIntent {
    pub data: SectionIntentData,
    pub context: DiscoveryContext,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SectionIntentData {
    pub section_name: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct ProviderRequestIntent {
    pub context: DiscoveryContext,
}

impl Default for ProviderRequestIntent {
    fn default() -> ProviderRequestIntent {
        ProviderRequestIntent {
            context: DiscoveryContext {
                source: "device".to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    pub fn test_base_entity() {
        if let Ok(v) = serde_json::from_str::<BaseEntity>("{\"appContentData\":null,\"assetId\":null,\"entityId\":\"example-movie-id\",\"entityType\":\"program\",\"programType\":\"movie\"}"){
            assert!(v.entity_id.eq("example-movie-id"));
        } else {
            panic!("failed schema expectation")
        }
    }

    #[test]
    pub fn test_navigation_intent_playback_strict() {
        if let Ok(v) = serde_json::from_str::<NavigationIntentStrict>("{\"action\":\"playback\",\"data\":{\"entityId\":\"example-movie-id\",\"programType\":\"movie\"},\"context\":{\"source\":\"voice\"}}"){
            if let NavigationIntentStrict::Playback(p) = v {
                assert!(p.context.source.eq("voice"));
                if let ProgramTypeEntity::Movie(m) = p.data {
                    assert!(m.base_entity.entity_id.eq("example-movie-id"));
                } else {
                    panic!("Wrong enum for playback")
                }
            } else {
                panic!("Wrong enum for navigation intent")
            }
        } else {
            panic!("failed schema expectation");
        }
    }

    #[test]
    pub fn test_navigation_intent_entity_strict() {
        let intent = NavigationIntentStrict::Entity(EntityIntent {
            data: EntityIntentData::Program(ProgramEntityIntentData::Movie(MovieEntity {
                base_entity: BaseEntity {
                    entity_type: ProgramEntityType("program".to_owned()),
                    entity_id: "example-movie-id".to_owned(),
                    asset_id: None,
                    app_content_data: None,
                },
            })),
            context: DiscoveryContext {
                source: "xrn:firebolt:application:refui".to_owned(),
            },
        });
        let value = serde_json::to_string(&intent).unwrap();
        assert!(value.eq("{\"action\":\"entity\",\"data\":{\"programType\":\"movie\",\"entityType\":\"program\",\"entityId\":\"example-movie-id\"},\"context\":{\"source\":\"xrn:firebolt:application:refui\"}}"));
    }

    #[test]
    pub fn test_navigation_intent_tune_strict() {
        match serde_json::from_str::<NavigationIntentStrict>(
            "{\"action\":\"tune\",\"data\":{
                \"entity\":{
                \"entityType\": \"program\",
                \"channelType\": \"streaming\",
                \"programType\": \"movie\",
                \"entityId\": \"example-movie-id\"},
                \"options\":{\"restartCurrentProgram\":true}
                },
                \"context\":{
                    \"source\":\"voice\"
                }
            }",
        ) {
            Ok(v) => {
                if let NavigationIntentStrict::Tune(t) = v {
                    assert!(t.context.source.eq("voice"));
                    if !matches!(t.data.entity.channel_type, ChannelType::Streaming) {
                        panic!("ChannelType mismatch");
                    }
                    assert!(t.data.entity.entity_type.0.eq("program"));
                } else {
                    panic!("Not tune intent");
                }
            }
            Err(e) => {
                panic!("failed schema expectation {:?}", e);
            }
        }
    }

    #[test]
    fn deserializing_enity_info_entity_type() {
        // valid program entry
        let v = json!({
            "expires": "2025-01-01T00:00:00.000Z",
            "entity": {
                "identifiers": {
                    "entityId": "98765"
                },
                "entityType": "program",
                "programType": "series",
                "title": "Perfect Strangers",
                "synopsis": "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Pulvinar sapien et ligula ullamcorper malesuada proin libero nunc.",
                "releaseDate": "1986-01-01T00:00:00.000Z",
                "contentRatings": [
                    {
                        "scheme": "US-TV",
                        "rating": "TV-PG"
                    }
                ]
            }
        });
        if let Err(e) = serde_json::from_value::<EntityInfoResult>(v) {
            panic!("{:?}", e)
        }

        // invalid program entry
        let v = json!({
            "expires": "2025-01-01T00:00:00.000Z",
            "entity": {
                "identifiers": {
                    "entityId": "98765"
                },
                "entityType": "program",
                "title": "Perfect Strangers",
                "synopsis": "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Pulvinar sapien et ligula ullamcorper malesuada proin libero nunc.",
                "releaseDate": "1986-01-01T00:00:00.000Z",
                "contentRatings": [
                    {
                        "scheme": "US-TV",
                        "rating": "TV-PG"
                    }
                ]
            }
        });
        if let Err(e) = serde_json::from_value::<EntityInfoResult>(v) {
            assert!(e
                .to_string()
                .contains("missing programType for entityType=program"))
        } else {
            panic!("expecting error for entityType of program without ProgramType")
        }

        // valid music entry
        let v = json!({
            "expires": "2025-01-01T00:00:00.000Z",
            "entity": {
                "identifiers": {
                    "entityId": "98765"
                },
                "entityType": "music",
                "musicType": "song",
                "title": "Perfect Strangers",
                "synopsis": "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Pulvinar sapien et ligula ullamcorper malesuada proin libero nunc.",
                "releaseDate": "1986-01-01T00:00:00.000Z",
                "contentRatings": [
                    {
                        "scheme": "US-TV",
                        "rating": "TV-PG"
                    }
                ]
            }
        });
        if let Err(e) = serde_json::from_value::<EntityInfoResult>(v) {
            panic!("{:?}", e)
        }

        // invalid music entry
        let v = json!({
            "expires": "2025-01-01T00:00:00.000Z",
            "entity": {
                "identifiers": {
                    "entityId": "98765"
                },
                "entityType": "music",
                "title": "Perfect Strangers",
                "synopsis": "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Pulvinar sapien et ligula ullamcorper malesuada proin libero nunc.",
                "releaseDate": "1986-01-01T00:00:00.000Z",
                "contentRatings": [
                    {
                        "scheme": "US-TV",
                        "rating": "TV-PG"
                    }
                ]
            }
        });
        if let Err(e) = serde_json::from_value::<EntityInfoResult>(v) {
            assert!(e
                .to_string()
                .contains("missing musicType for entityType=music"))
        } else {
            panic!("expecting error for entityType of music without musicType")
        }
    }

    #[test]
    pub fn test_playentity_intent() {
        let v = json!({
                "action": "play-entity",
                "data": {
                    "entity": {
                        "entityType": "playlist",
                        "entityId": "playlist/xyz"
                    },
                    "options": {
                        "playFirstId": "song/xyz"
                    }
                },
                "context": {
                    "source": "voice"
                }
        });
        if let Err(e) = serde_json::from_value::<NavigationIntentStrict>(v) {
            panic!("{:?}", e)
        }

        let v = json!({
            "action": "play-entity",
            "data": {
                "entity": {
                    "entityType": "playlist",
                    "entityId": "playlist/xyz"
                },
                "options": {
                    "playFirstTrack": 3
                }
            },
            "context": {
                "source": "voice"
            }
        });
        if let Err(e) = serde_json::from_value::<NavigationIntentStrict>(v) {
            panic!("{:?}", e)
        }

        // missing options
        let v = json!({
                "action": "play-entity",
                "data": {
                    "entity": {
                        "entityType": "playlist",
                        "entityId": "playlist/xyz"
                    }
                },
                "context": {
                    "source": "voice"
                }
        });

        if let Err(e) = serde_json::from_value::<NavigationIntentStrict>(v) {
            panic!("{:?}", e)
        }

        let v = json!({
                "action": "play-entity",
                "data": {
                    "entity": {
                        "entityType": "playlist",
                        "entityId": "playlist/xyz"
                    },
                    "options": {
                        "playFirstTrack": 0
                    }
                },
                "context": {
                    "source": "voice"
                }
        });

        if let Err(e) = serde_json::from_value::<NavigationIntentStrict>(v) {
            assert!(e
                .to_string()
                .contains("invalid options for entityType=playlist"))
        } else {
            panic!("expecting error for entityType of playlist without options")
        }
    }

    #[test]
    pub fn test_play_query_intent() {
        let v = json!({
            "action": "play-query",
            "data": {
                "query": "Ed Sheeran"
            },
            "context": {
                "source": "voice"
            }
        });
        if let Err(e) = serde_json::from_value::<NavigationIntentStrict>(v) {
            panic!("{:?}", e)
        }

        let v = json!({
            "action": "play-query",
            "data": {
                "query": "Ed Sheeran",
                "options": {
                    "programTypes": [
                        "movie"
                    ]
                }
            },
            "context": {
                "source": "voice"
            }
        });
        match serde_json::from_value::<NavigationIntentStrict>(v) {
            Ok(NavigationIntentStrict::PlayQuery(v)) => {
                assert!(matches!(
                    v.data
                        .options
                        .unwrap()
                        .program_types
                        .unwrap()
                        .first()
                        .unwrap(),
                    ProgramType::Movie
                ));
            }
            _ => panic!("parser error "),
        }

        let v = json!({
            "action": "play-query",
            "data": {
                "query": "Ed Sheeran",
                "options": {
                    "programTypes": [
                        "movie"
                    ],
                    "musicTypes": [
                        "song"
                    ]
                }
            },
            "context": {
                "source": "voice"
            }
        });
        match serde_json::from_value::<NavigationIntentStrict>(v) {
            Ok(NavigationIntentStrict::PlayQuery(v)) => {
                if let Some(v) = v.data.options {
                    assert!(matches!(
                        v.program_types.unwrap().first().unwrap(),
                        ProgramType::Movie
                    ));
                    assert!(matches!(
                        v.music_types.unwrap().first().unwrap(),
                        MusicType::Song
                    ));
                }
            }
            _ => panic!("parser error "),
        }

        let v = json!({
            "action": "play-query",
            "data": {},
            "context": {
                "source": "voice"
            }
        });
        if let Err(e) = serde_json::from_value::<NavigationIntentStrict>(v) {
            assert!(e.to_string().contains("missing field"))
        } else {
            panic!("expecting error for query field")
        }
    }

    #[tokio::test]
    async fn test_navigation_intent_missing_entity_type() {
        // missing entityType should look to see if programType is defined
        // if yes, then should be program entity and parse as such
        // if no, then should be strict untyped entity
        let json = r#"{
            "action": "entity",
            "context": {
                "source": "voice"
            },
            "data": {
                "entityId": "123",
                "programType": "movie"
            }
        }"#;
        match serde_json::from_str::<NavigationIntentStrict>(json) {
            Ok(i) => match i {
                NavigationIntentStrict::Entity(ei) => match ei.data {
                    EntityIntentData::Program(pei) => match pei {
                        ProgramEntityIntentData::Movie(_) => {
                            println!("parsed a movie intent")
                        }
                        _ => panic!(),
                    },
                    EntityIntentData::Untyped(_) => panic!(),
                },
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
        let json = r#"{
            "action": "entity",
            "context": {
                "source": "voice"
            },
            "data": {
                "entityId": "123"
            }
        }"#;

        match serde_json::from_str::<NavigationIntentStrict>(json) {
            Ok(i) => match i {
                NavigationIntentStrict::Entity(ei) => match ei.data {
                    EntityIntentData::Program(_) => panic!(),
                    EntityIntentData::Untyped(_) => println!("parsed a untyped entity intent"),
                },
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }
}
