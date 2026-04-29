//! Error types for vo-common.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Error)]
pub enum VoError {
    #[error("configuration error: {0}")]
    Config(String),
    #[error("internal error: {0}")]
    Internal(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("operation timed out: {0}")]
    Timeout(String),
}

impl From<std::io::Error> for VoError {
    fn from(err: std::io::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

impl From<serde_json::Error> for VoError {
    fn from(err: serde_json::Error) -> Self {
        Self::Validation(err.to_string())
    }
}

impl VoError {
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }

    pub fn timeout(msg: impl Into<String>) -> Self {
        Self::Timeout(msg.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vo_error_config_constructs() {
        let err = VoError::config("bad config");
        assert!(matches!(err, VoError::Config(msg) if msg == "bad config"));
    }

    #[test]
    fn vo_error_internal_constructs() {
        let err = VoError::internal("oops");
        assert!(matches!(err, VoError::Internal(msg) if msg == "oops"));
    }

    #[test]
    fn vo_error_not_found_constructs() {
        let err = VoError::not_found("missing");
        assert!(matches!(err, VoError::NotFound(msg) if msg == "missing"));
    }

    #[test]
    fn vo_error_validation_constructs() {
        let err = VoError::validation("invalid");
        assert!(matches!(err, VoError::Validation(msg) if msg == "invalid"));
    }

    #[test]
    fn vo_error_timeout_constructs() {
        let err = VoError::timeout("30s");
        assert!(matches!(err, VoError::Timeout(msg) if msg == "30s"));
    }

    #[test]
    fn vo_error_displays_message() {
        let err = VoError::Internal("something went wrong".to_string());
        let msg = err.to_string();
        assert!(msg.contains("something went wrong"));
    }
}
