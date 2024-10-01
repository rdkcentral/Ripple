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

use ripple_sdk::async_trait::async_trait;
use ripple_sdk::{framework::bootstrap::Bootstep, tokio, utils::error::RippleError};

use crate::processor::storage::storage_migrator::StorageMigrator;
use crate::state::bootstrap_state::BootstrapState;

pub struct StartStorageMigratorStep;

#[async_trait]
impl Bootstep<BootstrapState> for StartStorageMigratorStep {
    fn get_name(&self) -> String {
        "StartStorageMigrator".into()
    }

    async fn setup(&self, bootstrap_state: BootstrapState) -> Result<(), RippleError> {
        tokio::spawn(async move {
            let mut migrator = StorageMigrator::new(&bootstrap_state.platform_state);
            migrator.migration_test().await;
        });
        Ok(())
    }
}
