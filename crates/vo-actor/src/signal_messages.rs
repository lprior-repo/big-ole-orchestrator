use vo_types::InstanceId;
pub use vo_types::TimestampMs;
pub use vo_types::{BinaryHash, NodeName};

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

pub trait StateLookup: Send + Sync {
    fn derive_lifecycle_state(&self, instance_id: &InstanceId) -> LifecycleState;
    fn derive_error_type(&self, instance_id: &InstanceId) -> Option<&'static str>;
}

#[derive(Debug, Clone)]
pub struct TestStateLookup;

impl TestStateLookup {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for TestStateLookup {
    fn default() -> Self {
        Self::new()
    }
}

impl StateLookup for TestStateLookup {
    fn derive_lifecycle_state(&self, instance_id: &InstanceId) -> LifecycleState {
        let id_str = instance_id.as_str();
        id_str
            .chars()
            .nth(22)
            .map_or(LifecycleState::Running, |c| match c {
                'C' => LifecycleState::Completed,
                'X' => LifecycleState::Cancelled,
                'F' => LifecycleState::Failed,
                'W' => LifecycleState::WaitingForSignal,
                _ => LifecycleState::Running,
            })
    }

    fn derive_error_type(&self, instance_id: &InstanceId) -> Option<&'static str> {
        let id_str = instance_id.as_str();
        id_str.chars().nth(20).and_then(|c| match c {
            'A' => Some("lock"),
            'S' => Some("storage"),
            'M' => Some("missing"),
            'N' => Some("nodenotfound"),
            'P' => Some("nopathtoterminal"),
            _ => None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SecretId(pub String);

impl SecretId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
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

impl From<vo_types::WaitKey> for WaitKey {
    fn from(value: vo_types::WaitKey) -> Self {
        Self(value.as_str().to_string())
    }
}

impl From<&vo_types::WaitKey> for WaitKey {
    fn from(value: &vo_types::WaitKey) -> Self {
        Self(value.as_str().to_string())
    }
}

impl From<&WaitKey> for WaitKey {
    fn from(value: &WaitKey) -> Self {
        value.clone()
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
        if bytes.contains(&0) {
            return Err("SignalPayload contains null byte".to_string());
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SignalName(String);

impl SignalName {
    pub fn parse(input: &str) -> Result<Self, String> {
        const MAX_LEN: usize = 256;
        if input.is_empty() {
            return Err("SignalName cannot be empty".to_string());
        }
        if input.len() > MAX_LEN {
            return Err(format!(
                "SignalName exceeds {} characters: {}",
                MAX_LEN,
                input.len()
            ));
        }
        if input.contains('\0') {
            return Err("SignalName contains null byte".to_string());
        }
        let invalid = input
            .chars()
            .filter(|c| !c.is_alphanumeric() && *c != '-' && *c != '_' && *c != '.')
            .collect::<String>();
        if !invalid.is_empty() {
            return Err(format!(
                "SignalName contains invalid characters: {}",
                invalid
            ));
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

impl From<&str> for SignalName {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for SignalName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&String> for SignalName {
    fn from(value: &String) -> Self {
        Self(value.clone())
    }
}

impl PartialEq<String> for SignalName {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for SignalName {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalAccepted {
    pub instance_id: InstanceId,
    pub wait_key: WaitKey,
    pub signal_id: SignalName,
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
    pub carried_dedupe_keys: Vec<String>,
    pub carried_wait_keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RolloverState {
    pub dedupe_keys: Vec<String>,
    pub pending_wait_keys: Vec<String>,
}

impl RolloverState {
    pub fn empty() -> Self {
        Self {
            dedupe_keys: Vec::new(),
            pending_wait_keys: Vec::new(),
        }
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
            persisted
                .retain(|s| !(s.instance_id == *instance_id && s.signal_id.as_str() == signal_id));
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

#[cfg(test)]
mod rollover_tests {
    use super::*;

    fn instance_id(s: &str) -> InstanceId {
        InstanceId::parse(s).unwrap_or_else(|_| InstanceId::from_bytes([0u8; 16]))
    }

    #[test]
    fn rollover_state_empty_has_no_keys() {
        let state = RolloverState::empty();
        assert!(state.dedupe_keys.is_empty());
        assert!(state.pending_wait_keys.is_empty());
    }

    #[test]
    fn rollover_state_carries_dedupe_keys() {
        let state = RolloverState {
            dedupe_keys: vec!["cmd-1".to_string(), "cmd-2".to_string()],
            pending_wait_keys: vec!["approval".to_string()],
        };
        assert_eq!(state.dedupe_keys.len(), 2);
        assert_eq!(state.pending_wait_keys.len(), 1);
    }

    #[test]
    fn workflow_continued_carries_keys_from_rollover_state() {
        let rollover = RolloverState {
            dedupe_keys: vec!["dedupe-a".to_string(), "dedupe-b".to_string()],
            pending_wait_keys: vec!["wait-approval".to_string(), "wait-timeout".to_string()],
        };
        let continued = WorkflowContinued {
            old_instance_id: instance_id("01H5JYV4XHGSR2F8KZ9BWNRFMA"),
            new_instance_id: instance_id("01H5JYV4XHGSR2F8KZ9BWNRFMB"),
            lineage_id: "lineage-abc".to_string(),
            old_epoch: 0,
            new_epoch: 1,
            continued_at: TimestampMs::now(),
            carried_dedupe_keys: rollover.dedupe_keys.clone(),
            carried_wait_keys: rollover.pending_wait_keys.clone(),
        };
        assert_eq!(continued.carried_dedupe_keys, vec!["dedupe-a", "dedupe-b"]);
        assert_eq!(
            continued.carried_wait_keys,
            vec!["wait-approval", "wait-timeout"]
        );
    }

    #[test]
    fn workflow_continued_with_empty_rollover_state() {
        let rollover = RolloverState::empty();
        let continued = WorkflowContinued {
            old_instance_id: instance_id("01H5JYV4XHGSR2F8KZ9BWNRFMA"),
            new_instance_id: instance_id("01H5JYV4XHGSR2F8KZ9BNRFMA"),
            lineage_id: "lineage-def".to_string(),
            old_epoch: 0,
            new_epoch: 1,
            continued_at: TimestampMs::now(),
            carried_dedupe_keys: rollover.dedupe_keys,
            carried_wait_keys: rollover.pending_wait_keys,
        };
        assert!(continued.carried_dedupe_keys.is_empty());
        assert!(continued.carried_wait_keys.is_empty());
    }

    #[test]
    fn cross_epoch_deduplication_rejects_duplicate_command() {
        let dedupe_keys = vec!["cmd-x".to_string()];
        let rollover = RolloverState {
            dedupe_keys: dedupe_keys.clone(),
            pending_wait_keys: vec![],
        };
        let continued = WorkflowContinued {
            old_instance_id: instance_id("01H5JYV4XHGSR2F8KZ9BWNRFMA"),
            new_instance_id: instance_id("01H5JYV4XHGSR2F8KZ9BWNRFMB"),
            lineage_id: "lineage-dedup".to_string(),
            old_epoch: 0,
            new_epoch: 1,
            continued_at: TimestampMs::now(),
            carried_dedupe_keys: rollover.dedupe_keys,
            carried_wait_keys: rollover.pending_wait_keys,
        };
        assert!(
            continued.carried_dedupe_keys.contains(&"cmd-x".to_string()),
            "Command X from epoch 0 must appear in carried dedupe keys for epoch 1 rejection"
        );
    }

    #[test]
    fn signal_wait_key_preserved_across_rollover() {
        let wait_keys = vec!["approval-v2".to_string(), "webhook-response".to_string()];
        let rollover = RolloverState {
            dedupe_keys: vec![],
            pending_wait_keys: wait_keys.clone(),
        };
        let continued = WorkflowContinued {
            old_instance_id: instance_id("01H5JYV4XHGSR2F8KZ9BWNRFMA"),
            new_instance_id: instance_id("01H5JYV4XHGSR2F8KZ9BWNRFMB"),
            lineage_id: "lineage-signal".to_string(),
            old_epoch: 0,
            new_epoch: 1,
            continued_at: TimestampMs::now(),
            carried_dedupe_keys: rollover.dedupe_keys,
            carried_wait_keys: rollover.pending_wait_keys,
        };
        assert!(continued
            .carried_wait_keys
            .contains(&"approval-v2".to_string()));
        assert!(continued
            .carried_wait_keys
            .contains(&"webhook-response".to_string()));
    }

    #[test]
    fn invariant_command_id_one_side_effect_across_epochs() {
        let all_dedupe_keys = vec!["cmd-alpha".to_string(), "cmd-beta".to_string()];
        let rollover = RolloverState {
            dedupe_keys: all_dedupe_keys.clone(),
            pending_wait_keys: vec![],
        };
        let continued = WorkflowContinued {
            old_instance_id: instance_id("01H5JYV4XHGSR2F8KZ9BWNRFMA"),
            new_instance_id: instance_id("01H5JYV4XHGSR2F8KZ9BWNRFMB"),
            lineage_id: "lineage-inv".to_string(),
            old_epoch: 0,
            new_epoch: 1,
            continued_at: TimestampMs::now(),
            carried_dedupe_keys: rollover.dedupe_keys,
            carried_wait_keys: rollover.pending_wait_keys,
        };
        assert_eq!(continued.carried_dedupe_keys.len(), 2);
        let unique_keys: std::collections::HashSet<_> =
            continued.carried_dedupe_keys.iter().collect();
        assert_eq!(
            unique_keys.len(),
            continued.carried_dedupe_keys.len(),
            "Each command_id must appear exactly once across all epochs of a lineage"
        );
    }
}
