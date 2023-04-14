use dpab::core::model::privacy::PrivacySetting;
use serde::{Deserialize, Serialize};

pub const NAMESPACE_CLOSED_CAPTIONS: &'static str = "ClosedCaptions";
<<<<<<< HEAD
pub const NAMESPACE_PRIVACY: &'static str = "Privacy";
pub const NAMESPACE_DEVICE_NAME: &'static str = "DeviceName";
pub const NAMESPACE_LOCALIZATION: &'static str = "Localization";
pub const NAMESPACE_USER_GRANT: &'static str = "UserGrant";
=======
pub const NAMESPACE_VOICE_GUIDANCE: &'static str = "Voiceguidance";
>>>>>>> 8c2e11b (voice guidance added.)

pub const KEY_ENABLED: &'static str = "enabled";
pub const KEY_FONT_FAMILY: &'static str = "fontFamily";
pub const KEY_FONT_SIZE: &'static str = "fontSize";
pub const KEY_FONT_COLOR: &'static str = "fontColor";
pub const KEY_FONT_EDGE: &'static str = "fontEdge";
pub const KEY_FONT_EDGE_COLOR: &'static str = "fontEdgeColor";
pub const KEY_FONT_OPACITY: &'static str = "fontOpacity";
pub const KEY_BACKGROUND_COLOR: &'static str = "backgroundColor";
pub const KEY_BACKGROUND_OPACITY: &'static str = "backgroundOpacity";
pub const KEY_TEXT_ALIGN: &'static str = "textAlign";
pub const KEY_TEXT_ALIGN_VERTICAL: &'static str = "textAlignVertical";
<<<<<<< HEAD
pub const KEY_LIMIT_AD_TRACKING: &'static str = "limitAdTracking";
pub const KEY_NAME: &'static str = "name";
pub const KEY_POSTAL_CODE: &'static str = "postalCode";
pub const KEY_LOCALITY: &'static str = "locality";
pub const KEY_COUNTRY_CODE: &'static str = "countryCode";
pub const KEY_LANGUAGE: &'static str = "language";
pub const KEY_LOCALE: &'static str = "locale";
pub const KEY_LATLON: &'static str = "latlon";
pub const KEY_ADDITIONAL_INFO: &'static str = "additionalInfo";
pub const KEY_ENABLE_RECOMMENDATIONS: &'static str = "enableRecommendations";
pub const KEY_REMEMBER_WATCHED_PROGRAMS: &'static str = "rememberWatchedPrograms";
pub const KEY_SHARE_WATCH_HISTORY: &'static str = "shareWatchHistory";
pub const KEY_USER_GRANT: &'static str = "userGrantKey";
pub const KEY_ALLOW_ACR_COLLECTION: &'static str = "allowACRCollection";
pub const KEY_ALLOW_APP_CONTENT_AD_TARGETING: &'static str = "allowAppContentAdTargetting";
pub const KEY_ALLOW_CAMERA_ANALYTICS: &'static str = "allowCameraAnalytics";
pub const KEY_ALLOW_PERSONALIZATION: &'static str = "allowPersonalization";
pub const KEY_ALLOW_PRIMARY_BROWSE_AD_TARGETING: &'static str = "allowPrimaryBrowseAdTargeting";
pub const KEY_ALLOW_PRIMARY_CONTENT_AD_TARGETING: &'static str = "allowPrimaryContentAdTargeting";
pub const KEY_ALLOW_PRODUCT_ANALYTICS: &'static str = "allowProductAnalytics";
pub const KEY_ALLOW_REMOTE_DIAGNOSTICS: &'static str = "allowRemoteDiagnostics";
pub const KEY_ALLOW_RESUME_POINTS: &'static str = "allowResumePoints";
pub const KEY_ALLOW_UNENTITLED_PERSONALIZATION: &'static str = "allowUnentitledPersonalization";
pub const KEY_ALLOW_UNENTITLED_RESUME_POINTS: &'static str = "allowUnentitledResumePoints";
pub const KEY_ALLOW_WATCH_HISTORY: &'static str = "allowWatchHistory";
=======
pub const KEY_VOICE_GUIDANCE_SPEED: &'static str = "speed";
>>>>>>> 3e29104 (voice guidance bug.)

pub const EVENT_CLOSED_CAPTIONS_SETTINGS_CHANGED: &'static str =
    "accessibility.onClosedCaptionsSettingsChanged";
pub const EVENT_CLOSED_CAPTIONS_ENABLED: &'static str = "closedcaptions.onEnabledChanged";
pub const EVENT_CLOSED_CAPTIONS_FONT_FAMILY: &'static str = "closedcaptions.onFontFamilyChanged";
pub const EVENT_CLOSED_CAPTIONS_FONT_SIZE: &'static str = "closedcaptions.onFontSizeChanged";
pub const EVENT_CLOSED_CAPTIONS_FONT_COLOR: &'static str = "closedcaptions.onFontColorChanged";
pub const EVENT_CLOSED_CAPTIONS_FONT_EDGE: &'static str = "closedcaptions.onFontEdgeChanged";
pub const EVENT_CLOSED_CAPTIONS_FONT_EDGE_COLOR: &'static str =
    "closedcaptions.onFontEdgeColorChanged";
pub const EVENT_CLOSED_CAPTIONS_FONT_OPACITY: &'static str = "closedcaptions.onFontOpacityChanged";
pub const EVENT_CLOSED_CAPTIONS_BACKGROUND_COLOR: &'static str =
    "closedcaptions.onBackgroundColorChanged";
pub const EVENT_CLOSED_CAPTIONS_BACKGROUND_OPACITY: &'static str =
    "closedcaptions.onBackgroundOpacityChanged";
pub const EVENT_CLOSED_CAPTIONS_TEXT_ALIGN: &'static str = "closedcaptions.onTextAlignChanged";
pub const EVENT_CLOSED_CAPTIONS_TEXT_ALIGN_VERTICAL: &'static str =
    "closedcaptions.onTextAlignVerticalChanged";
pub const EVENT_LOCALITY: &'static str = "localization.onLocalityChanged";
pub const EVENT_COUNTRY_CODE: &'static str = "localization.onCountryCodeChanged";
pub const EVENT_LANGUAGE: &'static str = "localization.onLanguageChanged";
pub const EVENT_POSTAL_CODE: &'static str = "localization.onPostalCodeChanged";
pub const EVENT_LOCALE: &'static str = "localization.onLocaleChanged";
pub const EVENT_LATLON: &'static str = "localization.onLatlonChanged";
pub const EVENT_ADDITIONAL_INFO: &'static str = "localization.onAdditionalInfoChanged";
pub const EVENT_ENABLE_RECOMMENDATIONS: &'static str = "privacy.onEnableRecommendationsChanged";
pub const EVENT_LIMIT_AD_TRACKING: &'static str = "privacy.onLimitAdTrackingChanged";
pub const EVENT_REMEMBER_WATCHED_PROGRAMS: &'static str =
    "privacy.onRememberWatchedProgramsChanged";
pub const EVENT_SHARE_WATCH_HISTORY: &'static str = "privacy.onShareWatchHistoryChanged";
pub const EVENT_ALLOW_ACR_COLLECTION_CHANGED: &'static str = "privacy.onAllowACRCollectionChanged";
pub const EVENT_ALLOW_APP_CONTENT_AD_TARGETING_CHANGED: &'static str =
    "privacy.onAllowAppContentAdTargetingChanged";
pub const EVENT_ALLOW_CAMERA_ANALYTICS_CHANGED: &'static str =
    "privacy.onAllowCameraAnalyticsChanged";
pub const EVENT_ALLOW_PERSONALIZATION_CHANGED: &'static str =
    "privacy.onAllowPersonalizationChanged";
pub const EVENT_ALLOW_PRIMARY_BROWSE_AD_TARGETING_CHANGED: &'static str =
    "privacy.onAllowPrimaryBrowseAdTargetingChanged";
pub const EVENT_ALLOW_PRIMARY_CONTENT_AD_TARGETING_CHANGED: &'static str =
    "privacy.onAllowPrimaryContentAdTargetingChanged";
pub const EVENT_ALLOW_PRODUCT_ANALYTICS_CHANGED: &'static str =
    "privacy.onAllowProductAnalyticsChanged";
pub const EVENT_ALLOW_REMOTE_DIAGNOSTICS_CHANGED: &'static str =
    "privacy.onAllowRemoteDiagnosticsChanged";
pub const EVENT_ALLOW_RESUME_POINTS_CHANGED: &'static str = "privacy.onAllowResumePointsChanged";
pub const EVENT_ALLOW_UNENTITLED_PERSONALIZATION_CHANGED: &'static str =
    "privacy.onAllowUnentitledPersonalizationChanged";
pub const EVENT_ALLOW_UNENTITLED_RESUME_POINTS_CHANGED: &'static str =
    "privacy.onAllowUnentitledResumePointsChanged";
pub const EVENT_ALLOW_WATCH_HISTORY_CHANGED: &'static str = "privacy.onAllowWatchHistoryChanged";
pub const EVENT_DEVICE_NAME_CHANGED: &'static str = "device.onNameChanged";
pub const EVENT_DEVICE_DEVICE_NAME_CHANGED: &'static str = "device.onDeviceNameChanged";
pub const EVENT_SECOND_SCREEN_FRIENDLY_NAME_CHANGED: &'static str =
    "secondscreen.onFriendlyNameChanged";
pub const EVENT_ADVERTISING_POLICY_CHANGED: &'static str = "advertising.onPolicyChanged";

pub const EVENT_VOICE_GUIDANCE_SETTINGS_CHANGED: &'static str =
    "accessibility.onVoiceGuidanceSettingsChanged";
pub const EVENT_VOICE_GUIDANCE_ENABLED_CHANGED: &'static str = "voiceguidance.onEnabledChanged";
pub const EVENT_VOICE_GUIDANCE_SPEED_CHANGED: &'static str = "voiceguidance.onSpeedChanged";



const PROPERTY_DATA_VOICE_GUIDANCE_ENABLED: PropertyData = PropertyData {
    key: KEY_ENABLED,
    namespace: NAMESPACE_VOICE_GUIDANCE,
    event_names: Some(&[
        EVENT_VOICE_GUIDANCE_ENABLED_CHANGED,
        EVENT_VOICE_GUIDANCE_SETTINGS_CHANGED,
    ]),
};

const PROPERTY_DATA_VOICE_GUIDANCE_SPEED: PropertyData = PropertyData {
    key: KEY_VOICE_GUIDANCE_SPEED,
    namespace: NAMESPACE_VOICE_GUIDANCE,
    event_names: Some(&[
        EVENT_VOICE_GUIDANCE_ENABLED_CHANGED,
        EVENT_VOICE_GUIDANCE_SPEED_CHANGED,
    ]),
};



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
<<<<<<< HEAD
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
=======
    VoiceguidanceEnabled,
    VoiceguidanceSpeed,
>>>>>>> 3e29104 (voice guidance bug.)
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
<<<<<<< HEAD
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
=======
            },
            StorageProperty::VoiceguidanceEnabled => PROPERTY_DATA_VOICE_GUIDANCE_ENABLED,
            StorageProperty::VoiceguidanceSpeed => PROPERTY_DATA_VOICE_GUIDANCE_SPEED,
>>>>>>> 3e29104 (voice guidance bug.)
        }
    }
}


// pub enum VoiceguidanceProperty {
//     VoiceguidanceEnabled,
//     VoiceguidanceSpeed,
// }

// impl VoiceguidanceProperty {
//     pub fn as_data(&self) -> PropertyData {
//         match self {
//             VoiceguidanceProperty::VoiceguidanceEnabled => PROPERTY_DATA_CLOSED_CAPTIONS_ENABLED,
//             VoiceguidanceProperty::VoiceguidanceSpeed => PROPERTY_DATA_CLOSED_CAPTIONS_FONT_FAMILY,
//         }
//     }
// }
