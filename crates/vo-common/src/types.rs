//! Type definitions for vo-common with strict type boundaries.
//!
//! This module provides strongly-typed wrappers around common types to prevent
//! type leakage. Each type is a newtype wrapper with validation and strict
//! interfaces that prevent accidental mixing of similar types.

use serde::{Deserialize, Serialize};
use std::fmt;

// ============================================================================
// InstanceId: Strictly validated instance identifier
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InstanceId(String);

impl InstanceId {
    const MIN_LEN: usize = 1;
    const MAX_LEN: usize = 256;

    /// Create a new InstanceId with validation.
    pub fn new(s: impl Into<String>) -> Result<Self, InstanceIdError> {
        let s = s.into();
        if s.len() < Self::MIN_LEN {
            return Err(InstanceIdError::TooShort);
        }
        if s.len() > Self::MAX_LEN {
            return Err(InstanceIdError::TooLong);
        }
        if !Self::is_valid(&s) {
            return Err(InstanceIdError::InvalidCharacters);
        }
        Ok(Self(s))
    }

    fn is_valid(s: &str) -> bool {
        s.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '/')
    }

    /// Get a reference to the underlying string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume self and return the underlying string.
    pub fn into_inner(self) -> String {
        self.0
    }

    /// Get the length of the identifier.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if the identifier is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for InstanceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for InstanceId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<InstanceId> for String {
    fn from(id: InstanceId) -> Self {
        id.into_inner()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceIdError {
    pub kind: InstanceIdErrorKind,
}

impl InstanceIdError {
    pub const TooShort: Self = Self { kind: InstanceIdErrorKind::TooShort };
    pub const TooLong: Self = Self { kind: InstanceIdErrorKind::TooLong };
    pub const InvalidCharacters: Self = Self { kind: InstanceIdErrorKind::InvalidCharacters };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceIdErrorKind {
    TooShort,
    TooLong,
    InvalidCharacters,
}

impl fmt::Display for InstanceIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            InstanceIdErrorKind::TooShort => write!(f, "InstanceId too short"),
            InstanceIdErrorKind::TooLong => write!(f, "InstanceId too long"),
            InstanceIdErrorKind::InvalidCharacters => write!(f, "InstanceId contains invalid characters"),
        }
    }
}

impl std::error::Error for InstanceIdError {}

impl From<InstanceIdErrorKind> for InstanceIdError {
    fn from(kind: InstanceIdErrorKind) -> Self {
        Self { kind }
    }
}

// ============================================================================
// NamespaceId: Strictly validated namespace identifier
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NamespaceId(String);

impl NamespaceId {
    const MIN_LEN: usize = 1;
    const MAX_LEN: usize = 256;

    /// Create a new NamespaceId with validation.
    pub fn new(s: impl Into<String>) -> Result<Self, NamespaceIdError> {
        let s = s.into();
        if s.len() < Self::MIN_LEN {
            return Err(NamespaceIdError::TooShort);
        }
        if s.len() > Self::MAX_LEN {
            return Err(NamespaceIdError::TooLong);
        }
        if !Self::is_valid(&s) {
            return Err(NamespaceIdError::InvalidCharacters);
        }
        Ok(Self(s))
    }

    fn is_valid(s: &str) -> bool {
        // NamespaceId allows dots, slashes, hyphens, underscores, alphanumeric
        s.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_' || c == '/')
    }

    /// Get a reference to the underlying string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume self and return the underlying string.
    pub fn into_inner(self) -> String {
        self.0
    }

    /// Get the length of the identifier.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if the identifier is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for NamespaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for NamespaceId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<NamespaceId> for String {
    fn from(ns: NamespaceId) -> Self {
        ns.into_inner()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespaceIdError {
    pub kind: NamespaceIdErrorKind,
}

impl NamespaceIdError {
    pub const TooShort: Self = Self { kind: NamespaceIdErrorKind::TooShort };
    pub const TooLong: Self = Self { kind: NamespaceIdErrorKind::TooLong };
    pub const InvalidCharacters: Self = Self { kind: NamespaceIdErrorKind::InvalidCharacters };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NamespaceIdErrorKind {
    TooShort,
    TooLong,
    InvalidCharacters,
}

impl fmt::Display for NamespaceIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            NamespaceIdErrorKind::TooShort => write!(f, "NamespaceId too short"),
            NamespaceIdErrorKind::TooLong => write!(f, "NamespaceId too long"),
            NamespaceIdErrorKind::InvalidCharacters => write!(f, "NamespaceId contains invalid characters"),
        }
    }
}

impl std::error::Error for NamespaceIdError {}

impl From<NamespaceIdErrorKind> for NamespaceIdError {
    fn from(kind: NamespaceIdErrorKind) -> Self {
        Self { kind }
    }
}

// ============================================================================
// TimerId: Strictly validated timer identifier
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimerId(String);

impl TimerId {
    const MIN_LEN: usize = 1;
    const MAX_LEN: usize = 256;

    /// Create a new TimerId with validation.
    pub fn new(s: impl Into<String>) -> Result<Self, TimerIdError> {
        let s = s.into();
        if s.len() < Self::MIN_LEN {
            return Err(TimerIdError::TooShort);
        }
        if s.len() > Self::MAX_LEN {
            return Err(TimerIdError::TooLong);
        }
        if !Self::is_valid(&s) {
            return Err(TimerIdError::InvalidCharacters);
        }
        Ok(Self(s))
    }

    fn is_valid(s: &str) -> bool {
        // TimerId allows alphanumeric, hyphens, underscores, colons (for time-like IDs)
        s.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == ':')
    }

    /// Get a reference to the underlying string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume self and return the underlying string.
    pub fn into_inner(self) -> String {
        self.0
    }

    /// Get the length of the identifier.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if the identifier is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for TimerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for TimerId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<TimerId> for String {
    fn from(timer: TimerId) -> Self {
        timer.into_inner()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerIdError {
    pub kind: TimerIdErrorKind,
}

impl TimerIdError {
    pub const TooShort: Self = Self { kind: TimerIdErrorKind::TooShort };
    pub const TooLong: Self = Self { kind: TimerIdErrorKind::TooLong };
    pub const InvalidCharacters: Self = Self { kind: TimerIdErrorKind::InvalidCharacters };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimerIdErrorKind {
    TooShort,
    TooLong,
    InvalidCharacters,
}

impl fmt::Display for TimerIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            TimerIdErrorKind::TooShort => write!(f, "TimerId too short"),
            TimerIdErrorKind::TooLong => write!(f, "TimerId too long"),
            TimerIdErrorKind::InvalidCharacters => write!(f, "TimerId contains invalid characters"),
        }
    }
}

impl std::error::Error for TimerIdError {}

impl From<TimerIdErrorKind> for TimerIdError {
    fn from(kind: TimerIdErrorKind) -> Self {
        Self { kind }
    }
}

// ============================================================================
// Public API re-exports
// ============================================================================

pub use errors::*;

mod errors {
    pub use super::{InstanceIdError, InstanceIdErrorKind, NamespaceIdError, NamespaceIdErrorKind, TimerIdError, TimerIdErrorKind};
}
