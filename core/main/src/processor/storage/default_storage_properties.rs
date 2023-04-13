use std::collections::HashMap;

use tracing::debug;

use crate::{
    managers::storage::storage_property::{
        KEY_ADDITIONAL_INFO, KEY_ALLOW_ACR_COLLECTION, KEY_ALLOW_APP_CONTENT_AD_TARGETING,
        KEY_ALLOW_CAMERA_ANALYTICS, KEY_ALLOW_PERSONALIZATION,
        KEY_ALLOW_PRIMARY_BROWSE_AD_TARGETING, KEY_ALLOW_PRIMARY_CONTENT_AD_TARGETING,
        KEY_ALLOW_PRODUCT_ANALYTICS, KEY_ALLOW_REMOTE_DIAGNOSTICS, KEY_ALLOW_RESUME_POINTS,
        KEY_ALLOW_UNENTITLED_PERSONALIZATION, KEY_ALLOW_UNENTITLED_RESUME_POINTS,
        KEY_ALLOW_WATCH_HISTORY, KEY_COUNTRY_CODE, KEY_ENABLE_RECOMMENDATIONS, KEY_FONT_COLOR,
        KEY_FONT_EDGE, KEY_FONT_EDGE_COLOR, KEY_FONT_SIZE, KEY_LANGUAGE, KEY_LIMIT_AD_TRACKING,
        KEY_LOCALE, KEY_NAME, KEY_REMEMBER_WATCHED_PROGRAMS, KEY_SHARE_WATCH_HISTORY,
        KEY_TEXT_ALIGN_VERTICAL,
    },
    platform_state::PlatformState,
};

use super::storage_property::{
    KEY_BACKGROUND_COLOR, KEY_BACKGROUND_OPACITY, KEY_ENABLED, KEY_FONT_FAMILY, KEY_FONT_OPACITY,
    KEY_TEXT_ALIGN, NAMESPACE_CLOSED_CAPTIONS, NAMESPACE_DEVICE_NAME, NAMESPACE_LOCALIZATION,
    NAMESPACE_PRIVACY,
};

#[derive(Clone, Debug)]
pub struct DefaultStorageProperties;

impl DefaultStorageProperties {
    pub fn get_bool(
        state: &PlatformState,
        namespace: &String,
        key: &'static str,
    ) -> Result<bool, ()> {
        debug!("get_bool: namespace={}, key={}", namespace, key);
        if namespace.eq(NAMESPACE_CLOSED_CAPTIONS) {
            match key {
                KEY_ENABLED => Ok(state
                    .services
                    .cm
                    .clone()
                    .get_default_values()
                    .captions
                    .enabled),
                _ => Err(()),
            }
        } else if namespace.eq(NAMESPACE_PRIVACY) {
            match key {
                KEY_LIMIT_AD_TRACKING => Ok(true),
                KEY_ENABLE_RECOMMENDATIONS => Ok(false),
                KEY_REMEMBER_WATCHED_PROGRAMS => Ok(false),
                KEY_SHARE_WATCH_HISTORY => Ok(false),
                KEY_ALLOW_ACR_COLLECTION => Ok(false),
                KEY_ALLOW_APP_CONTENT_AD_TARGETING => Ok(false),
                KEY_ALLOW_CAMERA_ANALYTICS => Ok(false),
                KEY_ALLOW_PERSONALIZATION => Ok(false),
                KEY_ALLOW_PRIMARY_BROWSE_AD_TARGETING => Ok(false),
                KEY_ALLOW_PRIMARY_CONTENT_AD_TARGETING => Ok(false),
                KEY_ALLOW_PRODUCT_ANALYTICS => Ok(false),
                KEY_ALLOW_REMOTE_DIAGNOSTICS => Ok(false),
                KEY_ALLOW_RESUME_POINTS => Ok(false),
                KEY_ALLOW_UNENTITLED_PERSONALIZATION => Ok(false),
                KEY_ALLOW_UNENTITLED_RESUME_POINTS => Ok(false),
                KEY_ALLOW_WATCH_HISTORY => Ok(false),
                _ => Err(()),
            }
        } else {
            Err(())
        }
    }
    pub fn get_string(
        state: &PlatformState,
        namespace: &String,
        key: &'static str,
    ) -> Result<String, ()> {
        debug!("get_string: namespace={}, key={}", namespace, key);
        if namespace.eq(NAMESPACE_CLOSED_CAPTIONS) {
            match key {
                KEY_FONT_FAMILY => Ok(state
                    .services
                    .cm
                    .clone()
                    .get_default_values()
                    .captions
                    .font_family),
                KEY_FONT_COLOR => Ok(state
                    .services
                    .cm
                    .clone()
                    .get_default_values()
                    .captions
                    .font_color),
                KEY_FONT_EDGE => Ok(state
                    .services
                    .cm
                    .clone()
                    .get_default_values()
                    .captions
                    .font_edge),
                KEY_FONT_EDGE_COLOR => Ok(state
                    .services
                    .cm
                    .clone()
                    .get_default_values()
                    .captions
                    .font_edge_color),
                KEY_BACKGROUND_COLOR => Ok(state
                    .services
                    .cm
                    .clone()
                    .get_default_values()
                    .captions
                    .background_color),
                KEY_TEXT_ALIGN => Ok(state
                    .services
                    .cm
                    .clone()
                    .get_default_values()
                    .captions
                    .text_align),
                KEY_TEXT_ALIGN_VERTICAL => Ok(state
                    .services
                    .cm
                    .clone()
                    .get_default_values()
                    .captions
                    .text_align_vertical),
                _ => Err(()),
            }
        } else if namespace.eq(NAMESPACE_DEVICE_NAME) {
            match key {
                KEY_NAME => Ok(state.services.cm.clone().get_default_values().name),
                _ => Err(()),
            }
        } else if namespace.eq(NAMESPACE_LOCALIZATION) {
            match key {
                KEY_COUNTRY_CODE => Ok(state.services.cm.clone().get_default_values().country_code),
                KEY_LANGUAGE => Ok(state.services.cm.clone().get_default_values().language),
                KEY_LOCALE => Ok(state.services.cm.clone().get_default_values().locale),
                KEY_ADDITIONAL_INFO => {
                    let a_info_map: HashMap<String, String> = state
                        .services
                        .cm
                        .clone()
                        .get_default_values()
                        .additional_info;
                    Ok(serde_json::to_string(&a_info_map).unwrap())
                }
                _ => Err(()),
            }
        } else if let Some(defaults) = state
            .services
            .cm
            .clone()
            .get_settings_defaults_per_app()
            .get(namespace)
        {
            Ok(defaults.postal_code.clone())
        } else {
            Err(())
        }
    }

    pub fn get_number_as_u32(
        state: &PlatformState,
        namespace: &String,
        key: &'static str,
    ) -> Result<u32, ()> {
        debug!("get_number_as_u32: namespace={}, key={}", namespace, key);
        if namespace.eq(NAMESPACE_CLOSED_CAPTIONS) {
            match key {
                KEY_FONT_OPACITY => Ok(state
                    .services
                    .cm
                    .clone()
                    .get_default_values()
                    .captions
                    .font_opacity),
                KEY_BACKGROUND_OPACITY => Ok(state
                    .services
                    .cm
                    .clone()
                    .get_default_values()
                    .captions
                    .background_opacity),
                _ => Err(()),
            }
        } else {
            Err(())
        }
    }

    pub fn get_number_as_f32(
        state: &PlatformState,
        namespace: &String,
        key: &'static str,
    ) -> Result<f32, ()> {
        debug!("get_number_as_f32: namespace={}, key={}", namespace, key);
        if namespace.eq(NAMESPACE_CLOSED_CAPTIONS) {
            match key {
                KEY_FONT_SIZE => Ok(state
                    .services
                    .cm
                    .clone()
                    .get_default_values()
                    .captions
                    .font_size as f32),
                _ => Err(()),
            }
        } else {
            Err(())
        }
    }
}

