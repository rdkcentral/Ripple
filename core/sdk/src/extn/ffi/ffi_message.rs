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

use async_channel::Sender as CSender;

use crate::{
    extn::{
        extn_client_message::{ExtnMessage, ExtnPayload},
        extn_id::ExtnId,
    },
    framework::ripple_contract::RippleContract,
    utils::error::RippleError,
};

/// Contains C Alternates for
/// CExtnRequest
/// CExtnResponse
/// From<CExtnResponse> for ExtnResponse
/// From<ExtnRequest> for CExtnRequest
///
#[repr(C)]
#[derive(Clone, Debug)]
pub struct CExtnMessage {
    pub id: String,
    pub requestor: String,
    pub target: String,
    pub target_id: String,
    pub payload: String,
    pub callback: Option<CSender<CExtnMessage>>,
    pub ts: i64,
}



impl TryInto<ExtnMessage> for CExtnMessage {
    type Error = RippleError;

    fn try_into(self) -> Result<ExtnMessage, Self::Error> {
        let requestor_capability: Result<ExtnId, RippleError> = self.requestor.try_into();
        if requestor_capability.is_err() {
            return Err(RippleError::ParseError);
        }
        let requestor = requestor_capability.unwrap();

        let target_contract: Result<RippleContract, RippleError> = self.target.try_into();
        if target_contract.is_err() {
            return Err(RippleError::ParseError);
        }
        let target = target_contract.unwrap();
        let target_id: Result<ExtnId, RippleError> = self.target_id.try_into();
        let target_id = target_id.ok();

        let payload: Result<ExtnPayload, RippleError> = self.payload.try_into();
        if payload.is_err() {
            return Err(RippleError::ParseError);
        }
        let payload = payload.unwrap();
        let ts = Some(self.ts);
        Ok(ExtnMessage {
            id: self.id,
            requestor,
            target,
            target_id,
            payload,
            ts,
        })
    }
}

#[cfg(test)]
mod tests {
    use core::panic;

    use super::*;
    use crate::api::config::Config;
    use crate::extn::extn_client_message::ExtnRequest;
    use crate::extn::extn_id::ExtnClassId;
    use crate::framework::ripple_contract::RippleContract;
    use rstest::rstest;


    #[rstest(
        requestor,
        target,
        payload,
        exp_res,
        case(
            ExtnId::new_channel(ExtnClassId::Device, "thunder_comcast".into())
                .to_string(),
            format!("\"{}\"", RippleContract::DeviceInfo.as_clear_string()),
            r#"{"Request":{"Config":"DefaultName"}}"#.to_string(),
            Ok(()),
        ),
        case(
            RippleContract::Internal.as_clear_string(),
            format!("\"{}\"", RippleContract::DeviceInfo.as_clear_string()),
            r#"{"Request":{"Config":"DefaultName"}}"#.to_string(),
            Err(RippleError::ParseError),
        ),
        case(
            ExtnId::new_channel(ExtnClassId::Device, "thunder_comcast".into())
                .to_string(),
            RippleContract::DeviceInfo.as_clear_string(),
            r#"{"Request":{"Config":"DefaultName"}}"#.to_string(),
            Err(RippleError::ParseError),
        ),
        case(
            ExtnId::new_channel(ExtnClassId::Device, "thunder_comcast".into())
                .to_string(),
            format!("\"{}\"", RippleContract::DeviceInfo.as_clear_string()),
            r#"{"test":{"Config":"DefaultName"}}"#.to_string(),
            Err(RippleError::ParseError),
        )
    )]
    fn test_try_into_extn_message_from_c_extn_message(
        requestor: String,
        target: String,
        payload: String,
        exp_res: Result<(), RippleError>,
    ) {
        // Create a mock CExtnMessage
        let c_extn_message = CExtnMessage {
            id: "test_id".to_string(),
            requestor: requestor.clone(),
            target,
            target_id: "".to_string(),
            payload,
            callback: None,
            ts: 1234567890,
        };

        // Convert the mock CExtnMessage to ExtnMessage
        let extn_message: Result<ExtnMessage, RippleError> = c_extn_message.try_into();

        match exp_res {
            Ok(_) => {
                assert!(extn_message.is_ok(), "Expected Ok, but got Err");
                if let Ok(extn_message) = extn_message {
                    assert_eq!(extn_message.id, "test_id");
                    assert_eq!(extn_message.requestor.to_string(), requestor);
                    assert_eq!(extn_message.target, RippleContract::DeviceInfo);
                    assert_eq!(extn_message.target_id, None);
                    assert_eq!(
                        extn_message.payload,
                        ExtnPayload::Request(ExtnRequest::Config(
                            crate::api::config::Config::DefaultName
                        ))
                    );
                    assert_eq!(extn_message.ts.unwrap(), 1234567890);
                } else {
                    panic!("Expected Ok, but got Err: {:?}", extn_message);
                }
            }
            Err(expected_err) => {
                assert!(
                    extn_message.is_err(),
                    "Expected Err, but got Ok: {:?}",
                    extn_message
                );

                let actual_err = extn_message.unwrap_err();
                assert_eq!(actual_err, expected_err, "Unexpected error");
            }
        }
    }
}
