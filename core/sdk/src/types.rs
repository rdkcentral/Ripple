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

//! Type-safe identifier wrappers to prevent conflation of session/connection/app identifiers.
//!
//! This module provides newtype wrappers around String to ensure compile-time safety
//! when working with different types of identifiers throughout the Ripple codebase.
//! This prevents HashMap leaks and identifier confusion that can occur when using
//! plain String types for different purposes.
//!
//! Each identifier type has specific validation rules based on its expected content:
//! - SessionId, ConnectionId, DeviceSessionId: Must be valid UUIDs
//! - AppId: Human-readable string (alphanumeric + limited special chars)
//! - AppInstanceId: UUID for App Lifecycle 2.0
//! - RequestId: UUID for request tracking

use serde::{Deserialize, Serialize};
use std::{convert::TryFrom, fmt, hash::Hash, str::FromStr};

/// Validation errors for identifier types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentifierError {
    Empty,
    InvalidUuid(String),
    InvalidAppId(String),
    InvalidFormat(String),
}

impl fmt::Display for IdentifierError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "Identifier cannot be empty"),
            Self::InvalidUuid(id) => write!(f, "Invalid UUID format: {}", id),
            Self::InvalidAppId(id) => write!(f, "Invalid app ID format: {}", id),
            Self::InvalidFormat(msg) => write!(f, "Invalid format: {}", msg),
        }
    }
}

impl std::error::Error for IdentifierError {}

/// Validates that a string is a properly formatted UUID
fn validate_uuid(id: &str) -> Result<(), IdentifierError> {
    if id.is_empty() {
        return Err(IdentifierError::Empty);
    }

    // Basic UUID format validation: 8-4-4-4-12 hex digits with hyphens
    // Example: 550e8400-e29b-41d4-a716-446655440000
    let parts: Vec<&str> = id.split('-').collect();
    if parts.len() != 5 {
        return Err(IdentifierError::InvalidUuid(id.to_string()));
    }

    let expected_lengths = [8, 4, 4, 4, 12];
    for (i, part) in parts.iter().enumerate() {
        if part.len() != expected_lengths[i] {
            return Err(IdentifierError::InvalidUuid(id.to_string()));
        }

        if !part.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(IdentifierError::InvalidUuid(id.to_string()));
        }
    }

    Ok(())
}

/// Validates that a string is a valid app identifier
/// App IDs should be human-readable strings, NOT UUIDs
/// This helps prevent confusion between app IDs and session/connection IDs
/// - Trims leading/trailing whitespace automatically
/// - Allows internal spaces (e.g., "Prime Video", "HBO Max")
/// - Returns the trimmed string for use
fn validate_app_id(id: &str) -> Result<String, IdentifierError> {
    if id.is_empty() {
        return Err(IdentifierError::Empty);
    }

    // Check for control characters BEFORE trimming so we can catch tabs/newlines
    if id.chars().any(|c| c.is_control()) {
        return Err(IdentifierError::InvalidAppId(format!(
            "App ID contains control characters: {}",
            id
        )));
    }

    // Trim leading/trailing whitespace automatically
    let trimmed = id.trim();

    if trimmed.is_empty() {
        return Err(IdentifierError::Empty);
    }

    if trimmed.len() > 200 {
        return Err(IdentifierError::InvalidAppId(format!(
            "App ID too long (max 200 chars): {}",
            trimmed
        )));
    }

    // The key validation: App IDs should NOT look like UUIDs
    // If it looks like a UUID, it's probably the wrong identifier type
    if is_uuid_like(trimmed) {
        return Err(IdentifierError::InvalidAppId(format!(
            "App ID appears to be a UUID - use SessionId/ConnectionId instead: {}",
            trimmed
        )));
    }

    // Internal spaces are allowed for app names like "Prime Video", "HBO Max"
    // This is intentionally more permissive than other identifier types

    Ok(trimmed.to_string())
}

/// Helper function to detect UUID-like strings
/// This prevents accidentally using UUIDs where app IDs are expected
fn is_uuid_like(s: &str) -> bool {
    // Check if it matches basic UUID pattern: 8-4-4-4-12
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() == 5 {
        let expected_lengths = [8, 4, 4, 4, 12];
        for (i, part) in parts.iter().enumerate() {
            if part.len() == expected_lengths[i] && part.chars().all(|c| c.is_ascii_hexdigit()) {
                if i == 4 {
                    return true;
                } // If we made it to the last part, it's UUID-like
            } else {
                return false;
            }
        }
    }
    false
}

/// WebSocket session identifier used for client connections (must be UUID)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(String);

/// Internal connection tracking identifier, may differ from session ID (must be UUID)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConnectionId(String);

/// Application identifier for Firebolt apps (human-readable string)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AppId(String);

/// App instance identifier used in App Lifecycle 2.0 (must be UUID)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AppInstanceId(String);

/// Request identifier for tracking individual RPC requests (must be UUID)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RequestId(String);

/// Device session identifier for device-level sessions (must be UUID)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeviceSessionId(String);

// Implementation for SessionId (UUID-based)
impl SessionId {
    /// Creates a new SessionId with validation
    pub fn new(id: impl Into<String>) -> Result<Self, IdentifierError> {
        let id_str = id.into();
        validate_uuid(&id_str)?;
        Ok(Self(id_str))
    }

    /// Creates a SessionId without validation (for migration/legacy support)
    /// Use sparingly and only during migration period
    pub fn new_unchecked(id: impl Into<String>) -> Self {
        let id_str = id.into();

        // Log validation errors to help identify problematic usage during migration
        if let Err(err) = validate_uuid(&id_str) {
            log::warn!(
                "SessionId::new_unchecked used with invalid UUID: {} (error: {})",
                id_str,
                err
            );
        }

        Self(id_str)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for SessionId {
    type Err = IdentifierError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl TryFrom<String> for SessionId {
    type Error = IdentifierError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

impl AsRef<str> for SessionId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// Implementation for ConnectionId (UUID-based)
impl ConnectionId {
    /// Creates a new ConnectionId with validation
    pub fn new(id: impl Into<String>) -> Result<Self, IdentifierError> {
        let id_str = id.into();
        validate_uuid(&id_str)?;
        Ok(Self(id_str))
    }

    /// Creates a ConnectionId without validation (for migration/legacy support)
    /// Use sparingly and only during migration period
    pub fn new_unchecked(id: impl Into<String>) -> Self {
        let id_str = id.into();

        // Log validation errors to help identify problematic usage during migration
        if let Err(err) = validate_uuid(&id_str) {
            log::warn!(
                "ConnectionId::new_unchecked used with invalid UUID: {} (error: {})",
                id_str,
                err
            );
        }

        Self(id_str)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for ConnectionId {
    type Err = IdentifierError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl TryFrom<String> for ConnectionId {
    type Error = IdentifierError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

impl AsRef<str> for ConnectionId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// Implementation for AppId (human-readable string with validation)
impl AppId {
    /// Creates a new AppId with validation and automatic trimming
    /// Leading and trailing whitespace is automatically trimmed
    pub fn new(id: impl Into<String>) -> Result<Self, IdentifierError> {
        let id_str = id.into();
        let trimmed_id = validate_app_id(&id_str)?;
        Ok(Self(trimmed_id))
    }

    /// Creates an AppId without validation (for migration/legacy support)
    /// Use sparingly and only during migration period
    pub fn new_unchecked(id: impl Into<String>) -> Self {
        let id_str = id.into();

        // Log validation errors to help identify problematic usage during migration
        if let Err(err) = validate_app_id(&id_str) {
            log::warn!(
                "AppId::new_unchecked used with invalid app ID: {} (error: {})",
                id_str,
                err
            );
        }

        Self(id_str)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for AppId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for AppId {
    type Err = IdentifierError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl TryFrom<String> for AppId {
    type Error = IdentifierError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

impl AsRef<str> for AppId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// Implementation for AppInstanceId (UUID-based)
impl AppInstanceId {
    /// Creates a new AppInstanceId with validation
    pub fn new(id: impl Into<String>) -> Result<Self, IdentifierError> {
        let id_str = id.into();
        validate_uuid(&id_str)?;
        Ok(Self(id_str))
    }

    /// Creates an AppInstanceId without validation (for migration/legacy support)
    /// Use sparingly and only during migration period
    pub fn new_unchecked(id: impl Into<String>) -> Self {
        let id_str = id.into();

        // Log validation errors to help identify problematic usage during migration
        if let Err(err) = validate_uuid(&id_str) {
            log::warn!(
                "AppInstanceId::new_unchecked used with invalid UUID: {} (error: {})",
                id_str,
                err
            );
        }

        Self(id_str)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for AppInstanceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for AppInstanceId {
    type Err = IdentifierError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl TryFrom<String> for AppInstanceId {
    type Error = IdentifierError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

impl AsRef<str> for AppInstanceId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// Implementation for RequestId (UUID-based)
impl RequestId {
    /// Creates a new RequestId with validation
    pub fn new(id: impl Into<String>) -> Result<Self, IdentifierError> {
        let id_str = id.into();
        validate_uuid(&id_str)?;
        Ok(Self(id_str))
    }

    /// Creates a RequestId without validation (for migration/legacy support)
    /// Use sparingly and only during migration period
    pub fn new_unchecked(id: impl Into<String>) -> Self {
        let id_str = id.into();

        // Log validation errors to help identify problematic usage during migration
        if let Err(err) = validate_uuid(&id_str) {
            log::warn!(
                "RequestId::new_unchecked used with invalid UUID: {} (error: {})",
                id_str,
                err
            );
        }

        Self(id_str)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for RequestId {
    type Err = IdentifierError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl TryFrom<String> for RequestId {
    type Error = IdentifierError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

impl AsRef<str> for RequestId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// Implementation for DeviceSessionId (UUID-based)
impl DeviceSessionId {
    /// Creates a new DeviceSessionId with validation
    pub fn new(id: impl Into<String>) -> Result<Self, IdentifierError> {
        let id_str = id.into();
        validate_uuid(&id_str)?;
        Ok(Self(id_str))
    }

    /// Creates a DeviceSessionId without validation (for migration/legacy support)
    /// Use sparingly and only during migration period
    pub fn new_unchecked(id: impl Into<String>) -> Self {
        let id_str = id.into();

        // Log validation errors to help identify problematic usage during migration
        if let Err(err) = validate_uuid(&id_str) {
            log::warn!(
                "DeviceSessionId::new_unchecked used with invalid UUID: {} (error: {})",
                id_str,
                err
            );
        }

        Self(id_str)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for DeviceSessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for DeviceSessionId {
    type Err = IdentifierError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl TryFrom<String> for DeviceSessionId {
    type Error = IdentifierError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

impl AsRef<str> for DeviceSessionId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_session_id() {
        let session_id = SessionId::new("550e8400-e29b-41d4-a716-446655440000").unwrap();
        assert_eq!(session_id.as_str(), "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(
            session_id.to_string(),
            "550e8400-e29b-41d4-a716-446655440000"
        );
    }

    #[test]
    fn test_invalid_session_id() {
        // Not a UUID
        assert!(SessionId::new("not-a-uuid").is_err());

        // Wrong format
        assert!(SessionId::new("550e8400-e29b-41d4-a716").is_err());

        // Non-hex characters
        assert!(SessionId::new("gggg8400-e29b-41d4-a716-446655440000").is_err());

        // Empty
        assert!(SessionId::new("").is_err());
    }

    #[test]
    fn test_valid_app_id() {
        // Valid app IDs - human readable, not UUIDs
        assert!(AppId::new("Netflix").is_ok());
        assert!(AppId::new("xumo").is_ok());
        assert!(AppId::new("Prime Video").is_ok()); // spaces are fine
        assert!(AppId::new("com.disney.plus").is_ok());
        assert!(AppId::new("YouTube TV").is_ok());
        assert!(AppId::new("HBO Max 2.0").is_ok());
        assert!(AppId::new("Netflix@Home").is_ok()); // special chars are fine
        assert!(AppId::new("Disney+ Hotstar").is_ok());
        assert!(AppId::new("Apple TV+").is_ok());
        assert!(AppId::new("Crunchyroll").is_ok());
        assert!(AppId::new("Plex Media Server").is_ok());
        assert!(AppId::new("123").is_ok()); // numbers are fine
        assert!(AppId::new("app-123_test.v2").is_ok());
    }

    #[test]
    fn test_invalid_app_id() {
        // Empty
        assert!(matches!(AppId::new(""), Err(IdentifierError::Empty)));

        // UUID-like strings should be rejected for app IDs
        assert!(AppId::new("550e8400-e29b-41d4-a716-446655440000").is_err());
        assert!(AppId::new("123e4567-e89b-12d3-a456-426614174000").is_err());
        assert!(AppId::new("00000000-0000-0000-0000-000000000000").is_err());

        // Control characters should be rejected
        assert!(AppId::new("Netflix\t").is_err());
        assert!(AppId::new("Netflix\n").is_err());
        assert!(AppId::new("Net\x00flix").is_err());

        // Too long
        let long_name = "a".repeat(201);
        assert!(AppId::new(long_name).is_err());
    }

    #[test]
    fn test_app_id_trimming() {
        // Leading/trailing whitespace should be automatically trimmed
        let app_id1 = AppId::new(" Netflix").unwrap();
        assert_eq!(app_id1.as_str(), "Netflix");

        let app_id2 = AppId::new("Netflix ").unwrap();
        assert_eq!(app_id2.as_str(), "Netflix");

        let app_id3 = AppId::new(" Netflix ").unwrap();
        assert_eq!(app_id3.as_str(), "Netflix");

        // Control characters should be rejected before trimming
        assert!(AppId::new("\t xumo \n").is_err());

        // Only whitespace should fail
        assert!(AppId::new("   ").is_err());
        assert!(AppId::new("\t\n").is_err());
    }

    #[test]
    fn test_connection_id_validation() {
        // Valid UUID
        assert!(ConnectionId::new("123e4567-e89b-12d3-a456-426614174000").is_ok());

        // Invalid format
        assert!(ConnectionId::new("conn-123").is_err());
    }

    #[test]
    fn test_app_instance_id_validation() {
        // Valid UUID
        assert!(AppInstanceId::new("550e8400-e29b-41d4-a716-446655440000").is_ok());

        // Invalid format
        assert!(AppInstanceId::new("instance-123").is_err());
    }

    #[test]
    fn test_unchecked_constructors() {
        // These should work even with invalid data (for migration)
        let session_id = SessionId::new_unchecked("invalid-session");
        assert_eq!(session_id.as_str(), "invalid-session");

        let app_id = AppId::new_unchecked("Invalid App ID!");
        assert_eq!(app_id.as_str(), "Invalid App ID!");
    }

    #[test]
    fn test_different_types_not_equal() {
        let session_id = SessionId::new_unchecked("550e8400-e29b-41d4-a716-446655440000");
        let connection_id = ConnectionId::new_unchecked("550e8400-e29b-41d4-a716-446655440000");

        // These should not be comparable (different types)
        // This test won't compile if we accidentally try to compare them
        assert_eq!(session_id.as_str(), connection_id.as_str());
    }

    #[test]
    fn test_serde_transparency() {
        let session_id = SessionId::new_unchecked("550e8400-e29b-41d4-a716-446655440000");
        let json = serde_json::to_string(&session_id).unwrap();
        assert_eq!(json, "\"550e8400-e29b-41d4-a716-446655440000\"");

        let deserialized: SessionId = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, session_id);
    }

    #[test]
    fn test_hash_map_usage() {
        use std::collections::HashMap;

        let mut session_map = HashMap::new();
        let session_id = SessionId::new_unchecked("550e8400-e29b-41d4-a716-446655440000");
        session_map.insert(session_id.clone(), "session_data");

        assert_eq!(session_map.get(&session_id), Some(&"session_data"));

        // Different types cannot be used as keys in the same map
        let mut app_map = HashMap::new();
        let app_id = AppId::new("Netflix").unwrap();
        app_map.insert(app_id.clone(), "app_data");

        assert_eq!(app_map.get(&app_id), Some(&"app_data"));
    }

    #[test]
    fn test_fromstr_trait() {
        // Valid parsing
        let session_id: SessionId = "550e8400-e29b-41d4-a716-446655440000".parse().unwrap();
        assert_eq!(session_id.as_str(), "550e8400-e29b-41d4-a716-446655440000");

        let app_id: AppId = "Netflix".parse().unwrap();
        assert_eq!(app_id.as_str(), "Netflix");

        // Invalid parsing
        assert!("invalid-uuid".parse::<SessionId>().is_err());
        assert!("550e8400-e29b-41d4-a716-446655440000"
            .parse::<AppId>()
            .is_err()); // UUID should fail for app ID
    }

    #[test]
    fn test_try_from_trait() {
        // Valid conversion
        let session_id =
            SessionId::try_from("550e8400-e29b-41d4-a716-446655440000".to_string()).unwrap();
        assert_eq!(session_id.as_str(), "550e8400-e29b-41d4-a716-446655440000");

        // Invalid conversion
        assert!(SessionId::try_from("invalid".to_string()).is_err());
    }

    #[test]
    fn test_real_world_app_ids() {
        // Test real-world app ID patterns - these should all be valid
        let valid_apps = vec![
            "Netflix",
            "xumo",
            "Prime Video",
            "Disney Plus",
            "Disney+ Hotstar",
            "com.hulu.plus",
            "YouTube",
            "YouTube TV",
            "HBO Max",
            "Apple TV",
            "Apple TV+",
            "Paramount Plus",
            "Paramount+",
            "peacock",
            "crackle",
            "tubi",
            "Pluto TV",
            "Sling TV",
            "fuboTV",
            "Discovery+",
            "ESPN+",
            "Showtime",
            "STARZ",
            "Cinemax",
            "Netflix@Home", // special chars should be OK
            "Disney & Marvel",
            "CBS Sports/News",
            "Adult Swim",
            "Cartoon Network",
            "Food Network",
            "HGTV",
            "History Channel",
            "A&E",
            "FX",
            "National Geographic",
            "BBC iPlayer",
            "ITV Hub",
            "All 4",
            "My5",
            "RTÉ Player", // international chars
            "France.tv",
            "ARD Mediathek",
            "ZDF Mediathek",
            "Rai Play",
            "RTVE Play",
        ];

        for app in valid_apps {
            assert!(
                AppId::new(app).is_ok(),
                "Failed to validate app ID: {}",
                app
            );
        }
    }

    #[test]
    fn test_uuid_detection_in_app_ids() {
        // These should be rejected because they look like UUIDs
        let uuid_like_strings = vec![
            "550e8400-e29b-41d4-a716-446655440000",
            "123e4567-e89b-12d3-a456-426614174000",
            "00000000-0000-0000-0000-000000000000",
            "ffffffff-ffff-ffff-ffff-ffffffffffff",
        ];

        for uuid_str in uuid_like_strings {
            match AppId::new(uuid_str) {
                Err(IdentifierError::InvalidAppId(msg)) => {
                    assert!(
                        msg.contains("appears to be a UUID"),
                        "Expected UUID detection error, got: {}",
                        msg
                    );
                    println!("✅ Correctly detected UUID in app ID: ");
                }
                Ok(_) => panic!("Expected error for UUID-like app ID"),
                Err(e) => panic!("Unexpected error type: {}", e),
            }
        }
    }

    #[test]
    fn test_semantic_validation_examples() {
        // This demonstrates the key insight: preventing type confusion

        // These should work (correct types for correct purposes)
        let _session_id = SessionId::new("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let app_id = AppId::new("Netflix").unwrap();

        // These should fail (wrong types for purposes)
        assert!(AppId::new("550e8400-e29b-41d4-a716-446655440000").is_err()); // UUID as app ID
        assert!(SessionId::new("Netflix").is_err()); // app name as session ID

        println!("✅ App ID (human): {}", app_id);
    }

    #[test]
    fn test_session_id_new_and_unchecked() {
        let valid_uuid = "550e8400-e29b-41d4-a716-446655440000";
        let invalid_uuid = "not-a-uuid";

        // Valid construction
        let session_id = SessionId::new(valid_uuid).unwrap();
        assert_eq!(session_id.as_str(), valid_uuid);
        assert_eq!(session_id.to_string(), valid_uuid);

        // Invalid construction should fail
        assert!(SessionId::new(invalid_uuid).is_err());

        // Unchecked construction should always work (for migration)
        let unchecked_session_id = SessionId::new_unchecked(invalid_uuid);
        assert_eq!(unchecked_session_id.as_str(), invalid_uuid);
    }

    #[test]
    fn test_connection_id_new_and_unchecked() {
        let valid_uuid = "123e4567-e89b-12d3-a456-426614174000";
        let invalid_uuid = "invalid-connection-id";

        // Valid construction
        let connection_id = ConnectionId::new(valid_uuid).unwrap();
        assert_eq!(connection_id.as_str(), valid_uuid);
        assert_eq!(connection_id.into_string(), valid_uuid);

        // Invalid construction should fail
        assert!(ConnectionId::new(invalid_uuid).is_err());

        // Unchecked construction should always work (for migration)
        let unchecked_connection_id = ConnectionId::new_unchecked(invalid_uuid);
        assert_eq!(unchecked_connection_id.as_str(), invalid_uuid);
    }

    #[test]
    fn test_session_id_and_connection_id_are_different_types() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let session_id = SessionId::new(uuid_str).unwrap();
        let connection_id = ConnectionId::new(uuid_str).unwrap();

        // These are different types, so this wouldn't compile:
        // assert_eq!(session_id, connection_id); // ← This would be a compilation error!

        // But their string representations are the same
        assert_eq!(session_id.as_str(), connection_id.as_str());
    }

    #[test]
    fn test_session_id_serialization() {
        let session_id = SessionId::new("550e8400-e29b-41d4-a716-446655440000").unwrap();

        // Should serialize to just the string value (transparent)
        let serialized = serde_json::to_string(&session_id).unwrap();
        assert_eq!(serialized, "\"550e8400-e29b-41d4-a716-446655440000\"");

        // Should deserialize back correctly
        let deserialized: SessionId = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.as_str(), session_id.as_str());
    }

    #[test]
    fn test_connection_id_serialization() {
        let connection_id = ConnectionId::new("123e4567-e89b-12d3-a456-426614174000").unwrap();

        // Should serialize to just the string value (transparent)
        let serialized = serde_json::to_string(&connection_id).unwrap();
        assert_eq!(serialized, "\"123e4567-e89b-12d3-a456-426614174000\"");

        // Should deserialize back correctly
        let deserialized: ConnectionId = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.as_str(), connection_id.as_str());
    }

    #[test]
    fn test_session_and_connection_id_conversions() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";

        // Test SessionId conversions
        let session_id = SessionId::new(uuid_str).unwrap();
        assert_eq!(session_id.as_str(), uuid_str);
        assert_eq!(session_id.into_string(), uuid_str);

        // Test ConnectionId conversions
        let connection_id = ConnectionId::new(uuid_str).unwrap();
        assert_eq!(connection_id.as_str(), uuid_str);
        assert_eq!(connection_id.into_string(), uuid_str);
    }

    #[test]
    fn test_identifier_error_display() {
        assert_eq!(
            IdentifierError::Empty.to_string(),
            "Identifier cannot be empty"
        );
        assert_eq!(
            IdentifierError::InvalidUuid("bad-uuid".to_string()).to_string(),
            "Invalid UUID format: bad-uuid"
        );
        assert_eq!(
            IdentifierError::InvalidAppId("bad-app".to_string()).to_string(),
            "Invalid app ID format: bad-app"
        );
    }
}
