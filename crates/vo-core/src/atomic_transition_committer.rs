//! Production commit boundary for atomic control-plane transitions.
//!
//! Per ADR-016: Every control-plane transition must commit atomically via
//! a single `fjall::Batch`. `AtomicTransitionCommitter` is the sole
//! production gateway — all partition writes for a transition route
//! through this type instead of ad hoc store writes.
//!
//! # Invariants
//!
//! - No caller may bypass this boundary and write to storage partitions directly.
//! - All writes for a single transition are buffered and committed as one batch.
//! - If commit fails, zero state changes are visible (all-or-nothing).

use crate::db_writer_message::DbWriterMessage;
use crate::transaction::{Transaction, TransactionCommitter, TransactionError};

pub struct AtomicTransitionCommitter<C> {
    tx: Transaction<C>,
}

impl<C> AtomicTransitionCommitter<C> {
    pub fn new() -> Self {
        Self {
            tx: Transaction::new(),
        }
    }

    pub fn push(&mut self, message: DbWriterMessage) -> Result<(), TransactionError> {
        self.tx.push(message)
    }

    pub fn pending_count(&self) -> usize {
        self.tx.pending_count()
    }

    pub fn is_empty(&self) -> bool {
        self.tx.is_empty()
    }
}

impl<C> Default for AtomicTransitionCommitter<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C: TransactionCommitter> AtomicTransitionCommitter<C> {
    pub fn commit(self, committer: &C) -> Result<(), TransactionError> {
        self.tx.commit(committer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_writer_message::{SnapshotData, TimerOp};
    use std::cell::RefCell;
    use vo_types::events::EventMetadata;
    use vo_types::{
        EffectIntent, EffectKind, EffectRecord, EventEnvelope, FenceToken, FireAtMs,
        IdempotencyKey, InstanceId, InstanceStatus, SequenceNumber, StepId, TimerId,
        MAX_SUPPORTED_SCHEMA_VERSION,
    };

    struct MockCommitter {
        committed: RefCell<Vec<Vec<DbWriterMessage>>>,
    }

    impl MockCommitter {
        fn new() -> Self {
            Self {
                committed: RefCell::new(Vec::new()),
            }
        }
    }

    impl TransactionCommitter for MockCommitter {
        fn commit_batch(&self, messages: Vec<DbWriterMessage>) -> Result<(), TransactionError> {
            self.committed.borrow_mut().push(messages);
            Ok(())
        }
    }

    fn valid_instance_id() -> InstanceId {
        InstanceId::parse("01ARYZ6S410000000000000000").expect("valid instance id")
    }

    fn valid_sequence() -> SequenceNumber {
        SequenceNumber::new_unchecked(1)
    }

    fn valid_step_id() -> StepId {
        StepId::parse("step-1").expect("valid step id")
    }

    fn valid_fence_token() -> FenceToken {
        FenceToken::new(1).expect("valid fence token")
    }

    fn valid_idempotency_key() -> IdempotencyKey {
        IdempotencyKey::parse("key-1").expect("valid key")
    }

    fn valid_timer_id() -> TimerId {
        TimerId::parse("timer-1").expect("valid timer id")
    }

    fn valid_fire_at() -> FireAtMs {
        FireAtMs::try_from(1712200000000u64).expect("valid fire_at")
    }

    fn valid_event_envelope() -> EventEnvelope {
        EventEnvelope {
            schema_version: 1,
            instance_id: "01ARYZ6S410000000000000000".to_string(),
            sequence: 1,
            timestamp_ms: 1712200000000,
            payload: serde_json::json!({}),
            metadata: EventMetadata::default(),
        }
    }

    fn valid_snapshot_data() -> SnapshotData {
        SnapshotData::new(
            valid_sequence(),
            MAX_SUPPORTED_SCHEMA_VERSION,
            vec![0x01, 0x02, 0x03],
        )
        .expect("valid snapshot data")
    }

    fn valid_effect_record() -> EffectRecord {
        EffectRecord::new(
            "intent-1".to_string(),
            EffectKind::HttpCall,
            serde_json::json!({}),
            EffectIntent::Prepared,
            None,
        )
        .expect("valid effect record")
    }

    #[test]
    fn given_transition_when_committed_then_atomic_committer_is_used() {
        let committer = MockCommitter::new();
        let mut boundary = AtomicTransitionCommitter::new();

        let event_msg = DbWriterMessage::AppendEvent {
            instance_id: valid_instance_id(),
            sequence_number: valid_sequence(),
            idempotency_key: valid_idempotency_key(),
        };

        let status_msg = DbWriterMessage::RecordInstanceStatus {
            instance_id: valid_instance_id(),
            status_byte: InstanceStatus::Running as u8,
        };

        let lease_msg = DbWriterMessage::AcquireLease {
            instance_id: valid_instance_id(),
            step_id: valid_step_id(),
            fence: valid_fence_token(),
        };

        let timer_msg = DbWriterMessage::UpsertTimer {
            instance_id: valid_instance_id(),
            timer_id: valid_timer_id(),
            fire_at: valid_fire_at(),
        };

        let effect_msg = DbWriterMessage::RecordEffect {
            effect: valid_effect_record(),
        };

        let snapshot_msg = DbWriterMessage::TakeSnapshot {
            instance_id: valid_instance_id(),
            sequence_number: valid_sequence(),
            snapshot_data: valid_snapshot_data(),
        };

        let atomic_msg = DbWriterMessage::AtomicTransition {
            step_id: Some(valid_step_id()),
            instance_status: Some(InstanceStatus::Running),
            timer_ops: vec![TimerOp::Upsert {
                timer_id: valid_timer_id(),
                fire_at: valid_fire_at(),
            }],
            snapshot: Some(valid_snapshot_data()),
            event: valid_event_envelope(),
        };

        assert!(boundary.push(event_msg.clone()).is_ok());
        assert!(boundary.push(status_msg.clone()).is_ok());
        assert!(boundary.push(lease_msg.clone()).is_ok());
        assert!(boundary.push(timer_msg.clone()).is_ok());
        assert!(boundary.push(effect_msg.clone()).is_ok());
        assert!(boundary.push(snapshot_msg.clone()).is_ok());
        assert!(boundary.push(atomic_msg.clone()).is_ok());

        assert_eq!(boundary.pending_count(), 7);
        assert!(!boundary.is_empty());

        let commit_result = boundary.commit(&committer);

        assert!(commit_result.is_ok());

        let batches = committer.committed.borrow();
        assert_eq!(batches.len(), 1, "exactly one batch must be committed");
        assert_eq!(
            batches[0].len(),
            7,
            "all 7 partition writes must be in the single batch"
        );

        assert_eq!(batches[0][0], event_msg);
        assert_eq!(batches[0][1], status_msg);
        assert_eq!(batches[0][2], lease_msg);
        assert_eq!(batches[0][3], timer_msg);
        assert_eq!(batches[0][4], effect_msg);
        assert_eq!(batches[0][5], snapshot_msg);
        assert_eq!(batches[0][6], atomic_msg);
    }

    #[test]
    fn given_empty_transition_when_committed_then_returns_empty_transaction_error() {
        let committer = MockCommitter::new();
        let boundary: AtomicTransitionCommitter<MockCommitter> = AtomicTransitionCommitter::new();

        let result = boundary.commit(&committer);

        assert!(matches!(result, Err(TransactionError::EmptyTransaction)));
        assert_eq!(committer.committed.borrow().len(), 0);
    }

    #[test]
    fn given_single_atomic_transition_when_committed_then_batch_contains_one_message() {
        let committer = MockCommitter::new();
        let mut boundary = AtomicTransitionCommitter::new();

        let msg = DbWriterMessage::AtomicTransition {
            step_id: Some(valid_step_id()),
            instance_status: Some(InstanceStatus::Running),
            timer_ops: vec![TimerOp::Delete {
                timer_id: valid_timer_id(),
            }],
            snapshot: None,
            event: valid_event_envelope(),
        };

        assert!(boundary.push(msg.clone()).is_ok());
        assert!(boundary.commit(&committer).is_ok());

        let batches = committer.committed.borrow();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].len(), 1);
        assert_eq!(batches[0][0], msg);
    }
}
