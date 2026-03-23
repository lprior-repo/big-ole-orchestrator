//! Error types for wtf-actor.

use serde::{Deserialize, Serialize};
use wtf_common::InstanceId;

/// Error starting a new workflow instance.
#[derive(Debug, Clone, thiserror::Error, Serialize, Deserialize)]
pub enum StartError {
    #[error("orchestrator is at capacity ({running}/{max} instances)")]
    AtCapacity { running: usize, max: usize },
    #[error("instance {0} already exists")]
    AlreadyExists(InstanceId),
    #[error("failed to spawn actor: {0}")]
    SpawnFailed(String),
    #[error("metadata persistence failed: {0}")]
    PersistenceFailed(String),
}

/// Error terminating a workflow instance.
#[derive(Debug, Clone, thiserror::Error, Serialize, Deserialize)]
pub enum TerminateError {
    #[error("instance not found: {0}")]
    NotFound(InstanceId),
    #[error("cancel timed out for instance: {0}")]
    Timeout(InstanceId),
}

/// Error querying instance status from the orchestrator.
#[derive(Debug, Clone, thiserror::Error, Serialize, Deserialize)]
pub enum GetStatusError {
    #[error("instance actor timed out")]
    Timeout,
    #[error("instance actor died or was killed")]
    ActorDied,
}

/// Error during heartbeat-driven crash recovery.
#[derive(Debug, Clone, thiserror::Error)]
pub enum RecoveryError {
    #[error("instance metadata not found in KV: {0}")]
    InstanceNotFound(InstanceId),
    #[error("failed to create replay consumer: {0}")]
    ReplayFailed(String),
    #[error("failed to spawn actor: {0}")]
    SpawnFailed(String),
    #[error("NATS client unavailable for recovery")]
    NoNatsClient,
}
