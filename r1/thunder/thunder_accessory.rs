use async_trait::async_trait;
use dab_core::{
    message::{DabError, DabRequest},
    model::accessory::{AccessoryListType, AccessoryRequest, AccessoryService, AccessoryType},
};
use tracing::{error, Instrument};

use crate::{
    client::thunder_client::ThunderClient, service::thunder_service_resolver::ThunderDelegate,
};

use super::accessory::thunder_remote::ThunderRemoteAccessoryService;

pub struct AccessoryDelegate;

#[async_trait]
impl ThunderDelegate for AccessoryDelegate {
    async fn handle(&self, request: DabRequest, client: Box<ThunderClient>) {
        match request.payload.as_accessory_request() {
            Some(req) => match req {
                AccessoryRequest::Pair(request_params) => match request_params._type {
                    AccessoryType::Remote => {
                        let service = Box::new(ThunderRemoteAccessoryService { client: client });
                        let span = request.req_span("AccessoryRequest::Pair");
                        service
                            .pair(request, Some(request_params))
                            .instrument(span)
                            .await;
                    }
                    _ => request.respond_and_log(Err(DabError::NotSupported)),
                },
                AccessoryRequest::List(request_params) => match request_params._type {
                    Some(AccessoryListType::Remote) | Some(AccessoryListType::All) => {
                        let service = Box::new(ThunderRemoteAccessoryService { client: client });
                        let span = request.req_span("AccessoryRequest:::List");
                        service
                            .list(request, Some(request_params))
                            .instrument(span)
                            .await;
                    }
                    _ => request.respond_and_log(Err(DabError::NotSupported)),
                },
            },
            None => error!("Invalid DAB payload for Accessory module"),
        }
    }
}

#[cfg(test)]
pub mod tests {
    use std::collections::HashMap;

    use crate::{
        client::thunder_client::{
            ThunderCallMessage, ThunderResponseMessage, ThunderSubscribeMessage,
        },
        test::{mock_thunder_controller::CustomHandler, test_utils::ThunderDelegateTest},
        util::channel_util::oneshot_send_and_log,
    };
    use dab_core::{
        message::{DabRequestPayload, DabResponse},
        model::accessory::*,
    };
    use serde_json::json;

    use super::*;
    use tokio::sync::oneshot::error::RecvError;

    #[tokio::test]
    async fn test_bluetooth_remote_pair() {
        let mut custom_handler = CustomHandler {
            custom_request_handler: HashMap::new(),
            custom_subscription_handler: HashMap::new(),
        };
        let payload = DabRequestPayload::Accessory(AccessoryRequest::Pair(AccessoryPairRequest {
            _type: AccessoryType::Remote,
            protocol: AccessoryProtocol::BluetoothLE,
            timeout: 4,
        }));
        let handle_function = |msg: ThunderCallMessage| {
            let response = json!({"success" : true});
            let resp_msg = ThunderResponseMessage::call(response.clone());
            oneshot_send_and_log(msg.callback, resp_msg, "StatusReturn")
        };
        let event_handler = |_msg: &ThunderSubscribeMessage| {
            let remote_status_event = json!({"status":{"netType": 1, "pairingState":"CONFIGURATION_COMPLETE", "remoteData":[{"make":"xyz", "model":"abc"}]}});
            ThunderResponseMessage::call(remote_status_event)
        };
        custom_handler.custom_subscription_handler.insert(
            String::from("org.rdk.RemoteControl".to_owned() + &"onStatus".to_owned()),
            event_handler,
        );
        custom_handler.custom_request_handler.insert(
            String::from("org.rdk.RemoteControl".to_owned() + &"startPairing".to_owned()),
            handle_function,
        );
        let delegate = Box::new(AccessoryDelegate);
        let validator = |res: Result<DabResponse, RecvError>| {
            assert!(matches!(res, Ok { .. }));
            if let Ok(dab_response) = res {
                assert!(matches!(dab_response, Ok { .. }));
                if let Ok(payload) = dab_response {
                    assert!(matches!(
                        payload,
                        dab_core::message::DabResponsePayload::AccessoryDeviceResponse { .. }
                    ));
                    if let dab_core::message::DabResponsePayload::AccessoryDeviceResponse(data) =
                        payload
                    {
                        assert_eq!(data.make, "xyz");
                        assert_eq!(data.model, "abc");
                    }
                }
            }
        };
        let delegate_test = ThunderDelegateTest::new(delegate);
        delegate_test
            .start_test(payload, Some(custom_handler), Box::new(validator))
            .await;
    }

    #[tokio::test]
    async fn test_bluetooth_remote_pair_fail() {
        let mut custom_handler = CustomHandler {
            custom_request_handler: HashMap::new(),
            custom_subscription_handler: HashMap::new(),
        };
        let payload = DabRequestPayload::Accessory(AccessoryRequest::Pair(AccessoryPairRequest {
            _type: AccessoryType::Remote,
            protocol: AccessoryProtocol::BluetoothLE,
            timeout: 4,
        }));
        let handle_function = |msg: ThunderCallMessage| {
            let response = json!({"success" : false});
            let resp_msg = ThunderResponseMessage::call(response.clone());
            oneshot_send_and_log(msg.callback, resp_msg, "StatusReturn")
        };
        custom_handler.custom_request_handler.insert(
            String::from("org.rdk.RemoteControl".to_owned() + &"startPairing".to_owned()),
            handle_function,
        );
        let delegate = Box::new(AccessoryDelegate);
        let validator = |res: Result<DabResponse, RecvError>| {
            assert!(matches!(res, Ok { .. }));
            if let Ok(dab_response) = res {
                assert!(matches!(dab_response, Err { .. }));
            }
        };
        let delegate_test = ThunderDelegateTest::new(delegate);
        delegate_test
            .start_test(payload, Some(custom_handler), Box::new(validator))
            .await;
    }

    #[tokio::test]
    async fn test_bluetooth_remote_pair_empty_data() {
        let mut custom_handler = CustomHandler {
            custom_request_handler: HashMap::new(),
            custom_subscription_handler: HashMap::new(),
        };
        let payload = DabRequestPayload::Accessory(AccessoryRequest::Pair(AccessoryPairRequest {
            _type: AccessoryType::Remote,
            protocol: AccessoryProtocol::BluetoothLE,
            timeout: 4,
        }));

        let handle_function = |msg: ThunderCallMessage| {
            let response = json!({"success" : true});
            let resp_msg = ThunderResponseMessage::call(response.clone());
            oneshot_send_and_log(msg.callback, resp_msg, "StatusReturn")
        };
        let event_handler = |_msg: &ThunderSubscribeMessage| {
            let remote_status_event = json!({"status":{"netType": 1, "pairingState":"CONFIGURATION_COMPLETE", "remoteData":[]}});
            ThunderResponseMessage::call(remote_status_event)
        };
        custom_handler.custom_subscription_handler.insert(
            String::from("org.rdk.RemoteControl".to_owned() + &"onStatus".to_owned()),
            event_handler,
        );
        custom_handler.custom_request_handler.insert(
            String::from("org.rdk.RemoteControl".to_owned() + &"startPairing".to_owned()),
            handle_function,
        );
        let delegate = Box::new(AccessoryDelegate);
        let validator = |res: Result<DabResponse, RecvError>| {
            assert!(matches!(res, Ok { .. }));
            if let Ok(dab_response) = res {
                assert!(matches!(dab_response, Ok { .. }));
                if let Ok(payload) = dab_response {
                    assert!(matches!(
                        payload,
                        dab_core::message::DabResponsePayload::AccessoryDeviceResponse { .. }
                    ));
                    if let dab_core::message::DabResponsePayload::AccessoryDeviceResponse(data) =
                        payload
                    {
                        assert_eq!(data.make, "");
                        assert_eq!(data.model, "");
                    }
                }
            }
        };
        let delegate_test = ThunderDelegateTest::new(delegate);
        delegate_test
            .start_test(payload, Some(custom_handler), Box::new(validator))
            .await;
    }

    #[tokio::test]
    async fn test_bluetooth_remote_pair_failed() {
        let mut custom_handler = CustomHandler {
            custom_request_handler: HashMap::new(),
            custom_subscription_handler: HashMap::new(),
        };
        let payload = DabRequestPayload::Accessory(AccessoryRequest::Pair(AccessoryPairRequest {
            _type: AccessoryType::Remote,
            protocol: AccessoryProtocol::BluetoothLE,
            timeout: 4,
        }));

        let handle_function = |msg: ThunderCallMessage| {
            let response = json!({"success" : true});
            let resp_msg = ThunderResponseMessage::call(response.clone());
            oneshot_send_and_log(msg.callback, resp_msg, "StatusReturn")
        };
        let event_handler = |_msg: &ThunderSubscribeMessage| {
            let remote_status_event =
                json!({"status":{"netType": 1, "pairingState":"FAILED", "remoteData":[]}});
            ThunderResponseMessage::call(remote_status_event)
        };
        custom_handler.custom_subscription_handler.insert(
            String::from("org.rdk.RemoteControl".to_owned() + &"onStatus".to_owned()),
            event_handler,
        );
        custom_handler.custom_request_handler.insert(
            String::from("org.rdk.RemoteControl".to_owned() + &"startPairing".to_owned()),
            handle_function,
        );
        let delegate = Box::new(AccessoryDelegate);
        let validator = |res: Result<DabResponse, RecvError>| {
            assert!(matches!(res, Ok { .. }));
            if let Ok(dab_response) = res {
                assert!(matches!(dab_response, Err { .. }));
                if let Err(payload) = dab_response {
                    assert!(matches!(payload, DabError::OsError));
                }
            }
        };
        let delegate_test = ThunderDelegateTest::new(delegate);
        delegate_test
            .start_test(payload, Some(custom_handler), Box::new(validator))
            .await;
    }

    #[tokio::test]
    async fn test_bluetooth_speaker_pair() {
        let payload = DabRequestPayload::Accessory(AccessoryRequest::Pair(AccessoryPairRequest {
            _type: AccessoryType::Speaker,
            protocol: AccessoryProtocol::BluetoothLE,
            timeout: 4,
        }));
        let delegate = Box::new(AccessoryDelegate);
        let validator = |res: Result<DabResponse, RecvError>| {
            assert!(matches!(res, Ok { .. }));
            if let Ok(dab_response) = res {
                assert!(matches!(dab_response, Err { .. }));
                if let Err(payload) = dab_response {
                    assert!(matches!(payload, DabError::NotSupported));
                }
            }
        };
        let delegate_test = ThunderDelegateTest::new(delegate);
        delegate_test
            .start_test(payload, None, Box::new(validator))
            .await;
    }

    #[tokio::test]
    async fn test_list_speaker() {
        let payload = DabRequestPayload::Accessory(AccessoryRequest::List(AccessoryListRequest {
            _type: Some(AccessoryListType::Speaker),
            protocol: Some(AccessoryProtocolListType::BluetoothLE),
        }));

        let delegate = Box::new(AccessoryDelegate);
        let validator = |res: Result<DabResponse, RecvError>| {
            assert!(matches!(res, Ok { .. }));
            if let Ok(dab_response) = res {
                assert!(matches!(dab_response, Err { .. }));
                if let Err(payload) = dab_response {
                    assert!(matches!(payload, DabError::NotSupported));
                }
            }
        };
        let delegate_test = ThunderDelegateTest::new(delegate);
        delegate_test
            .start_test(payload, None, Box::new(validator))
            .await;
    }

    #[tokio::test]
    async fn test_list_remote() {
        let payload = DabRequestPayload::Accessory(AccessoryRequest::List(AccessoryListRequest {
            _type: Some(AccessoryListType::Remote),
            protocol: Some(AccessoryProtocolListType::BluetoothLE),
        }));
        let mut custom_handler = CustomHandler {
            custom_request_handler: HashMap::new(),
            custom_subscription_handler: HashMap::new(),
        };
        let handle_function = |msg: ThunderCallMessage| {
            let response = json!({"status":{"netType": 1, "pairingState":"CONFIGURATION_COMPLETE", "remoteData":[{"make":"xyz", "model":"abc"}]}});
            let resp_msg = ThunderResponseMessage::call(response.clone());
            oneshot_send_and_log(msg.callback, resp_msg, "StatusReturn")
        };
        custom_handler.custom_request_handler.insert(
            String::from("org.rdk.RemoteControl".to_owned() + &"getNetStatus".to_owned()),
            handle_function,
        );

        let delegate = Box::new(AccessoryDelegate);
        let validator = |res: Result<DabResponse, RecvError>| {
            assert!(matches!(res, Ok { .. }));
            if let Ok(dab_response) = res {
                assert!(matches!(dab_response, Ok { .. }));
                if let Ok(payload) = dab_response {
                    assert!(matches!(
                        payload,
                        dab_core::message::DabResponsePayload::AccessoryDeviceListResponse { .. }
                    ));
                    if let dab_core::message::DabResponsePayload::AccessoryDeviceListResponse(
                        data,
                    ) = payload
                    {
                        assert_eq!(data.list.len(), 1);
                    }
                }
            }
        };
        let delegate_test = ThunderDelegateTest::new(delegate);
        delegate_test
            .start_test(payload, Some(custom_handler), Box::new(validator))
            .await;
    }

    #[tokio::test]
    async fn test_list_remote_empty() {
        let payload = DabRequestPayload::Accessory(AccessoryRequest::List(AccessoryListRequest {
            _type: Some(AccessoryListType::Remote),
            protocol: Some(AccessoryProtocolListType::BluetoothLE),
        }));

        let mut custom_handler = CustomHandler {
            custom_request_handler: HashMap::new(),
            custom_subscription_handler: HashMap::new(),
        };
        let handle_function = |msg: ThunderCallMessage| {
            let response = json!({"status":{"netType": 1, "pairingState":"CONFIGURATION_COMPLETE", "remoteData":[]}});
            let resp_msg = ThunderResponseMessage::call(response.clone());
            oneshot_send_and_log(msg.callback, resp_msg, "StatusReturn")
        };
        custom_handler.custom_request_handler.insert(
            String::from("org.rdk.RemoteControl".to_owned() + &"getNetStatus".to_owned()),
            handle_function,
        );
        let delegate = Box::new(AccessoryDelegate);
        let validator = |res: Result<DabResponse, RecvError>| {
            assert!(matches!(res, Ok { .. }));
            if let Ok(dab_response) = res {
                assert!(matches!(dab_response, Ok { .. }));
                if let Ok(payload) = dab_response {
                    assert!(matches!(
                        payload,
                        dab_core::message::DabResponsePayload::AccessoryDeviceListResponse { .. }
                    ));
                    if let dab_core::message::DabResponsePayload::AccessoryDeviceListResponse(
                        data,
                    ) = payload
                    {
                        assert_eq!(data.list.len(), 0);
                    }
                }
            }
        };
        let delegate_test = ThunderDelegateTest::new(delegate);
        delegate_test
            .start_test(payload, Some(custom_handler), Box::new(validator))
            .await;
    }
}
