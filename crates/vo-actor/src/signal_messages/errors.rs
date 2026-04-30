use vo_types::{InstanceId, NodeName};

use super::signal_in::{LifecycleState, SecretId, SignalName, SignalPayload, WaitKey};
use super::registry::{SignalStorage, SignalWorkQueue};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AcceptResumeError {
    #[error("invalid lifecycle state for {instance_id}: got {actual:?}, expected {expected:?}")]
    InvalidLifecycleState {
        instance_id: InstanceId,
        actual: LifecycleState,
        expected: LifecycleState,
    },
    #[error(
        "wait key mismatch for {instance_id}: expected {expected_key:?}, got {provided_key:?}"
    )]
    WaitKeyMismatch {
        instance_id: InstanceId,
        expected_key: WaitKey,
        provided_key: WaitKey,
    },
    #[error("instance actor not found: {instance_id}")]
    InstanceActorNotFound { instance_id: InstanceId },
    #[error("payload too large for {instance_id}: {payload_size} > {max_size}")]
    PayloadTooLarge {
        instance_id: InstanceId,
        payload_size: usize,
        max_size: usize,
    },
    #[error("lock acquisition failed for {instance_id}: {reason}")]
    LockAcquisitionFailed {
        instance_id: InstanceId,
        reason: String,
    },
    #[error("storage error for {instance_id}: {reason}")]
    StorageError {
        instance_id: InstanceId,
        reason: String,
    },
}

impl AcceptResumeError {
    pub const fn is_precondition(&self) -> bool {
        matches!(
            self,
            Self::InvalidLifecycleState { .. }
                | Self::WaitKeyMismatch { .. }
                | Self::InstanceActorNotFound { .. }
                | Self::PayloadTooLarge { .. }
        )
    }

    pub const fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::LockAcquisitionFailed { .. } | Self::StorageError { .. }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CancelError {
    #[error("already terminal for {instance_id}: {current_state:?}")]
    AlreadyTerminal {
        instance_id: InstanceId,
        current_state: LifecycleState,
    },
    #[error("instance actor not found: {instance_id}")]
    InstanceActorNotFound { instance_id: InstanceId },
    #[error("lock acquisition failed for {instance_id}: {reason}")]
    LockAcquisitionFailed {
        instance_id: InstanceId,
        reason: String,
    },
    #[error("storage error for {instance_id}: {reason}")]
    StorageError {
        instance_id: InstanceId,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResumeError {
    #[error("invalid lifecycle state: got {actual:?}, expected {expected:?}")]
    InvalidLifecycleState {
        actual: LifecycleState,
        expected: LifecycleState,
    },
    #[error("missing secrets for {instance_id}: {missing_secret_ids:?}")]
    MissingSecrets {
        instance_id: InstanceId,
        missing_secret_ids: Vec<SecretId>,
    },
    #[error("node not found for {instance_id}: {node_name:?}")]
    NodeNotFound {
        instance_id: InstanceId,
        node_name: NodeName,
    },
    #[error("no path to terminal from {current_node:?} for {instance_id}")]
    NoPathToTerminal {
        instance_id: InstanceId,
        current_node: NodeName,
    },
    #[error("instance actor not found: {instance_id}")]
    InstanceActorNotFound { instance_id: InstanceId },
    #[error("lock acquisition failed for {instance_id}: {reason}")]
    LockAcquisitionFailed {
        instance_id: InstanceId,
        reason: String,
    },
    #[error("storage error for {instance_id}: {reason}")]
    StorageError {
        instance_id: InstanceId,
        reason: String,
    },
}

impl ResumeError {
    pub const fn is_precondition(&self) -> bool {
        matches!(
            self,
            Self::InvalidLifecycleState { .. }
                | Self::MissingSecrets { .. }
                | Self::NodeNotFound { .. }
                | Self::NoPathToTerminal { .. }
                | Self::InstanceActorNotFound { .. }
        )
    }

    pub const fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::LockAcquisitionFailed { .. } | Self::StorageError { .. }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ContinueAsNewError {
    #[error("instance actor not found: {instance_id}")]
    InstanceActorNotFound { instance_id: InstanceId },
    #[error("instance is terminal: {instance_id} is in state {current_state:?}")]
    AlreadyTerminal {
        instance_id: InstanceId,
        current_state: LifecycleState,
    },
    #[error("lineage is tombstoned: {lineage_id}")]
    LineageTombstoned { lineage_id: String },
    #[error("lock acquisition failed for {instance_id}: {reason}")]
    LockAcquisitionFailed {
        instance_id: InstanceId,
        reason: String,
    },
    #[error("storage error for {instance_id}: {reason}")]
    StorageError {
        instance_id: InstanceId,
        reason: String,
    },
}