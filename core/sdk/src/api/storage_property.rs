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

use serde::{Deserialize, Serialize};

use crate::{
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::{ContractAdjective, RippleContract},
};

use super::distributor::distributor_privacy::PrivacySetting;

pub const NAMESPACE_CLOSED_CAPTIONS: &str = "ClosedCaptions";
pub const NAMESPACE_PRIVACY: &str = "Privacy";
pub const NAMESPACE_DEVICE_NAME: &str = "DeviceName";
pub const NAMESPACE_LOCALIZATION: &str = "Localization";
pub const NAMESPACE_USER_GRANT: &str = "UserGrant";
pub const NAMESPACE_ADVERTISING: &str = "Advertising";

pub const KEY_ENABLED: &str = "enabled";
pub const KEY_FONT_FAMILY: &str = "fontFamily";
pub const KEY_FONT_SIZE: &str = "fontSize";
pub const KEY_FONT_COLOR: &str = "fontColor";
pub const KEY_FONT_EDGE: &str = "fontEdge";
pub const KEY_FONT_EDGE_COLOR: &str = "fontEdgeColor";
pub const KEY_FONT_OPACITY: &str = "fontOpacity";
pub const KEY_BACKGROUND_COLOR: &str = "backgroundColor";
pub const KEY_BACKGROUND_OPACITY: &str = "backgroundOpacity";
pub const KEY_TEXT_ALIGN: &str = "textAlign";
pub const KEY_TEXT_ALIGN_VERTICAL: &str = "textAlignVertical";
pub const KEY_LIMIT_AD_TRACKING: &str = "limitAdTracking";
pub const KEY_NAME: &str = "name";
pub const KEY_POSTAL_CODE: &str = "postalCode";
pub const KEY_LOCALITY: &str = "locality";
pub const KEY_COUNTRY_CODE: &str = "countryCode";
pub const KEY_LANGUAGE: &str = "language";
pub const KEY_LOCALE: &str = "locale";
pub const KEY_LATLON: &str = "latlon";
pub const KEY_ADDITIONAL_INFO: &str = "additionalInfo";
pub const KEY_ENABLE_RECOMMENDATIONS: &str = "enableRecommendations";
pub const KEY_REMEMBER_WATCHED_PROGRAMS: &str = "rememberWatchedPrograms";
pub const KEY_SHARE_WATCH_HISTORY: &str = "shareWatchHistory";
pub const KEY_USER_GRANT: &str = "userGrantKey";
pub const KEY_ALLOW_ACR_COLLECTION: &str = "allowACRCollection";
pub const KEY_ALLOW_APP_CONTENT_AD_TARGETING: &str = "allowAppContentAdTargetting";
pub const KEY_ALLOW_CAMERA_ANALYTICS: &str = "allowCameraAnalytics";
pub const KEY_ALLOW_PERSONALIZATION: &str = "allowPersonalization";
pub const KEY_ALLOW_PRIMARY_BROWSE_AD_TARGETING: &str = "allowPrimaryBrowseAdTargeting";
pub const KEY_ALLOW_PRIMARY_CONTENT_AD_TARGETING: &str = "allowPrimaryContentAdTargeting";
pub const KEY_ALLOW_PRODUCT_ANALYTICS: &str = "allowProductAnalytics";
pub const KEY_ALLOW_REMOTE_DIAGNOSTICS: &str = "allowRemoteDiagnostics";
pub const KEY_ALLOW_RESUME_POINTS: &str = "allowResumePoints";
pub const KEY_ALLOW_UNENTITLED_PERSONALIZATION: &str = "allowUnentitledPersonalization";
pub const KEY_ALLOW_UNENTITLED_RESUME_POINTS: &str = "allowUnentitledResumePoints";
pub const KEY_ALLOW_WATCH_HISTORY: &str = "allowWatchHistory";
pub const KEY_VOICE_GUIDANCE_SPEED: &str = "speed";
pub const KEY_PARTNER_EXCLUSIONS: &str = "partnerExclusions";
pub const KEY_SKIP_RESTRICTION: &str = "skipRestriction";

pub const EVENT_CLOSED_CAPTIONS_SETTINGS_CHANGED: &str =
    "accessibility.onClosedCaptionsSettingsChanged";
pub const EVENT_CLOSED_CAPTIONS_ENABLED: &str = "closedcaptions.onEnabledChanged";
pub const EVENT_CLOSED_CAPTIONS_FONT_FAMILY: &str = "closedcaptions.onFontFamilyChanged";
pub const EVENT_CLOSED_CAPTIONS_FONT_SIZE: &str = "closedcaptions.onFontSizeChanged";
pub const EVENT_CLOSED_CAPTIONS_FONT_COLOR: &str = "closedcaptions.onFontColorChanged";
pub const EVENT_CLOSED_CAPTIONS_FONT_EDGE: &str = "closedcaptions.onFontEdgeChanged";
pub const EVENT_CLOSED_CAPTIONS_FONT_EDGE_COLOR: &str = "closedcaptions.onFontEdgeColorChanged";
pub const EVENT_CLOSED_CAPTIONS_FONT_OPACITY: &str = "closedcaptions.onFontOpacityChanged";
pub const EVENT_CLOSED_CAPTIONS_BACKGROUND_COLOR: &str = "closedcaptions.onBackgroundColorChanged";
pub const EVENT_CLOSED_CAPTIONS_BACKGROUND_OPACITY: &str =
    "closedcaptions.onBackgroundOpacityChanged";
pub const EVENT_CLOSED_CAPTIONS_TEXT_ALIGN: &str = "closedcaptions.onTextAlignChanged";
pub const EVENT_CLOSED_CAPTIONS_TEXT_ALIGN_VERTICAL: &str =
    "closedcaptions.onTextAlignVerticalChanged";
pub const EVENT_LOCALITY: &str = "localization.onLocalityChanged";
pub const EVENT_COUNTRY_CODE: &str = "localization.onCountryCodeChanged";
pub const EVENT_LANGUAGE: &str = "localization.onLanguageChanged";
pub const EVENT_POSTAL_CODE: &str = "localization.onPostalCodeChanged";
pub const EVENT_LOCALE: &str = "localization.onLocaleChanged";
pub const EVENT_LATLON: &str = "localization.onLatlonChanged";
pub const EVENT_ADDITIONAL_INFO: &str = "localization.onAdditionalInfoChanged";
pub const EVENT_ENABLE_RECOMMENDATIONS: &str = "privacy.onEnableRecommendationsChanged";
pub const EVENT_LIMIT_AD_TRACKING: &str = "privacy.onLimitAdTrackingChanged";
pub const EVENT_REMEMBER_WATCHED_PROGRAMS: &str = "privacy.onRememberWatchedProgramsChanged";
pub const EVENT_SHARE_WATCH_HISTORY: &str = "privacy.onShareWatchHistoryChanged";
pub const EVENT_ALLOW_ACR_COLLECTION_CHANGED: &str = "privacy.onAllowACRCollectionChanged";
pub const EVENT_ALLOW_APP_CONTENT_AD_TARGETING_CHANGED: &str =
    "privacy.onAllowAppContentAdTargetingChanged";
pub const EVENT_ALLOW_CAMERA_ANALYTICS_CHANGED: &str = "privacy.onAllowCameraAnalyticsChanged";
pub const EVENT_ALLOW_PERSONALIZATION_CHANGED: &str = "privacy.onAllowPersonalizationChanged";
pub const EVENT_ALLOW_PRIMARY_BROWSE_AD_TARGETING_CHANGED: &str =
    "privacy.onAllowPrimaryBrowseAdTargetingChanged";
pub const EVENT_ALLOW_PRIMARY_CONTENT_AD_TARGETING_CHANGED: &str =
    "privacy.onAllowPrimaryContentAdTargetingChanged";
pub const EVENT_ALLOW_PRODUCT_ANALYTICS_CHANGED: &str = "privacy.onAllowProductAnalyticsChanged";
pub const EVENT_ALLOW_REMOTE_DIAGNOSTICS_CHANGED: &str = "privacy.onAllowRemoteDiagnosticsChanged";
pub const EVENT_ALLOW_RESUME_POINTS_CHANGED: &str = "privacy.onAllowResumePointsChanged";
pub const EVENT_ALLOW_UNENTITLED_PERSONALIZATION_CHANGED: &str =
    "privacy.onAllowUnentitledPersonalizationChanged";
pub const EVENT_ALLOW_UNENTITLED_RESUME_POINTS_CHANGED: &str =
    "privacy.onAllowUnentitledResumePointsChanged";
pub const EVENT_ALLOW_WATCH_HISTORY_CHANGED: &str = "privacy.onAllowWatchHistoryChanged";
pub const EVENT_DEVICE_NAME_CHANGED: &str = "device.onNameChanged";
pub const EVENT_DEVICE_DEVICE_NAME_CHANGED: &str = "device.onDeviceNameChanged";
pub const EVENT_SECOND_SCREEN_FRIENDLY_NAME_CHANGED: &str = "secondscreen.onFriendlyNameChanged";
pub const EVENT_ADVERTISING_POLICY_CHANGED: &str = "advertising.onPolicyChanged";
pub const EVENT_ADVERTISING_SKIP_RESTRICTION: &str = "advertising.setSkipRestriction";

pub const EVENT_TIMEZONE_CHANGED: &str = "localization.onTimeZoneChanged";

const PROPERTY_DATA_CLOSED_CAPTIONS_ENABLED: PropertyData = PropertyData {
    key: KEY_ENABLED,
    namespace: NAMESPACE_CLOSED_CAPTIONS,
    event_names: Some(&[
        EVENT_CLOSED_CAPTIONS_ENABLED,
        EVENT_CLOSED_CAPTIONS_SETTINGS_CHANGED,
    ]),
};

const PROPERTY_DATA_CLOSED_CAPTIONS_FONT_FAMILY: PropertyData = PropertyData {
    key: KEY_FONT_FAMILY,
    namespace: NAMESPACE_CLOSED_CAPTIONS,
    event_names: Some(&[
        EVENT_CLOSED_CAPTIONS_FONT_FAMILY,
        EVENT_CLOSED_CAPTIONS_SETTINGS_CHANGED,
    ]),
};

const PROPERTY_DATA_CLOSED_CAPTIONS_FONT_SIZE: PropertyData = PropertyData {
    key: KEY_FONT_SIZE,
    namespace: NAMESPACE_CLOSED_CAPTIONS,
    event_names: Some(&[
        EVENT_CLOSED_CAPTIONS_FONT_SIZE,
        EVENT_CLOSED_CAPTIONS_SETTINGS_CHANGED,
    ]),
};

const PROPERTY_DATA_CLOSED_CAPTIONS_FONT_COLOR: PropertyData = PropertyData {
    key: KEY_FONT_COLOR,
    namespace: NAMESPACE_CLOSED_CAPTIONS,
    event_names: Some(&[
        EVENT_CLOSED_CAPTIONS_FONT_COLOR,
        EVENT_CLOSED_CAPTIONS_SETTINGS_CHANGED,
    ]),
};

const PROPERTY_DATA_CLOSED_CAPTIONS_FONT_EDGE: PropertyData = PropertyData {
    key: KEY_FONT_EDGE,
    namespace: NAMESPACE_CLOSED_CAPTIONS,
    event_names: Some(&[
        EVENT_CLOSED_CAPTIONS_FONT_EDGE,
        EVENT_CLOSED_CAPTIONS_SETTINGS_CHANGED,
    ]),
};

const PROPERTY_DATA_CLOSED_CAPTIONS_FONT_EDGE_COLOR: PropertyData = PropertyData {
    key: KEY_FONT_EDGE_COLOR,
    namespace: NAMESPACE_CLOSED_CAPTIONS,
    event_names: Some(&[
        EVENT_CLOSED_CAPTIONS_FONT_EDGE_COLOR,
        EVENT_CLOSED_CAPTIONS_SETTINGS_CHANGED,
    ]),
};

const PROPERTY_DATA_CLOSED_CAPTIONS_FONT_OPACITY: PropertyData = PropertyData {
    key: KEY_FONT_OPACITY,
    namespace: NAMESPACE_CLOSED_CAPTIONS,
    event_names: Some(&[
        EVENT_CLOSED_CAPTIONS_FONT_OPACITY,
        EVENT_CLOSED_CAPTIONS_SETTINGS_CHANGED,
    ]),
};

const PROPERTY_DATA_CLOSED_CAPTIONS_BACKGROUND_COLOR: PropertyData = PropertyData {
    key: KEY_BACKGROUND_COLOR,
    namespace: NAMESPACE_CLOSED_CAPTIONS,
    event_names: Some(&[
        EVENT_CLOSED_CAPTIONS_BACKGROUND_COLOR,
        EVENT_CLOSED_CAPTIONS_SETTINGS_CHANGED,
    ]),
};

const PROPERTY_DATA_CLOSED_CAPTIONS_BACKGROUND_OPACITY: PropertyData = PropertyData {
    key: KEY_BACKGROUND_OPACITY,
    namespace: NAMESPACE_CLOSED_CAPTIONS,
    event_names: Some(&[
        EVENT_CLOSED_CAPTIONS_BACKGROUND_OPACITY,
        EVENT_CLOSED_CAPTIONS_SETTINGS_CHANGED,
    ]),
};

const PROPERTY_DATA_CLOSED_CAPTIONS_TEXT_ALIGN: PropertyData = PropertyData {
    key: KEY_TEXT_ALIGN,
    namespace: NAMESPACE_CLOSED_CAPTIONS,
    event_names: Some(&[
        EVENT_CLOSED_CAPTIONS_TEXT_ALIGN,
        EVENT_CLOSED_CAPTIONS_SETTINGS_CHANGED,
    ]),
};

const PROPERTY_DATA_CLOSED_CAPTIONS_TEXT_ALIGN_VERTICAL: PropertyData = PropertyData {
    key: KEY_TEXT_ALIGN_VERTICAL,
    namespace: NAMESPACE_CLOSED_CAPTIONS,
    event_names: Some(&[
        EVENT_CLOSED_CAPTIONS_TEXT_ALIGN_VERTICAL,
        EVENT_CLOSED_CAPTIONS_SETTINGS_CHANGED,
    ]),
};

const PROPERTY_DATA_LOCALITY: PropertyData = PropertyData {
    key: KEY_LOCALITY,
    namespace: NAMESPACE_LOCALIZATION,
    event_names: Some(&[EVENT_LOCALITY]),
};

const PROPERTY_DATA_COUNTRY_CODE: PropertyData = PropertyData {
    key: KEY_COUNTRY_CODE,
    namespace: NAMESPACE_LOCALIZATION,
    event_names: Some(&[EVENT_COUNTRY_CODE]),
};

const PROPERTY_DATA_LANGUAGE: PropertyData = PropertyData {
    key: KEY_LANGUAGE,
    namespace: NAMESPACE_LOCALIZATION,
    event_names: Some(&[EVENT_LANGUAGE]),
};

const PROPERTY_DATA_POSTAL_CODE: PropertyData = PropertyData {
    key: KEY_POSTAL_CODE,
    namespace: NAMESPACE_LOCALIZATION,
    event_names: Some(&[EVENT_POSTAL_CODE]),
};

const PROPERTY_DATA_LOCALE: PropertyData = PropertyData {
    key: KEY_LOCALE,
    namespace: NAMESPACE_LOCALIZATION,
    event_names: Some(&[EVENT_LOCALE]),
};

const PROPERTY_DATA_LATLON: PropertyData = PropertyData {
    key: KEY_LATLON,
    namespace: NAMESPACE_LOCALIZATION,
    event_names: Some(&[EVENT_LATLON]),
};

const PROPERTY_DATA_ADDITIONAL_INFO: PropertyData = PropertyData {
    key: KEY_ADDITIONAL_INFO,
    namespace: NAMESPACE_LOCALIZATION,
    event_names: Some(&[EVENT_ADDITIONAL_INFO]),
};

const PROPERTY_DATA_ENABLE_RECOMMENDATIONS: PropertyData = PropertyData {
    key: KEY_ENABLE_RECOMMENDATIONS,
    namespace: NAMESPACE_PRIVACY,
    event_names: Some(&[EVENT_ENABLE_RECOMMENDATIONS]),
};

const PROPERTY_DATA_LIMIT_AD_TRACKING: PropertyData = PropertyData {
    key: KEY_LIMIT_AD_TRACKING,
    namespace: NAMESPACE_PRIVACY,
    event_names: Some(&[EVENT_LIMIT_AD_TRACKING, EVENT_ADVERTISING_POLICY_CHANGED]),
};

const PROPERTY_DATA_REMEMBER_WATCHED_PROGRAMS: PropertyData = PropertyData {
    key: KEY_REMEMBER_WATCHED_PROGRAMS,
    namespace: NAMESPACE_PRIVACY,
    event_names: Some(&[EVENT_REMEMBER_WATCHED_PROGRAMS]),
};

const PROPERTY_DATA_SHARE_WATCH_HISTORY: PropertyData = PropertyData {
    key: KEY_SHARE_WATCH_HISTORY,
    namespace: NAMESPACE_PRIVACY,
    event_names: Some(&[EVENT_SHARE_WATCH_HISTORY]),
};

const PROPERTY_DATA_DEVICE_NAME: PropertyData = PropertyData {
    key: KEY_NAME,
    namespace: NAMESPACE_DEVICE_NAME,
    event_names: Some(&[
        EVENT_DEVICE_NAME_CHANGED,
        EVENT_DEVICE_DEVICE_NAME_CHANGED,
        EVENT_SECOND_SCREEN_FRIENDLY_NAME_CHANGED,
    ]),
};
const PROPERTY_DATA_USER_GRANT: PropertyData = PropertyData {
    key: KEY_USER_GRANT,
    namespace: NAMESPACE_USER_GRANT,
    event_names: None,
};

const PROPERTY_DATA_ALLOW_ACR_COLLECTION: PropertyData = PropertyData {
    key: KEY_ALLOW_ACR_COLLECTION,
    namespace: NAMESPACE_PRIVACY,
    event_names: Some(&[EVENT_ALLOW_ACR_COLLECTION_CHANGED]),
};

const PROPERTY_DATA_ALLOW_APP_CONTENT_AD_TARGETING: PropertyData = PropertyData {
    key: KEY_ALLOW_APP_CONTENT_AD_TARGETING,
    namespace: NAMESPACE_PRIVACY,
    event_names: Some(&[EVENT_ALLOW_APP_CONTENT_AD_TARGETING_CHANGED]),
};

const PROPERTY_DATA_ALLOW_CAMERA_ANALYTICS: PropertyData = PropertyData {
    key: KEY_ALLOW_CAMERA_ANALYTICS,
    namespace: NAMESPACE_PRIVACY,
    event_names: Some(&[EVENT_ALLOW_CAMERA_ANALYTICS_CHANGED]),
};

const PROPERTY_DATA_ALLOW_PERSONALIZATION: PropertyData = PropertyData {
    key: KEY_ALLOW_PERSONALIZATION,
    namespace: NAMESPACE_PRIVACY,
    event_names: Some(&[EVENT_ALLOW_PERSONALIZATION_CHANGED]),
};

const PROPERTY_DATA_ALLOW_PRIMARY_BROWSE_AD_TARGETING: PropertyData = PropertyData {
    key: KEY_ALLOW_PRIMARY_BROWSE_AD_TARGETING,
    namespace: NAMESPACE_PRIVACY,
    event_names: Some(&[EVENT_ALLOW_PRIMARY_BROWSE_AD_TARGETING_CHANGED]),
};

const PROPERTY_DATA_ALLOW_PRIMARY_CONTENT_AD_TARGETING: PropertyData = PropertyData {
    key: KEY_ALLOW_PRIMARY_CONTENT_AD_TARGETING,
    namespace: NAMESPACE_PRIVACY,
    event_names: Some(&[EVENT_ALLOW_PRIMARY_CONTENT_AD_TARGETING_CHANGED]),
};

const PROPERTY_DATA_ALLOW_PRODUCT_ANALYTICS: PropertyData = PropertyData {
    key: KEY_ALLOW_PRODUCT_ANALYTICS,
    namespace: NAMESPACE_PRIVACY,
    event_names: Some(&[EVENT_ALLOW_PRODUCT_ANALYTICS_CHANGED]),
};

const PROPERTY_DATA_ALLOW_REMOTE_DIAGNOSTICS: PropertyData = PropertyData {
    key: KEY_ALLOW_REMOTE_DIAGNOSTICS,
    namespace: NAMESPACE_PRIVACY,
    event_names: Some(&[EVENT_ALLOW_REMOTE_DIAGNOSTICS_CHANGED]),
};

const PROPERTY_DATA_ALLOW_RESUME_POINTS: PropertyData = PropertyData {
    key: KEY_ALLOW_RESUME_POINTS,
    namespace: NAMESPACE_PRIVACY,
    event_names: Some(&[EVENT_ALLOW_RESUME_POINTS_CHANGED]),
};

const PROPERTY_DATA_ALLOW_UNENTITLED_PERSONALIZATION: PropertyData = PropertyData {
    key: KEY_ALLOW_UNENTITLED_PERSONALIZATION,
    namespace: NAMESPACE_PRIVACY,
    event_names: Some(&[EVENT_ALLOW_UNENTITLED_PERSONALIZATION_CHANGED]),
};

const PROPERTY_DATA_ALLOW_UNENTITLED_RESUME_POINTS: PropertyData = PropertyData {
    key: KEY_ALLOW_UNENTITLED_RESUME_POINTS,
    namespace: NAMESPACE_PRIVACY,
    event_names: Some(&[EVENT_ALLOW_UNENTITLED_RESUME_POINTS_CHANGED]),
};

const PROPERTY_DATA_ALLOW_WATCH_HISTORY: PropertyData = PropertyData {
    key: KEY_ALLOW_WATCH_HISTORY,
    namespace: NAMESPACE_PRIVACY,
    event_names: Some(&[EVENT_ALLOW_WATCH_HISTORY_CHANGED]),
};

const PROPERTY_DATA_PARTNER_EXCLUSIONS: PropertyData = PropertyData {
    key: KEY_PARTNER_EXCLUSIONS,
    namespace: NAMESPACE_PRIVACY,
    event_names: None,
};

const PROPERTY_DATA_SKIP_RESTRICTION: PropertyData = PropertyData {
    key: KEY_SKIP_RESTRICTION,
    namespace: NAMESPACE_ADVERTISING,
    event_names: Some(&[EVENT_ADVERTISING_SKIP_RESTRICTION]),
};

#[derive(Debug)]
pub struct PropertyData {
    pub namespace: &'static str,
    pub key: &'static str,
    pub event_names: Option<&'static [&'static str]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageProperty {
    ClosedCaptionsEnabled,
    ClosedCaptionsFontFamily,
    ClosedCaptionsFontSize,
    ClosedCaptionsFontColor,
    ClosedCaptionsFontEdge,
    ClosedCaptionsFontEdgeColor,
    ClosedCaptionsFontOpacity,
    ClosedCaptionsBackgroundColor,
    ClosedCaptionsBackgroundOpacity,
    ClosedCaptionsTextAlign,
    ClosedCaptionsTextAlignVertical,
    Locality,
    CountryCode,
    Language,
    PostalCode,
    Locale,
    LatLon,
    AdditionalInfo,
    EnableRecommendations,
    LimitAdTracking,
    RemeberWatchedPrograms,
    ShareWatchHistory,
    DeviceName,
    UserGrants,
    AllowAcrCollection,
    AllowAppContentAdTargeting,
    AllowCameraAnalytics,
    AllowPersonalization,
    AllowPrimaryBrowseAdTargeting,
    AllowPrimaryContentAdTargeting,
    AllowProductAnalytics,
    AllowRemoteDiagnostics,
    AllowResumePoints,
    AllowUnentitledPersonalization,
    AllowUnentitledResumePoints,
    AllowWatchHistory,
    PartnerExclusions,
    SkipRestriction,
}

impl StorageProperty {
    pub fn as_data(&self) -> PropertyData {
        match self {
            StorageProperty::ClosedCaptionsEnabled => PROPERTY_DATA_CLOSED_CAPTIONS_ENABLED,
            StorageProperty::ClosedCaptionsFontFamily => PROPERTY_DATA_CLOSED_CAPTIONS_FONT_FAMILY,
            StorageProperty::ClosedCaptionsFontSize => PROPERTY_DATA_CLOSED_CAPTIONS_FONT_SIZE,
            StorageProperty::ClosedCaptionsFontColor => PROPERTY_DATA_CLOSED_CAPTIONS_FONT_COLOR,
            StorageProperty::ClosedCaptionsFontEdge => PROPERTY_DATA_CLOSED_CAPTIONS_FONT_EDGE,
            StorageProperty::ClosedCaptionsFontEdgeColor => {
                PROPERTY_DATA_CLOSED_CAPTIONS_FONT_EDGE_COLOR
            }
            StorageProperty::ClosedCaptionsFontOpacity => {
                PROPERTY_DATA_CLOSED_CAPTIONS_FONT_OPACITY
            }
            StorageProperty::ClosedCaptionsBackgroundColor => {
                PROPERTY_DATA_CLOSED_CAPTIONS_BACKGROUND_COLOR
            }
            StorageProperty::ClosedCaptionsBackgroundOpacity => {
                PROPERTY_DATA_CLOSED_CAPTIONS_BACKGROUND_OPACITY
            }
            StorageProperty::ClosedCaptionsTextAlign => PROPERTY_DATA_CLOSED_CAPTIONS_TEXT_ALIGN,
            StorageProperty::ClosedCaptionsTextAlignVertical => {
                PROPERTY_DATA_CLOSED_CAPTIONS_TEXT_ALIGN_VERTICAL
            }
            StorageProperty::Locality => PROPERTY_DATA_LOCALITY,
            StorageProperty::CountryCode => PROPERTY_DATA_COUNTRY_CODE,
            StorageProperty::Language => PROPERTY_DATA_LANGUAGE,
            StorageProperty::PostalCode => PROPERTY_DATA_POSTAL_CODE,
            StorageProperty::Locale => PROPERTY_DATA_LOCALE,
            StorageProperty::LatLon => PROPERTY_DATA_LATLON,
            StorageProperty::AdditionalInfo => PROPERTY_DATA_ADDITIONAL_INFO,
            StorageProperty::EnableRecommendations => PROPERTY_DATA_ENABLE_RECOMMENDATIONS,
            StorageProperty::LimitAdTracking => PROPERTY_DATA_LIMIT_AD_TRACKING,
            StorageProperty::RemeberWatchedPrograms => PROPERTY_DATA_REMEMBER_WATCHED_PROGRAMS,
            StorageProperty::ShareWatchHistory => PROPERTY_DATA_SHARE_WATCH_HISTORY,
            StorageProperty::DeviceName => PROPERTY_DATA_DEVICE_NAME,
            StorageProperty::UserGrants => PROPERTY_DATA_USER_GRANT,
            StorageProperty::AllowAcrCollection => PROPERTY_DATA_ALLOW_ACR_COLLECTION,
            StorageProperty::AllowAppContentAdTargeting => {
                PROPERTY_DATA_ALLOW_APP_CONTENT_AD_TARGETING
            }
            StorageProperty::AllowCameraAnalytics => PROPERTY_DATA_ALLOW_CAMERA_ANALYTICS,
            StorageProperty::AllowPersonalization => PROPERTY_DATA_ALLOW_PERSONALIZATION,
            StorageProperty::AllowPrimaryBrowseAdTargeting => {
                PROPERTY_DATA_ALLOW_PRIMARY_BROWSE_AD_TARGETING
            }
            StorageProperty::AllowPrimaryContentAdTargeting => {
                PROPERTY_DATA_ALLOW_PRIMARY_CONTENT_AD_TARGETING
            }
            StorageProperty::AllowProductAnalytics => PROPERTY_DATA_ALLOW_PRODUCT_ANALYTICS,
            StorageProperty::AllowRemoteDiagnostics => PROPERTY_DATA_ALLOW_REMOTE_DIAGNOSTICS,
            StorageProperty::AllowResumePoints => PROPERTY_DATA_ALLOW_RESUME_POINTS,
            StorageProperty::AllowUnentitledPersonalization => {
                PROPERTY_DATA_ALLOW_UNENTITLED_PERSONALIZATION
            }
            StorageProperty::AllowUnentitledResumePoints => {
                PROPERTY_DATA_ALLOW_UNENTITLED_RESUME_POINTS
            }
            StorageProperty::AllowWatchHistory => PROPERTY_DATA_ALLOW_WATCH_HISTORY,
            StorageProperty::PartnerExclusions => PROPERTY_DATA_PARTNER_EXCLUSIONS,
            StorageProperty::SkipRestriction => PROPERTY_DATA_SKIP_RESTRICTION,
        }
    }

    pub fn as_privacy_setting(&self) -> Option<PrivacySetting> {
        match self {
            StorageProperty::AllowAcrCollection => Some(PrivacySetting::Acr),
            StorageProperty::AllowAppContentAdTargeting => {
                Some(PrivacySetting::AppContentAdTargeting)
            }
            StorageProperty::AllowCameraAnalytics => Some(PrivacySetting::CameraAnalytics),
            StorageProperty::AllowPersonalization => Some(PrivacySetting::Personalization),
            StorageProperty::AllowPrimaryBrowseAdTargeting => {
                Some(PrivacySetting::PrimaryBrowseAdTargeting)
            }
            StorageProperty::AllowPrimaryContentAdTargeting => {
                Some(PrivacySetting::PrimaryContentAdTargeting)
            }
            StorageProperty::AllowProductAnalytics => Some(PrivacySetting::ProductAnalytics),
            StorageProperty::AllowRemoteDiagnostics => Some(PrivacySetting::RemoteDiagnostics),
            StorageProperty::AllowResumePoints => Some(PrivacySetting::ContinueWatching),
            StorageProperty::AllowUnentitledPersonalization => {
                Some(PrivacySetting::UnentitledPersonalization)
            }
            StorageProperty::AllowUnentitledResumePoints => {
                Some(PrivacySetting::UnentitledContinueWatching)
            }
            StorageProperty::AllowWatchHistory => Some(PrivacySetting::WatchHistory),
            _ => None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum StorageManagerRequest {
    GetBool(StorageProperty, bool),
    GetString(StorageProperty),
}

impl ExtnPayloadProvider for StorageManagerRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::StorageManager(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<StorageManagerRequest> {
        if let ExtnPayload::Request(ExtnRequest::StorageManager(r)) = payload {
            return Some(r);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::Storage(StorageAdjective::Manager)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StorageAdjective {
    PrivacyCloud,
    PrivacyLocal,
    UsergrantCloud,
    UsergrantLocal,
    Local,
    Manager,
    Secure,
}

impl ContractAdjective for StorageAdjective {
    fn get_contract(&self) -> RippleContract {
        RippleContract::Storage(self.clone())
    }
}
