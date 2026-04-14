use vo_types::InstanceId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LifecycleState {
    Running,
    Failed,
    Completed,
    Cancelled,
    WaitingForSignal,
}

impl LifecycleState {
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SecretId(pub String);

impl SecretId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BinaryHash(pub String);

impl BinaryHash {
    pub fn new(hash: impl Into<String>) -> Self {
        Self(hash.into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimestampMs(pub i64);

impl TimestampMs {
    pub fn now() -> Self {
        Self(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WaitKey(String);

impl WaitKey {
    pub fn parse(input: &str) -> Result<Self, String> {
        if input.is_empty() {
            return Err("WaitKey cannot be empty".to_string());
        }
        if input.len() > 256 {
            return Err(format!("WaitKey exceeds 256 characters: {}", input.len()));
        }
        Ok(Self(input.to_string()))
    }

    pub fn new_unchecked(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalPayload(Vec<u8>);

impl SignalPayload {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, String> {
        if bytes.len() > 65536 {
            return Err(format!(
                "SignalPayload exceeds 64 KiB: {} bytes",
                bytes.len()
            ));
        }
        Ok(Self(bytes))
    }

    pub fn empty() -> Self {
        Self(Vec::new())
    }

    pub fn new_unchecked(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalAccepted {
    pub instance_id: InstanceId,
    pub wait_key: WaitKey,
    pub signal_id: String,
    pub payload: SignalPayload,
    pub accepted_at: TimestampMs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptResumeOutcome {
    pub accepted: SignalAccepted,
    pub resumed: InstanceResumed,
}

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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeName(pub String);

impl NodeName {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancelRequested {
    pub instance_id: InstanceId,
    pub requested_at: TimestampMs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowCancelled {
    pub instance_id: InstanceId,
    pub cancelled_at: TimestampMs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceResumed {
    pub instance_id: InstanceId,
    pub previous_binary_hash: BinaryHash,
    pub resumed_binary_hash: BinaryHash,
    pub resumed_at: TimestampMs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowContinued {
    pub old_instance_id: InstanceId,
    pub new_instance_id: InstanceId,
    pub lineage_id: String,
    pub old_epoch: u64,
    pub new_epoch: u64,
    pub continued_at: TimestampMs,
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

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SignalStorageError {
    #[error("Instance not found: {0}")]
    InstanceNotFound(InstanceId),
    #[error("Write error for {instance_id}: {reason}")]
    WriteError {
        instance_id: InstanceId,
        reason: String,
    },
    #[error("Delete error for {instance_id}: {reason}")]
    DeleteError {
        instance_id: InstanceId,
        reason: String,
    },
}

pub trait SignalStorage: Send + Sync {
    fn persist_signal_accepted(&self, accepted: &SignalAccepted) -> Result<(), SignalStorageError>;

    fn remove_signal_accepted(
        &self,
        instance_id: &InstanceId,
        signal_id: &str,
    ) -> Result<(), SignalStorageError>;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SignalWorkQueueError {
    #[error("Instance not found: {0}")]
    InstanceNotFound(InstanceId),
    #[error("Enqueue error for {instance_id}: {reason}")]
    EnqueueError {
        instance_id: InstanceId,
        reason: String,
    },
}

pub trait SignalWorkQueue: Send + Sync {
    fn enqueue_resume(&self, instance_id: InstanceId) -> Result<(), SignalWorkQueueError>;
}

pub mod mock_signal_storage {
    use super::*;

    #[derive(Debug, Default)]
    pub struct MockSignalStorage {
        persisted: std::sync::Mutex<Vec<SignalAccepted>>,
        should_fail: std::sync::Mutex<bool>,
    }

    impl MockSignalStorage {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn set_should_fail(&self, should_fail: bool) {
            *self.should_fail.lock().unwrap() = should_fail;
        }

        pub fn persisted_signals(&self) -> Vec<SignalAccepted> {
            self.persisted.lock().unwrap().clone()
        }

        #[allow(dead_code)]
        pub fn clear(&self) {
            self.persisted.lock().unwrap().clear();
        }
    }

    impl SignalStorage for MockSignalStorage {
        fn persist_signal_accepted(
            &self,
            accepted: &SignalAccepted,
        ) -> Result<(), SignalStorageError> {
            if *self.should_fail.lock().unwrap() {
                return Err(SignalStorageError::WriteError {
                    instance_id: accepted.instance_id.clone(),
                    reason: "Mock storage failure".to_string(),
                });
            }
            self.persisted.lock().unwrap().push(accepted.clone());
            Ok(())
        }

        fn remove_signal_accepted(
            &self,
            instance_id: &InstanceId,
            signal_id: &str,
        ) -> Result<(), SignalStorageError> {
            if *self.should_fail.lock().unwrap() {
                return Err(SignalStorageError::DeleteError {
                    instance_id: instance_id.clone(),
                    reason: "Mock storage failure".to_string(),
                });
            }
            let mut persisted = self.persisted.lock().unwrap();
            persisted.retain(|s| !(s.instance_id == *instance_id && s.signal_id == signal_id));
            Ok(())
        }
    }

    #[derive(Debug, Default)]
    pub struct MockSignalWorkQueue {
        enqueued: std::sync::Mutex<Vec<InstanceId>>,
        should_fail: std::sync::Mutex<bool>,
        instance_not_found: std::sync::Mutex<bool>,
    }

    impl MockSignalWorkQueue {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn set_should_fail(&self, should_fail: bool) {
            *self.should_fail.lock().unwrap() = should_fail;
        }

        pub fn set_instance_not_found(&self, not_found: bool) {
            *self.instance_not_found.lock().unwrap() = not_found;
        }

        pub fn enqueued_instances(&self) -> Vec<InstanceId> {
            self.enqueued.lock().unwrap().clone()
        }

        #[allow(dead_code)]
        pub fn clear(&self) {
            self.enqueued.lock().unwrap().clear();
        }
    }

    impl SignalWorkQueue for MockSignalWorkQueue {
        fn enqueue_resume(&self, instance_id: InstanceId) -> Result<(), SignalWorkQueueError> {
            if *self.instance_not_found.lock().unwrap() {
                return Err(SignalWorkQueueError::InstanceNotFound(instance_id));
            }
            if *self.should_fail.lock().unwrap() {
                return Err(SignalWorkQueueError::EnqueueError {
                    instance_id,
                    reason: "Mock queue failure".to_string(),
                });
            }
            self.enqueued.lock().unwrap().push(instance_id);
            Ok(())
        }
    }
}
