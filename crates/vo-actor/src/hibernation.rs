//! Actor hibernation serialization tests.
//!
//! Tests the hibernate() and thaw() functions for actor state serialization:
//! 1. Round-trip: hibernate then thaw preserves state
//! 2. Corruption: thaw with corrupt bytes returns DeserializationFailed
//! 3. Idempotency: hibernate twice without thaw returns Ok

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActorStatus {
    Paused,
    Running,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActorState {
    pub status: ActorStatus,
    pub sequence: u64,
}

#[derive(Debug, Error)]
pub enum HibernationError {
    #[error("deserialization failed: {0}")]
    DeserializationFailed(String),
}

pub fn hibernate(state: &ActorState) -> Result<Vec<u8>, HibernationError> {
    serde_json::to_vec(state).map_err(|e| HibernationError::DeserializationFailed(e.to_string()))
}

pub fn thaw(bytes: &[u8]) -> Result<ActorState, HibernationError> {
    serde_json::from_slice(bytes).map_err(|e| HibernationError::DeserializationFailed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hibernate_thaw_roundtrip() {
        let state = ActorState {
            status: ActorStatus::Paused,
            sequence: 42,
        };

        let bytes = hibernate(&state).expect("hibernate should succeed");
        let thawed = thaw(&bytes).expect("thaw should succeed");

        assert_eq!(thawed.status, ActorStatus::Paused);
        assert_eq!(thawed.sequence, 42);
    }

    #[test]
    fn test_thaw_corrupt_bytes() {
        let corrupt_bytes = b"not valid json at all!!!";
        let result = thaw(corrupt_bytes);
        assert!(matches!(result, Err(HibernationError::DeserializationFailed(_))));
    }

    #[test]
    fn test_hibernate_idempotent() {
        let state = ActorState {
            status: ActorStatus::Paused,
            sequence: 42,
        };

        let first = hibernate(&state).expect("first hibernate should succeed");
        let second = hibernate(&state).expect("second hibernate should succeed");

        assert_eq!(first, second);
    }
}