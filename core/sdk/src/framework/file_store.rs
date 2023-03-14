use log::{info, warn};
use serde::{de::DeserializeOwned, Serialize};
use std::fs::{self};

use crate::utils::error::RippleError;


#[derive(Debug, Clone)]
pub struct FileStore<S> {
    pub value: S,
    path: String,
}

impl<S> FileStore<S>
where
    S: Serialize + DeserializeOwned + Clone,
{
    pub fn new(path: String, value: S) -> FileStore<S> {
        FileStore {
            value: value.clone(),
            path,
        }
    }

    pub fn update(&mut self, new_value: S) {
        let new_value_string = serde_json::to_string(&new_value).unwrap();
        if let Err(_) = fs::write(self.path.clone(), new_value_string) {
            warn!("Failed to file store for {}", self.path);
        }
        self.value = new_value;
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
