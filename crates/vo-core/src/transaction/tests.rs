//! Transaction tests

use crate::db_writer_message::{DbWriterMessage, SnapshotData, TimerOp};
use crate::transaction::{Transaction, TransactionCommitter, TransactionError};
use vo_types::events::EventMetadata;
use vo_types::{
    EventEnvelope, FenceToken, FireAtMs, IdempotencyKey, InstanceId, InstanceStatus,
    SequenceNumber, StepId, TimerId,
};

struct MockCommitter {
    should_fail: bool,
    occ_fail: bool,
    committed: std::cell::RefCell<Vec<Vec<DbWriterMessage>>>,
}

impl MockCommitter {
    fn new() -> Self {
        Self {
            should_fail: false,
            occ_fail: false,
            committed: std::cell::RefCell::new(Vec::new()),
        }
    }

    fn failing() -> Self {
        Self {
            should_fail: true,
            occ_fail: false,
            committed: std::cell::RefCell::new(Vec::new()),
        }
    }

    fn occ_failing() -> Self {
        Self {
            should_fail: false,
            occ_fail: true,
            committed: std::cell::RefCell::new(Vec::new()),
        }
    }
}

impl TransactionCommitter for MockCommitter {
    fn commit_batch(&self, messages: Vec<DbWriterMessage>) -> Result<(), TransactionError> {
        if self.should_fail {
            return Err(TransactionError::StorageCommitFailed(
                "disk full".to_string(),
            ));
        }
        if self.occ_fail {
            return Err(TransactionError::OccConflict(
                "fence token stale".to_string(),
            ));
        }
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

fn valid_idempotency_key() -> IdempotencyKey {
    IdempotencyKey::parse("key-1").expect("valid key")
}

fn valid_step_id() -> StepId {
    StepId::parse("step-1").expect("valid step id")
}

fn valid_fence_token() -> FenceToken {
    FenceToken::new(1).expect("valid fence token")
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

fn make_append_event() -> DbWriterMessage {
    DbWriterMessage::AppendEvent {
        instance_id: valid_instance_id(),
        sequence_number: valid_sequence(),
        idempotency_key: valid_idempotency_key(),
    }
}

fn make_acquire_lease() -> DbWriterMessage {
    DbWriterMessage::AcquireLease {
        instance_id: valid_instance_id(),
        step_id: valid_step_id(),
        fence: valid_fence_token(),
    }
}

fn make_upsert_timer() -> DbWriterMessage {
    DbWriterMessage::UpsertTimer {
        instance_id: valid_instance_id(),
        timer_id: valid_timer_id(),
        fire_at: valid_fire_at(),
    }
}

fn make_atomic_transition() -> DbWriterMessage {
    DbWriterMessage::AtomicTransition {
        step_id: Some(valid_step_id()),
        instance_status: Some(InstanceStatus::Running),
        timer_ops: vec![TimerOp::Upsert {
            timer_id: valid_timer_id(),
            fire_at: valid_fire_at(),
        }],
        snapshot: Some(
            SnapshotData::new(valid_sequence(), 1, vec![0x01, 0x02]).expect("valid snapshot"),
        ),
        event: valid_event_envelope(),
    }
}

#[test]
fn test_gathers_3_distinct_events_and_commits_them_successfully_as_a_batch() {
    let committer = MockCommitter::new();
    let mut tx: Transaction<MockCommitter> = Transaction::new();

    let result1 = tx.push(make_append_event());
    let result2 = tx.push(make_acquire_lease());
    let result3 = tx.push(make_upsert_timer());

    assert!(result1.is_ok());
    assert!(result2.is_ok());
    assert!(result3.is_ok());
    assert_eq!(tx.pending_count(), 3);

    let commit_result = tx.commit(&committer);

    assert!(commit_result.is_ok());
    let batches = committer.committed.borrow();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].len(), 3);
}

#[test]
fn test_gathers_3_distinct_events_and_commits_them_successfully_as_a_batch_duplicate_for() {
    let committer = MockCommitter::new();
    let mut tx: Transaction<MockCommitter> = Transaction::new();

    tx.push(make_append_event()).ok();
    tx.push(make_acquire_lease()).ok();
    tx.push(make_upsert_timer()).ok();

    assert_eq!(tx.pending_count(), 3);

    let commit_result = tx.commit(&committer);

    assert!(commit_result.is_ok());
    let batches = committer.committed.borrow();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].len(), 3);
}

#[test]
fn test_commit_fails_with_an_occ_error_all_events_remain_uncommitted_locally() {
    let committer = MockCommitter::occ_failing();
    let mut tx: Transaction<MockCommitter> = Transaction::new();

    tx.push(make_append_event()).ok();
    tx.push(make_acquire_lease()).ok();
    tx.push(make_upsert_timer()).ok();

    let commit_result = tx.commit(&committer);

    assert!(commit_result.is_err());
    match commit_result {
        Err(TransactionError::OccConflict(msg)) => {
            assert!(msg.contains("fence token stale"));
        }
        other => panic!("expected OccConflict, got: {other:?}"),
    }

    let batches = committer.committed.borrow();
    assert_eq!(batches.len(), 0);
}

#[test]
fn test_commit_fails_with_an_occ_error_all_events_remain_uncommitted_locally_duplicate() {
    let committer = MockCommitter::occ_failing();
    let mut tx: Transaction<MockCommitter> = Transaction::new();

    tx.push(make_atomic_transition()).ok();
    tx.push(make_append_event()).ok();

    let commit_result = tx.commit(&committer);

    assert!(commit_result.is_err());
    assert!(matches!(
        commit_result,
        Err(TransactionError::OccConflict(_))
    ));

    let batches = committer.committed.borrow();
    assert_eq!(batches.len(), 0);
}

#[test]
fn test_push_after_commit_is_prevented_by_ownership() {
    let committer = MockCommitter::new();
    let mut tx: Transaction<MockCommitter> = Transaction::new();

    tx.push(make_append_event()).ok();
    let _result = tx.commit(&committer);

    assert!(matches!(_result, Ok(())));
    let batches = committer.committed.borrow();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].len(), 1);
}

#[test]
fn test_commit_on_empty_transaction_returns_error() {
    let committer = MockCommitter::new();
    let tx: Transaction<MockCommitter> = Transaction::new();

    let result = tx.commit(&committer);
    assert!(matches!(result, Err(TransactionError::EmptyTransaction)));

    let batches = committer.committed.borrow();
    assert_eq!(batches.len(), 0);
}

#[test]
fn test_double_commit_is_prevented_by_ownership() {
    let committer = MockCommitter::new();
    let mut tx: Transaction<MockCommitter> = Transaction::new();

    tx.push(make_append_event()).ok();
    let first = tx.commit(&committer);

    assert!(first.is_ok());
    let batches = committer.committed.borrow();
    assert_eq!(batches.len(), 1);
}

#[test]
fn test_storage_commit_failure_preserves_no_partial_state() {
    let committer = MockCommitter::failing();
    let mut tx: Transaction<MockCommitter> = Transaction::new();

    tx.push(make_append_event()).ok();
    tx.push(make_acquire_lease()).ok();

    let result = tx.commit(&committer);

    assert!(matches!(
        result,
        Err(TransactionError::StorageCommitFailed(_))
    ));

    let batches = committer.committed.borrow();
    assert_eq!(batches.len(), 0);
}

#[test]
fn test_messages_returns_all_pending_messages() {
    let mut tx: Transaction<MockCommitter> = Transaction::new();

    assert!(tx.is_empty());

    tx.push(make_append_event()).ok();
    tx.push(make_acquire_lease()).ok();

    assert!(!tx.is_empty());
    assert_eq!(tx.messages().len(), 2);
}

#[test]
fn test_pending_count_tracks_pushes() {
    let mut tx: Transaction<MockCommitter> = Transaction::new();

    assert_eq!(tx.pending_count(), 0);

    tx.push(make_append_event()).ok();
    assert_eq!(tx.pending_count(), 1);

    tx.push(make_acquire_lease()).ok();
    assert_eq!(tx.pending_count(), 2);

    tx.push(make_upsert_timer()).ok();
    assert_eq!(tx.pending_count(), 3);
}

#[test]
fn test_default_creates_open_transaction() {
    let tx: Transaction<MockCommitter> = Transaction::default();
    assert_eq!(tx.pending_count(), 0);
    assert!(tx.is_empty());
}

#[test]
fn test_atomic_transition_commits_as_single_batch() {
    let committer = MockCommitter::new();
    let mut tx: Transaction<MockCommitter> = Transaction::new();

    tx.push(make_atomic_transition()).ok();

    let commit_result = tx.commit(&committer);

    assert!(commit_result.is_ok());
    let batches = committer.committed.borrow();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].len(), 1);
}

#[test]
fn test_mixed_message_types_commit_together() {
    let committer = MockCommitter::new();
    let mut tx: Transaction<MockCommitter> = Transaction::new();

    tx.push(make_append_event()).ok();
    tx.push(make_acquire_lease()).ok();
    tx.push(make_upsert_timer()).ok();
    tx.push(make_atomic_transition()).ok();

    let commit_result = tx.commit(&committer);

    assert!(commit_result.is_ok());
    let batches = committer.committed.borrow();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].len(), 4);
}
