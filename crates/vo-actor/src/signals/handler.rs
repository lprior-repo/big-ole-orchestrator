use super::types::*;
use vo_types::InstanceId;

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
