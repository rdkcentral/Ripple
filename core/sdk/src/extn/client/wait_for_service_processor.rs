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
    api::status_update::ExtnStatus,
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnEventProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::ExtnMessage,
        extn_id::ExtnId,
    },
    tokio::sync::{mpsc::Receiver as MReceiver, mpsc::Sender as MSender},
};

#[cfg(not(test))]
use log::error;

#[cfg(test)]
use println as error;

#[derive(Debug, Clone)]
pub struct WaitForState {
    capability: ExtnId,
    sender: MSender<ExtnStatus>,
}

#[derive(Debug)]
pub struct WaitForStatusReadyEventProcessor {
    state: WaitForState,
    streamer: DefaultExtnStreamer,
}

/// Event processor used for cases where a certain Extension Capability is required to be ready.
/// Bootstrap uses the [WaitForStatusReadyEventProcessor] to await during Device Connnection before starting the gateway.
impl WaitForStatusReadyEventProcessor {
    pub fn new(
        capability: ExtnId,
        sender: MSender<ExtnStatus>,
    ) -> WaitForStatusReadyEventProcessor {
        WaitForStatusReadyEventProcessor {
            state: WaitForState { capability, sender },
            streamer: DefaultExtnStreamer::new(),
        }
    }

    #[cfg(test)]
    fn clone_with_state(&self, new_state: WaitForState) -> Self {
        WaitForStatusReadyEventProcessor {
            state: new_state,
            streamer: DefaultExtnStreamer::new(), // Assuming DefaultExtnStreamer is also cloneable
        }
    }
}

impl ExtnStreamProcessor for WaitForStatusReadyEventProcessor {
    type VALUE = ExtnStatus;
    type STATE = WaitForState;

    fn get_state(&self) -> Self::STATE {
        self.state.clone()
    }

    fn sender(&self) -> MSender<ExtnMessage> {
        self.streamer.sender()
    }

    fn receiver(&mut self) -> MReceiver<ExtnMessage> {
        self.streamer.receiver()
    }
}

#[async_trait]
impl ExtnEventProcessor for WaitForStatusReadyEventProcessor {
    async fn process_event(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> Option<bool> {
        if msg.requestor.to_string().eq(&state.capability.to_string()) {
            if let ExtnStatus::Ready = extracted_message {
                if state.sender.send(ExtnStatus::Ready).await.is_err() {
                    error!("Failure to wait status message");
                }
                return Some(true);
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        api::config::Config,
        extn::{
            extn_client_message::{ExtnPayload, ExtnRequest},
            extn_id::ExtnClassId,
        },
        framework::ripple_contract::RippleContract,
        tokio::time::Duration,
    };

    #[tokio::test]
    async fn test_wait_for_status_ready_event_processor() {
        // Create a channel for sending ExtnStatus
        let (status_sender, mut status_receiver) = tokio::sync::mpsc::channel::<ExtnStatus>(1);

        // Create a WaitForStatusReadyEventProcessor
        let capability = ExtnId::new_channel(ExtnClassId::Device, "info".to_string());
        let wait_processor =
            WaitForStatusReadyEventProcessor::new(capability.clone(), status_sender);

        // Clone the sender for the inner DefaultExtnStreamer
        let sender_clone = wait_processor.sender();

        // Simulate an ExtnMessage indicating Ready status
        let ready_message = ExtnMessage {
            id: "test_id".to_string(),
            requestor: ExtnId::new_channel(ExtnClassId::Device, "info".to_string()),
            target: RippleContract::RippleContext,
            target_id: Some(ExtnId::get_main_target("main".into())),
            payload: ExtnPayload::Request(ExtnRequest::Config(Config::DefaultName)),
            ts: Some(1234567890),
        };

        // Clone ready_message before moving it into the closure
        let ready_message_clone = ready_message.clone();

        // Use tokio::spawn to run the async function concurrently
        tokio::spawn(async move {
            // Sleep for a short duration before sending the Ready message
            tokio::time::sleep(Duration::from_millis(100)).await;
            sender_clone
                .send(ready_message_clone)
                .await
                .expect("Failed to send ready message");
        });

        // Simulate processing of the ExtnMessage
        let processing_result = WaitForStatusReadyEventProcessor::process_event(
            wait_processor.get_state(),
            ready_message,
            ExtnStatus::Ready,
        )
        .await;

        // Check if the status_sender received the Ready status
        let received_status = status_receiver.recv().await;
        assert_eq!(received_status, Some(ExtnStatus::Ready));

        // Check if the processing result is `Some(true)`
        assert_eq!(processing_result, Some(true));
    }

    #[tokio::test]
    async fn test_wait_for_status_ready_event_processor_no_matching_capability_or_status() {
        // Create a channel for sending ExtnStatus
        let (status_sender, mut status_receiver) = tokio::sync::mpsc::channel::<ExtnStatus>(1);

        // Create a WaitForStatusReadyEventProcessor with a specific capability
        let capability = ExtnId::new_channel(ExtnClassId::Device, "info".to_string());
        let wait_processor =
            WaitForStatusReadyEventProcessor::new(capability.clone(), status_sender);

        // Simulate an ExtnMessage with a different capability and different status
        let invalid_message = ExtnMessage {
            id: "test_id".to_string(),
            requestor: ExtnId::new_channel(ExtnClassId::Device, "other".to_string()),
            target: RippleContract::RippleContext,
            target_id: Some(ExtnId::get_main_target("main".into())),
            payload: ExtnPayload::Request(ExtnRequest::Config(Config::DefaultName)),
            ts: Some(1234567890),
        };

        // Simulate an ExtnMessage with a different capability and different status
        let wait_processor_clone = wait_processor.clone_with_state(wait_processor.get_state());
        let invalid_message_clone = invalid_message.clone();

        // Use tokio::spawn to run the async function concurrently
        tokio::spawn(async move {
            // Sleep for a short duration before sending the message
            tokio::time::sleep(Duration::from_millis(100)).await;
            wait_processor
                .sender()
                .send(invalid_message.clone())
                .await
                .expect("Failed to send invalid message");
        });

        // Simulate processing of the ExtnMessage
        let processing_result = WaitForStatusReadyEventProcessor::process_event(
            wait_processor_clone.get_state().clone(),
            invalid_message_clone,
            ExtnStatus::Error,
        )
        .await;

        // Check if the status_sender did not receive any status
        let received_status =
            tokio::time::timeout(Duration::from_secs(5), status_receiver.recv()).await;

        // Check if the received status is an error with the Elapsed variant
        assert!(matches!(received_status, Err(_elapsed)));

        // Check if the processing result is None because neither capability nor status matched
        assert_eq!(processing_result, None);
    }

    #[tokio::test]
    async fn test_wait_for_status_ready_event_processor_no_matching_capability() {
        // Create a channel for sending ExtnStatus
        let (status_sender, mut status_receiver) = tokio::sync::mpsc::channel::<ExtnStatus>(1);

        // Create a WaitForStatusReadyEventProcessor with a specific capability
        let capability = ExtnId::new_channel(ExtnClassId::Device, "info".to_string());
        let wait_processor =
            WaitForStatusReadyEventProcessor::new(capability.clone(), status_sender);

        // Simulate an ExtnMessage with a different capability and Ready status
        let ready_message = ExtnMessage {
            id: "test_id".to_string(),
            requestor: ExtnId::new_channel(ExtnClassId::Device, "other".to_string()),
            target: RippleContract::RippleContext,
            target_id: Some(ExtnId::get_main_target("main".into())),
            payload: ExtnPayload::Request(ExtnRequest::Config(Config::DefaultName)),
            ts: Some(1234567890),
        };

        // Simulate processing of the ExtnMessage
        let processing_result = WaitForStatusReadyEventProcessor::process_event(
            wait_processor.get_state().clone(),
            ready_message.clone(),
            ExtnStatus::Ready,
        )
        .await;

        // Check if the status_sender did not receive any status
        let _received_status =
            tokio::time::timeout(Duration::from_secs(5), status_receiver.recv()).await;

        // Check if the processing result is None because capability did not match
        assert_eq!(processing_result, None);
    }

    #[tokio::test]
    async fn test_wait_for_status_ready_event_processor_no_matching_status() {
        // Create a channel for sending ExtnStatus
        let (status_sender, mut status_receiver) = tokio::sync::mpsc::channel::<ExtnStatus>(1);

        // Create a WaitForStatusReadyEventProcessor with a specific capability
        let capability = ExtnId::new_channel(ExtnClassId::Device, "info".to_string());
        let wait_processor =
            WaitForStatusReadyEventProcessor::new(capability.clone(), status_sender);

        // Simulate an ExtnMessage with the correct capability but different status
        let invalid_message = ExtnMessage {
            id: "test_id".to_string(),
            requestor: ExtnId::new_channel(ExtnClassId::Device, "info".to_string()),
            target: RippleContract::RippleContext,
            target_id: Some(ExtnId::get_main_target("main".into())),
            payload: ExtnPayload::Request(ExtnRequest::Config(Config::DefaultName)),
            ts: Some(1234567890),
        };

        // Simulate processing of the ExtnMessage
        let processing_result = WaitForStatusReadyEventProcessor::process_event(
            wait_processor.get_state().clone(),
            invalid_message.clone(),
            ExtnStatus::Error,
        )
        .await;

        // Check if the status_sender did not receive any status
        let _received_status =
            tokio::time::timeout(Duration::from_secs(5), status_receiver.recv()).await;

        // Check if the processing result is None because status did not match
        assert_eq!(processing_result, None);
    }
}
