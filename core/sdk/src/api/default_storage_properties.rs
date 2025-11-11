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

use crate::{
    api::storage_property::{
        KEY_ALLOW_ACR_COLLECTION, KEY_ALLOW_APP_CONTENT_AD_TARGETING, KEY_ALLOW_BUSINESS_ANALYTICS,
        KEY_ALLOW_CAMERA_ANALYTICS, KEY_ALLOW_PERSONALIZATION,
        KEY_ALLOW_PRIMARY_BROWSE_AD_TARGETING, KEY_ALLOW_PRIMARY_CONTENT_AD_TARGETING,
        KEY_ALLOW_PRODUCT_ANALYTICS, KEY_ALLOW_REMOTE_DIAGNOSTICS, KEY_ALLOW_RESUME_POINTS,
        KEY_ALLOW_UNENTITLED_PERSONALIZATION, KEY_ALLOW_UNENTITLED_RESUME_POINTS,
        KEY_ALLOW_WATCH_HISTORY, KEY_BACKGROUND_COLOR, KEY_BACKGROUND_OPACITY, KEY_ENABLED,
        KEY_FONT_COLOR, KEY_FONT_EDGE, KEY_FONT_EDGE_COLOR, KEY_FONT_FAMILY, KEY_FONT_OPACITY,
        KEY_FONT_SIZE, KEY_NAME, KEY_SKIP_RESTRICTION, KEY_TEXT_ALIGN, KEY_TEXT_ALIGN_VERTICAL,
        KEY_WINDOW_COLOR, KEY_WINDOW_OPACITY, NAMESPACE_ADVERTISING, NAMESPACE_CLOSED_CAPTIONS,
        NAMESPACE_DEVICE_NAME, NAMESPACE_PRIVACY,
    },
    log::trace,
};

use super::manifest::device_manifest::DefaultValues;

#[derive(Debug, Clone)]
pub enum DefaultStoragePropertiesError {
    UnreconizedKey(String),
    UnreconizedNamespace(String),
    NotFound(String),
}

#[derive(Clone, Debug)]
pub struct DefaultStorageProperties;

impl DefaultStorageProperties {
    pub fn get_bool(
        value: &DefaultValues,
        namespace: &String,
        key: &'static str,
    ) -> Result<bool, DefaultStoragePropertiesError> {
        trace!("get_bool: namespace={}, key={}", namespace, key);
        if namespace.eq(NAMESPACE_CLOSED_CAPTIONS) {
            match key {
                KEY_ENABLED => Ok(value.captions.enabled),
                _ => Err(DefaultStoragePropertiesError::UnreconizedKey(
                    key.to_owned(),
                )),
            }
        } else if namespace.eq(NAMESPACE_PRIVACY) {
            match key {
                KEY_ALLOW_ACR_COLLECTION => Ok(value.allow_acr_collection),
                KEY_ALLOW_APP_CONTENT_AD_TARGETING => Ok(value.allow_app_content_ad_targeting),
                KEY_ALLOW_BUSINESS_ANALYTICS => Ok(value.allow_business_analytics),
                KEY_ALLOW_CAMERA_ANALYTICS => Ok(value.allow_camera_analytics),
                KEY_ALLOW_PERSONALIZATION => Ok(value.allow_personalization),
                KEY_ALLOW_PRIMARY_BROWSE_AD_TARGETING => {
                    Ok(value.allow_primary_browse_ad_targeting)
                }
                KEY_ALLOW_PRIMARY_CONTENT_AD_TARGETING => {
                    Ok(value.allow_primary_content_ad_targeting)
                }
                KEY_ALLOW_PRODUCT_ANALYTICS => Ok(value.allow_product_analytics),
                KEY_ALLOW_REMOTE_DIAGNOSTICS => Ok(value.allow_remote_diagnostics),
                KEY_ALLOW_RESUME_POINTS => Ok(value.allow_resume_points),
                KEY_ALLOW_UNENTITLED_PERSONALIZATION => Ok(value.allow_unentitled_personalization),
                KEY_ALLOW_UNENTITLED_RESUME_POINTS => Ok(value.allow_unentitled_resume_points),
                KEY_ALLOW_WATCH_HISTORY => Ok(value.allow_watch_history),
                _ => Err(DefaultStoragePropertiesError::UnreconizedKey(
                    key.to_owned(),
                )),
            }
        } else {
            Err(DefaultStoragePropertiesError::UnreconizedNamespace(
                namespace.to_owned(),
            ))
        }
    }

    pub fn get_string(
        value: &DefaultValues,
        namespace: &String,
        key: &'static str,
    ) -> Result<String, DefaultStoragePropertiesError> {
        trace!("get_string: namespace={}, key={}", namespace, key);
        if namespace.eq(NAMESPACE_CLOSED_CAPTIONS) {
            let captions = value.clone().captions;
            let not_found = DefaultStoragePropertiesError::NotFound(key.to_owned());
            match key {
                KEY_FONT_FAMILY => match captions.font_family {
                    Some(val) => Ok(val),
                    _ => Err(not_found),
                },
                KEY_FONT_COLOR => match captions.font_color {
                    Some(val) => Ok(val),
                    _ => Err(not_found),
                },
                KEY_FONT_EDGE => match captions.font_edge {
                    Some(val) => Ok(val),
                    _ => Err(not_found),
                },
                KEY_FONT_EDGE_COLOR => match captions.font_edge_color {
                    Some(val) => Ok(val),
                    _ => Err(not_found),
                },
                KEY_BACKGROUND_COLOR => match captions.background_color {
                    Some(val) => Ok(val),
                    _ => Err(not_found),
                },
                KEY_WINDOW_COLOR => match captions.window_color {
                    Some(val) => Ok(val),
                    _ => Err(not_found),
                },
                KEY_TEXT_ALIGN => match captions.text_align {
                    Some(val) => Ok(val),
                    _ => Err(not_found),
                },
                KEY_TEXT_ALIGN_VERTICAL => match captions.text_align_vertical {
                    Some(val) => Ok(val),
                    _ => Err(not_found),
                },
                _ => Err(DefaultStoragePropertiesError::UnreconizedKey(
                    key.to_owned(),
                )),
            }
        } else if namespace.eq(NAMESPACE_DEVICE_NAME) {
            match key {
                KEY_NAME => Ok(value.clone().name),
                _ => Err(DefaultStoragePropertiesError::UnreconizedKey(
                    key.to_owned(),
                )),
            }
        } else if namespace.eq(NAMESPACE_ADVERTISING) {
            match key {
                KEY_SKIP_RESTRICTION => Ok(value.clone().skip_restriction),
                _ => Err(DefaultStoragePropertiesError::UnreconizedKey(
                    key.to_owned(),
                )),
            }
        } else {
            Err(DefaultStoragePropertiesError::UnreconizedNamespace(
                namespace.to_owned(),
            ))
        }
    }

    pub fn get_number_as_u32(
        value: &DefaultValues,
        namespace: &String,
        key: &'static str,
    ) -> Result<u32, DefaultStoragePropertiesError> {
        trace!("get_number_as_u32: namespace={}, key={}", namespace, key);
        if namespace.eq(NAMESPACE_CLOSED_CAPTIONS) {
            let captions = value.clone().captions;
            let not_found = DefaultStoragePropertiesError::NotFound(key.to_owned());
            match key {
                KEY_FONT_OPACITY => match captions.font_opacity {
                    Some(val) => Ok(val),
                    _ => Err(not_found),
                },
                KEY_BACKGROUND_OPACITY => match captions.background_opacity {
                    Some(val) => Ok(val),
                    _ => Err(not_found),
                },
                KEY_WINDOW_OPACITY => match captions.window_opacity {
                    Some(val) => Ok(val),
                    _ => Err(not_found),
                },
                _ => Err(DefaultStoragePropertiesError::UnreconizedKey(
                    key.to_owned(),
                )),
            }
        } else {
            Err(DefaultStoragePropertiesError::UnreconizedNamespace(
                namespace.to_owned(),
            ))
        }
    }

    pub fn get_number_as_f32(
        value: &DefaultValues,
        namespace: &String,
        key: &'static str,
    ) -> Result<f32, DefaultStoragePropertiesError> {
        trace!("get_number_as_f32: namespace={}, key={}", namespace, key);
        if namespace.eq(NAMESPACE_CLOSED_CAPTIONS) {
            let captions = value.clone().captions;
            let not_found = DefaultStoragePropertiesError::NotFound(key.to_owned());
            match key {
                KEY_FONT_SIZE => match captions.font_size {
                    Some(val) => Ok(val),
                    _ => Err(not_found),
                },
                _ => Err(DefaultStoragePropertiesError::UnreconizedKey(
                    key.to_owned(),
                )),
            }
        } else {
            Err(DefaultStoragePropertiesError::UnreconizedNamespace(
                namespace.to_owned(),
            ))
        }
    }
}
