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

use jsonrpsee::tracing::debug;

use crate::{
    processor::storage::storage_property::{
        KEY_BACKGROUND_COLOR, KEY_BACKGROUND_OPACITY, KEY_ENABLED, KEY_FONT_COLOR, KEY_FONT_EDGE,
        KEY_FONT_EDGE_COLOR, KEY_FONT_FAMILY, KEY_FONT_OPACITY, KEY_FONT_SIZE, KEY_TEXT_ALIGN,
        KEY_TEXT_ALIGN_VERTICAL, NAMESPACE_CLOSED_CAPTIONS,
    },
    state::platform_state::PlatformState,
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
                    .get_device_manifest()
                    .clone()
                    .configuration
                    .default_values
                    .captions
                    .enabled),
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
                    .get_device_manifest()
                    .clone()
                    .configuration
                    .default_values
                    .captions
                    .font_family),
                KEY_FONT_COLOR => Ok(state
                    .get_device_manifest()
                    .clone()
                    .configuration
                    .default_values
                    .captions
                    .font_color),
                KEY_FONT_EDGE => Ok(state
                    .get_device_manifest()
                    .clone()
                    .configuration
                    .default_values
                    .captions
                    .font_edge),
                KEY_FONT_EDGE_COLOR => Ok(state
                    .get_device_manifest()
                    .clone()
                    .configuration
                    .default_values
                    .captions
                    .font_edge_color),
                KEY_BACKGROUND_COLOR => Ok(state
                    .get_device_manifest()
                    .clone()
                    .configuration
                    .default_values
                    .captions
                    .background_color),
                KEY_TEXT_ALIGN => Ok(state
                    .get_device_manifest()
                    .clone()
                    .configuration
                    .default_values
                    .captions
                    .text_align),
                KEY_TEXT_ALIGN_VERTICAL => Ok(state
                    .get_device_manifest()
                    .clone()
                    .configuration
                    .default_values
                    .captions
                    .text_align_vertical),
                _ => Err(()),
            }
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
                    .get_device_manifest()
                    .clone()
                    .configuration
                    .default_values
                    .captions
                    .font_opacity),
                KEY_BACKGROUND_OPACITY => Ok(state
                    .get_device_manifest()
                    .clone()
                    .configuration
                    .default_values
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
                    .get_device_manifest()
                    .clone()
                    .configuration
                    .default_values
                    .captions
                    .font_size as f32),
                _ => Err(()),
            }
        } else {
            Err(())
        }
    }
}
