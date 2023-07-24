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

use crossbeam::channel::Sender as CSender;

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
    pub payload: String,
    pub callback: Option<CSender<CExtnMessage>>,
    pub ts: i64,
}

impl From<ExtnMessage> for CExtnMessage {
    fn from(value: ExtnMessage) -> Self {
        let payload: String = value.payload.into();
        CExtnMessage {
            callback: value.callback,
            id: value.id,
            payload,
            requestor: value.requestor.to_string(),
            target: value.target.into(),
            ts: if let Some(v) = value.ts {
                v
            } else {
                chrono::Utc::now().timestamp_millis()
            },
        }
    }
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

        let payload: Result<ExtnPayload, RippleError> = self.payload.try_into();
        if payload.is_err() {
            return Err(RippleError::ParseError);
        }
        let payload = payload.unwrap();
        let ts = Some(self.ts);
        Ok(ExtnMessage {
            callback: self.callback,
            id: self.id,
            requestor,
            target,
            payload,
            ts,
        })
    }
}
