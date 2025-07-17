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

use crate::extn::extn_client_message::ExtnPayloadProvider;
use crate::framework::ripple_contract::RippleContract;
use sysinfo::System;

pub fn test_extn_payload_provider<T>(request: T, contract_type: RippleContract)
where
    T: ExtnPayloadProvider + PartialEq + std::fmt::Debug,
{
    let value = request.get_extn_payload();
    if let Some(v) = T::get_from_payload(value) {
        assert_eq!(v, request);
        assert_eq!(contract_type, T::contract());
    } else {
        panic!("Test failed for ExtnRequest variant: {:?}", request);
    }
}

pub fn log_memory_usage(label: &str) {
    let mut sys = System::new();
    sys.refresh_process(sysinfo::get_current_pid().unwrap());

    if let Some(proc) = sys.process(sysinfo::get_current_pid().unwrap()) {
        let memory_usage = proc.memory() as f64 / 1_048_576.0; // Convert to MB
        println!("\n\nMemory usage {}: {:.2} MB\n\n", label, memory_usage);
    } else {
        println!("Failed to retrieve process information for memory usage.");
    }
}

pub fn get_memory_usage_mb() -> f64 {
    let mut sys = System::new();
    sys.refresh_process(sysinfo::get_current_pid().unwrap());

    if let Some(proc) = sys.process(sysinfo::get_current_pid().unwrap()) {
        proc.memory() as f64 / 1_048_576.0 // Convert to MB
    } else {
        0.0 // Return 0 if process info is not available
    }
}
