// Copyright 2025 Comcast Cable Communications Management, LLC
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

#[allow(dead_code, unused_imports)]
use crate::ripple_sdk::{serde_json::json, tokio};
#[allow(dead_code, unused_imports)]
use crate::tests::contracts::contract_utils::*;
#[allow(dead_code, unused_imports)]
use crate::{get_pact_with_params, send_thunder_call_message};
#[allow(dead_code, unused_imports)]
use pact_consumer::mock_server::StartMockServerAsync;
#[allow(dead_code, unused_imports)]
use pact_consumer::prelude::*;
#[allow(dead_code, unused_imports)]
use ripple_sdk::chrono;
#[allow(dead_code, unused_imports)]
use rstest::rstest;
#[allow(dead_code, unused_imports)]
use std::collections::HashMap;

#[allow(dead_code, unused_imports)]
use futures_util::{SinkExt, StreamExt};
#[allow(dead_code, unused_imports)]
use ripple_sdk::tokio_tungstenite::connect_async;
#[allow(dead_code, unused_imports)]
use ripple_sdk::tokio_tungstenite::tungstenite::protocol::Message;
#[allow(dead_code, unused_imports)]
use tokio::time::{timeout, Duration};

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_get_presentation_language() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
    .synchronous_message_interaction("A request to get the presentation language", |mut i| async move {
        i.contents_from(json!({
            "pact:content-type": "application/json",
            "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 0)", "method": "org.rdk.UserSettings.getPresentationLanguage"},
            "requestMetadata": {
                "path": "/jsonrpc"
            },
            "response": [{
                "jsonrpc": "matching(type, '2.0')",
                "id": "matching(integer, 0)",
                "result": "matching(type, 'en-US')"
            }]
        })).await;
        i.test_name("get_presentation_language");

        i
    }).await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "org.rdk.UserSettings.getPresentationLanguage"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_set_presentation_language() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction(
            "A request to set the presentation language",
            |mut i| async move {
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 42)",
                        "method": "org.rdk.UserSettings.setPresentationLanguage",
                        "params": {
                            "presentationLanguage": "matching(type, 'en-US')"
                        }
                    },
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [{
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 42)",
                        "result": null
                    }]
                }))
                .await;
                i.test_name("set_presentation_language");

                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 42,
            "method": "org.rdk.UserSettings.setPresentationLanguage",
            "params": {
                "presentationLanguage": "en-US"
            }
        })
    )
    .await;
}
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_get_preferred_audio_languages() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction(
            "A request to get the preferred audio languages",
            |mut i| async move {
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 0)",
                        "method": "org.rdk.UserSettings.getPreferredAudioLanguages"
                    },
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [{
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 0)",
                        "result": "matching(type, 'eng')"
                    }]
                }))
                .await;
                i.test_name("get_preferred_audio_languages");

                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "org.rdk.UserSettings.getPreferredAudioLanguages"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_set_preferred_audio_languages() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction(
            "A request to set the preferred audio languages",
            |mut i| async move {
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 42)",
                        "method": "org.rdk.UserSettings.setPreferredAudioLanguages",
                        "params": {
                            "languages": "matching(type, 'eng')"
                        }
                    },
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [{
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 42)",
                        "result": null
                    }]
                }))
                .await;
                i.test_name("set_preferred_audio_languages");

                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 42,
            "method": "org.rdk.UserSettings.setPreferredAudioLanguages",
            "params": {
                "languages": "eng"
            }
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_set_voice_guidance() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction(
            "A request to set the voice guidance",
            |mut i| async move {
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 42)",
                        "method": "org.rdk.UserSettings.setVoiceGuidance",
                        "params": {
                            "enabled": "matching(type, true)"
                        }
                    },
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [{
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 42)",
                        "result": null
                    }]
                }))
                .await;
                i.test_name("set_voice_guidance");

                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 42,
            "method": "org.rdk.UserSettings.setVoiceGuidance",
            "params": {
                "enabled": true
            }
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_get_voice_guidance_rate() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    // Use the same value as set in the setter
    let rate_value = 0.1;

    pact_builder_async
        .synchronous_message_interaction(
            "A request to get the voice guidance rate",
            |mut i| async move {
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 42)",
                        "method": "org.rdk.UserSettings.getVoiceGuidanceRate"
                    },
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [{
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 42)",
                        "result": rate_value
                    }]
                }))
                .await;
                i.test_name("get_voice_guidance_rate");

                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 42,
            "method": "org.rdk.UserSettings.getVoiceGuidanceRate"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_set_voice_guidance_rate() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    // Set a valid value for rate
    let rate_value = 0.1;

    pact_builder_async
        .synchronous_message_interaction(
            "A request to set the voice guidance rate",
            |mut i| async move {
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 42)",
                        "method": "org.rdk.UserSettings.setVoiceGuidanceRate",
                        "params": {
                            "rate": rate_value
                        }
                    },
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [{
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 42)",
                        "result": null
                    }]
                }))
                .await;
                i.test_name("set_voice_guidance_rate");

                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 42,
            "method": "org.rdk.UserSettings.setVoiceGuidanceRate",
            "params": {
                "rate": rate_value
            }
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_get_audio_description() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction(
            "A request to get the audio description",
            |mut i| async move {
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 0)",
                        "method": "org.rdk.UserSettings.getAudioDescription"
                    },
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [{
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 0)",
                        "result": "matching(type, true)"
                    }]
                }))
                .await;
                i.test_name("get_audio_description");

                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "org.rdk.UserSettings.getAudioDescription"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_set_audio_description() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction(
            "A request to set the audio description",
            |mut i| async move {
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 42)",
                        "method": "org.rdk.UserSettings.setAudioDescription",
                        "params": {
                            "enabled": "matching(type, true)"
                        }
                    },
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [{
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 42)",
                        "result": null
                    }]
                }))
                .await;
                i.test_name("set_audio_description");

                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 42,
            "method": "org.rdk.UserSettings.setAudioDescription",
            "params": {
                "enabled": true
            }
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_get_high_contrast() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction(
            "A request to get the high contrast setting",
            |mut i| async move {
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 0)",
                        "method": "org.rdk.UserSettings.getHighContrast"
                    },
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [{
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 0)",
                        "result": "matching(type, true)"
                    }]
                }))
                .await;
                i.test_name("get_high_contrast");

                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "org.rdk.UserSettings.getHighContrast"
        })
    )
    .await;
}
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_get_captions() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction(
            "A request to get the captions setting",
            |mut i| async move {
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 0)",
                        "method": "org.rdk.UserSettings.getCaptions"
                    },
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [{
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 0)",
                        "result": "matching(type, true)"
                    }]
                }))
                .await;
                i.test_name("get_captions");

                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "org.rdk.UserSettings.getCaptions"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_set_captions() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction(
            "A request to set the captions setting",
            |mut i| async move {
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 42)",
                        "method": "org.rdk.UserSettings.setCaptions",
                        "params": {
                            "enabled": "matching(type, true)"
                        }
                    },
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [{
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 42)",
                        "result": null
                    }]
                }))
                .await;
                i.test_name("set_captions");

                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 42,
            "method": "org.rdk.UserSettings.setCaptions",
            "params": {
                "enabled": true
            }
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_get_preferred_captions_languages() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction(
            "A request to get the preferred captions languages",
            |mut i| async move {
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 0)",
                        "method": "org.rdk.UserSettings.getPreferredCaptionsLanguages"
                    },
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [{
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 0)",
                        "result": "matching(type, 'eng')"
                    }]
                }))
                .await;
                i.test_name("get_preferred_captions_languages");

                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "org.rdk.UserSettings.getPreferredCaptionsLanguages"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_set_preferred_captions_languages() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction(
            "A request to set the preferred captions languages",
            |mut i| async move {
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 42)",
                        "method": "org.rdk.UserSettings.setPreferredCaptionsLanguages",
                        "params": {
                            "languages": "matching(type, 'eng')"
                        }
                    },
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [{
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 42)",
                        "result": null
                    }]
                }))
                .await;
                i.test_name("set_preferred_captions_languages");

                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 42,
            "method": "org.rdk.UserSettings.setPreferredCaptionsLanguages",
            "params": {
                "languages": "eng"
            }
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_get_voice_guidance_hints() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    // Use the same value as set in the setter
    let hints_value = "enabled";

    pact_builder_async
        .synchronous_message_interaction(
            "A request to get the voice guidance hints",
            |mut i| async move {
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 42)",
                        "method": "org.rdk.UserSettings.getVoiceGuidanceHints"
                    },
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [{
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 42)",
                        "result": hints_value
                    }]
                }))
                .await;
                i.test_name("get_voice_guidance_hints");

                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 42,
            "method": "org.rdk.UserSettings.getVoiceGuidanceHints"
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_set_voice_guidance_hints() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    // Set a valid value for hints
    let hints_value = "enabled";

    pact_builder_async
        .synchronous_message_interaction(
            "A request to set the voice guidance hints",
            |mut i| async move {
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 42)",
                        "method": "org.rdk.UserSettings.setVoiceGuidanceHints",
                        "params": {
                            "hints": hints_value
                        }
                    },
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [{
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 42)",
                        "result": null
                    }]
                }))
                .await;
                i.test_name("set_voice_guidance_hints");

                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 42,
            "method": "org.rdk.UserSettings.setVoiceGuidanceHints",
            "params": {
                "hints": hints_value
            }
        })
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "websocket_contract_tests"), ignore)]
async fn test_device_get_voice_guidance() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
        .synchronous_message_interaction(
            "A request to get the voice guidance setting",
            |mut i| async move {
                i.contents_from(json!({
                    "pact:content-type": "application/json",
                    "request": {
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 0)",
                        "method": "org.rdk.UserSettings.getVoiceGuidance"
                    },
                    "requestMetadata": {
                        "path": "/jsonrpc"
                    },
                    "response": [{
                        "jsonrpc": "matching(type, '2.0')",
                        "id": "matching(integer, 0)",
                        "result": "matching(type, true)"
                    }]
                }))
                .await;
                i.test_name("get_voice_guidance");

                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    send_thunder_call_message!(
        url::Url::parse(mock_server.path("/jsonrpc").as_str())
            .unwrap()
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "org.rdk.UserSettings.getVoiceGuidance"
        })
    )
    .await;
}
