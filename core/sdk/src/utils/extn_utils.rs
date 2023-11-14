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

use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::runtime::{Builder, Runtime};

pub struct ExtnUtils;
const MIN_STACK_SIZE: usize = 512 * 1024;
const MEDIUM_STACK_SIZE: usize = 1024 * 1024;
const MAX_STACK_SIZE: usize = 2 * 1024 * 1024;

pub enum ExtnStackSize {
    Min,
    Medium,
    Max,
}

impl ExtnStackSize {
    fn get_from_env(env: &str) -> Option<usize> {
        if let Ok(v) = std::env::var(env) {
            if let Ok(v) = v.as_str().parse::<usize>() {
                return Some(v * 1024);
            }
        }
        None
    }
    fn get_size(&self) -> usize {
        match self {
            Self::Min => {
                Self::get_from_env("RIPPLE_STACK_SIZE_MIN_IN_KB").unwrap_or(MIN_STACK_SIZE)
            }
            Self::Medium => {
                Self::get_from_env("RIPPLE_STACK_SIZE_MEDIUM_IN_KB").unwrap_or(MEDIUM_STACK_SIZE)
            }
            Self::Max => {
                Self::get_from_env("RIPPLE_STACK_SIZE_MAX_IN_KB").unwrap_or(MAX_STACK_SIZE)
            }
        }
    }
}

impl From<&str> for ExtnStackSize {
    fn from(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "min" => ExtnStackSize::Min,
            "medium" => ExtnStackSize::Medium,
            "max" => ExtnStackSize::Max,
            _ => ExtnStackSize::Min,
        }
    }
}

impl ExtnUtils {
    pub fn get_runtime(name: String, size: Option<ExtnStackSize>) -> Runtime {
        let size = size.unwrap_or(ExtnStackSize::Min);
        Builder::new_multi_thread()
            .enable_all()
            .worker_threads(1)
            .thread_stack_size(size.get_size())
            .thread_name_fn(move || {
                static ATOMIC_ID: AtomicUsize = AtomicUsize::new(0);
                let id = ATOMIC_ID.fetch_add(1, Ordering::SeqCst);
                format!("{}-{}", name, id)
            })
            .build()
            .unwrap()
    }
}
