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

use log::{debug, info, warn};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

use crate::utils::error::RippleError;

#[derive(Debug, Clone)]
pub struct FileStore<S> {
    pub value: S,
    path: String,
}
impl<S: Default> Default for FileStore<S> {
    fn default() -> Self {
        FileStore {
            value: S::default(),
            path: String::from("/tmp/mock.json"),
        }
    }
}
impl<S> FileStore<S>
where
    S: Serialize + DeserializeOwned + Clone,
{
    pub fn new(path: String, value: S) -> FileStore<S> {
        FileStore {
            value,
            path: Path::new(&path).to_str().unwrap().into(),
        }
    }

    fn write_to_disk(&self, value: String) {
        // Create the folder if it doesnt exist
        let p = Path::new(&self.path);
        if let Some(parent) = p.parent() {
            let _ = fs::create_dir_all(parent);
        }
        match OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(self.path.clone())
        {
            Ok(mut file) => {
                if let Err(e) = file.write_all(value.as_bytes()) {
                    warn!("Failed to write file store for {:?} {}", e, self.path);
                }
            }
            Err(e) => {
                warn!("Failed to open file store for {} {:?}", self.path, e);
            }
        }
    }

    pub fn sync(&mut self) {
        let new_value_string = serde_json::to_string(&self.value).unwrap();
        self.write_to_disk(new_value_string);
    }

    fn load_from_content(contents: String) -> Result<S, RippleError> {
        match serde_json::from_str::<S>(&contents) {
            Ok(s) => Ok(s),
            Err(err) => {
                warn!("{:?} could not parse file content", err);
                Err(RippleError::ParseError)
            }
        }
    }

    pub fn load(path: String) -> Result<FileStore<S>, RippleError> {
        if let Ok(contents) = fs::read_to_string(&path) {
            if let Ok(s) = Self::load_from_content(contents.clone()) {
                debug!("valid filestore content {} from {}", contents, path);
                Ok(FileStore { value: s, path })
            } else {
                Err(RippleError::InvalidAccess)
            }
        } else {
            info!("No file found in {}", path);
            Err(RippleError::MissingInput)
        }
    }
}
