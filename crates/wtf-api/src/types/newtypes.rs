//! Strongly-typed wrappers for engine IDs and domain types.

#![allow(clippy::wrong_self_convention)]

use super::errors::{ParseError, ValidationError};
use chrono::{DateTime, Utc};
use serde::{de::Error as DeError, Deserialize, Deserializer, Serialize, Serializer};
use std::num::NonZeroU64;

/// Compile-time enforcement for workflow_name pattern `[a-z][a-z0-9_]*`
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WorkflowName(String);

impl WorkflowName {
    pub fn new(s: impl AsRef<str>) -> Result<Self, ParseError> {
        let s = s.as_ref();
        if s.is_empty() {
            return Err(ParseError::EmptyWorkflowName);
        }
        let re = workflow_name_regex().map_err(|_| ParseError::InvalidWorkflowNameFormat)?;
        if !re.is_match(s) {
            return Err(ParseError::InvalidWorkflowNameFormat);
        }
        Ok(Self(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for WorkflowName {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for WorkflowName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::new(&s).map_err(|e| D::Error::custom(e.to_string()))
    }
}

fn workflow_name_regex() -> Result<&'static regex::Regex, String> {
    static RE: std::sync::OnceLock<Result<regex::Regex, String>> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"^[a-z][a-z0-9_]*$")
            .map_err(|e| format!("workflow_name regex failed: {e}"))
    })
    .as_ref()
    .map_err(Clone::clone)
}

/// Compile-time enforcement for signal_name pattern `[a-z][a-z0-9_]+`
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SignalName(String);

impl SignalName {
    pub fn new(s: impl AsRef<str>) -> Result<Self, ParseError> {
        let s = s.as_ref();
        if s.is_empty() {
            return Err(ParseError::EmptySignalName);
        }
        let re = signal_name_regex().map_err(|_| ParseError::InvalidSignalNameFormat)?;
        if !re.is_match(s) {
            return Err(ParseError::InvalidSignalNameFormat);
        }
        Ok(Self(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for SignalName {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for SignalName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::new(&s).map_err(|e| D::Error::custom(e.to_string()))
    }
}

fn signal_name_regex() -> Result<&'static regex::Regex, String> {
    static RE: std::sync::OnceLock<Result<regex::Regex, String>> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"^[a-z][a-z0-9_]+$")
            .map_err(|e| format!("signal_name regex failed: {e}"))
    })
    .as_ref()
    .map_err(Clone::clone)
}

/// Compile-time enforcement for ULID format (26 chars, Crockford base32)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvocationId(String);

impl InvocationId {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: impl AsRef<str>) -> Result<Self, ParseError> {
        let s = s.as_ref();
        let re = ulid_regex().map_err(|_| ParseError::InvalidUlidFormat)?;
        if s.len() != 26 || !re.is_match(s) {
            return Err(ParseError::InvalidUlidFormat);
        }
        Ok(Self(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for InvocationId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for InvocationId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::from_str(&s).map_err(|e| D::Error::custom(e.to_string()))
    }
}

fn ulid_regex() -> Result<&'static regex::Regex, String> {
    static RE: std::sync::OnceLock<Result<regex::Regex, String>> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"^[0-9A-HJKMNP-TV-Z]{26}$")
            .map_err(|e| format!("ulid regex failed: {e}"))
    })
    .as_ref()
    .map_err(Clone::clone)
}

/// Compile-time enforcement for retry_after_seconds > 0
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetryAfterSeconds(NonZeroU64);

impl RetryAfterSeconds {
    pub fn new(seconds: u64) -> Result<Self, ValidationError> {
        NonZeroU64::new(seconds)
            .map(Self)
            .ok_or(ValidationError::InvalidRetryAfterSeconds)
    }

    pub fn get(&self) -> u64 {
        self.0.get()
    }
}

impl Serialize for RetryAfterSeconds {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(self.0.get())
    }
}

impl<'de> Deserialize<'de> for RetryAfterSeconds {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = u64::deserialize(deserializer)?;
        Self::new(s).map_err(|e| D::Error::custom(e.to_string()))
    }
}

/// RFC3339 timestamp wrapper
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Timestamp(String);

impl Timestamp {
    pub fn new(s: impl AsRef<str>) -> Result<Self, ParseError> {
        let s = s.as_ref();
        DateTime::parse_from_rfc3339(s).map_err(|_| ParseError::InvalidTimestampFormat)?;
        Ok(Self(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn as_datetime(&self) -> Option<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(&self.0)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::new(&s).map_err(|e| D::Error::custom(e.to_string()))
    }
}
