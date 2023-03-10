use log::{error, info, warn};
use serde::{de::DeserializeOwned, Serialize};
use std::fs::{self};

use crate::utils::error::RippleError;

use super::RippleResponse;

#[derive(Debug, Clone)]
pub struct FileStore<S> {
    pub value: S,
    path: String,
}

impl<S> FileStore<S>
where
    S: Serialize + DeserializeOwned + Clone,
{
    pub fn new(path: String, value: S) -> Result<FileStore<S>, RippleError> {
        let mut file_cache = FileStore {
            value: value.clone(),
            path,
        };
        if let Err(e) = file_cache.update(value) {
            return Err(e);
        }
        Ok(file_cache)
    }

    pub fn update(&mut self, new_value: S) -> RippleResponse {
        if let Ok(json) = serde_json::to_string(&new_value) {
            if let Err(_) = fs::write(self.path.clone(), json) {
                error!("Failed to file store for {}", self.path);
                return Err(RippleError::InvalidAccess);
            }
            self.value = new_value;
        } else {
            error!("Failed to serialize app permissions for {}", self.path);
            return Err(RippleError::ParseError);
        }
        Ok(())
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
            if let Ok(s) = Self::load_from_content(contents) {
                return Ok(FileStore { value: s, path });
            } else {
                return Err(RippleError::InvalidAccess);
            }
        } else {
            info!("No file found in {}", path);
            return Err(RippleError::MissingInput);
        }
    }
}
