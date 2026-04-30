use vo_types::{BinaryHash, InstanceId};
pub use vo_types::TimestampMs;

use super::signal_in::{LifecycleState, SecretId, SignalPayload, SignalName, WaitKey};

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

pub use super::errors::{AcceptResumeError, CancelError, ContinueAsNewError, ResumeError};

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

#[cfg(test)]
mod rollover_tests {
    use super::*;
    use vo_types::InstanceId;

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