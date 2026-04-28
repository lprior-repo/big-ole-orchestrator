//! Fjall-backed TransactionCommitter for production DbWriterActor use.
//!
//! Per ADR-016: This committer writes all control-plane transitions atomically
//! via `fjall::Batch` to ensure exact-once semantics across all partitions.
//!
//! # Partitions Written
//!
//! - `events`: Minimal replay events and state transitions
//! - `instances`: Materialized instance summaries
//! - `timers`: Durable wake-up schedule for hibernated workflows
//! - `dedupe`: Exactly-once ingress deduplication records
//! - `effects`: EffectPrepared and EffectCommitted journal entries
//! - `leases`: Monotonic fence tokens for step execution ownership
//! - `snapshots`: Periodic replay acceleration checkpoints

use std::sync::Arc;

use vo_types::{
    EffectIntent, EffectKind, EffectRecord, EventEnvelope, FenceToken, FireAtMs, IdempotencyKey,
    InstanceId, InstanceStatus, SequenceNumber, StepId, TimerId,
};

use crate::db_writer_message::types::TimerOp;
use crate::db_writer_message::DbWriterMessage;
use crate::transaction::{TransactionCommitter, TransactionError};

const EVENTS_PARTITION: &str = "events";
const INSTANCES_PARTITION: &str = "instances";
const TIMERS_PARTITION: &str = "timers";
const SNAPSHOTS_PARTITION: &str = "snapshots";
const DEDUPE_PARTITION: &str = "dedupe";
const EFFECTS_PARTITION: &str = "effects";
const LEASES_PARTITION: &str = "leases";

pub struct FjallDbWriter {
    db: Arc<fjall::Database>,
}

impl FjallDbWriter {
    #[must_use]
    pub fn new(db: Arc<fjall::Database>) -> Self {
        Self { db }
    }

    fn open_keyspace(&self, name: &str) -> Result<fjall::Keyspace, TransactionError> {
        self.db
            .keyspace(name, fjall::KeyspaceCreateOptions::default())
            .map_err(|e| TransactionError::StorageCommitFailed(format!("keyspace {name}: {e}")))
    }

    fn encode_event_key(instance_id: &InstanceId, sequence: &SequenceNumber) -> Result<Vec<u8>, TransactionError> {
        let id_bytes = instance_id
            .to_bytes()
            .map_err(|_| TransactionError::StorageCommitFailed("invalid instance_id".to_string()))?;
        let seq_bytes = sequence.as_u64().to_be_bytes();
        let mut key = Vec::with_capacity(24);
        key.extend_from_slice(&id_bytes);
        key.extend_from_slice(&seq_bytes);
        Ok(key)
    }

    fn encode_timer_key(instance_id: &InstanceId, timer_id: &TimerId) -> Result<Vec<u8>, TransactionError> {
        let id_bytes = instance_id
            .to_bytes()
            .map_err(|_| TransactionError::StorageCommitFailed("invalid instance_id".to_string()))?;
        let timer_bytes = timer_id.as_bytes();
        let mut key = Vec::with_capacity(16 + timer_bytes.len());
        key.extend_from_slice(&id_bytes);
        key.extend_from_slice(timer_bytes);
        Ok(key)
    }

    fn encode_lease_key(instance_id: &InstanceId, step_id: &StepId) -> Result<Vec<u8>, TransactionError> {
        let id_bytes = instance_id
            .to_bytes()
            .map_err(|_| TransactionError::StorageCommitFailed("invalid instance_id".to_string()))?;
        let step_bytes = step_id.as_bytes();
        let mut key = Vec::with_capacity(16 + step_bytes.len());
        key.extend_from_slice(&id_bytes);
        key.extend_from_slice(step_bytes);
        Ok(key)
    }

    fn commit_message(
        &self,
        batch: &mut fjall::Batch,
        msg: &DbWriterMessage,
    ) -> Result<(), TransactionError> {
        match msg {
            DbWriterMessage::AppendEvent {
                instance_id,
                sequence_number,
                idempotency_key,
            } => {
                let ks = self.open_keyspace(EVENTS_PARTITION)?;
                let key = Self::encode_event_key(instance_id, sequence_number)?;
                let value = serde_json::to_vec(&idempotency_key)
                    .map_err(|e| TransactionError::StorageCommitFailed(format!("serialize: {e}")))?;
                batch.insert(&ks, &key, &value);
            }
            DbWriterMessage::RecordInstanceStatus {
                instance_id,
                status_byte,
            } => {
                let ks = self.open_keyspace(INSTANCES_PARTITION)?;
                let key = instance_id
                    .to_bytes()
                    .map_err(|_| TransactionError::StorageCommitFailed("invalid instance_id".to_string()))?;
                let mut value = Vec::with_capacity(1);
                value.push(*status_byte);
                batch.insert(&ks, &key, &value);
            }
            DbWriterMessage::AcquireLease {
                instance_id,
                step_id,
                fence,
            } => {
                let ks = self.open_keyspace(LEASES_PARTITION)?;
                let key = Self::encode_lease_key(instance_id, step_id)?;
                let value = fence.as_u64().to_be_bytes().to_vec();
                batch.insert(&ks, &key, &value);
            }
            DbWriterMessage::ReleaseLease { instance_id, step_id } => {
                let ks = self.open_keyspace(LEASES_PARTITION)?;
                let key = Self::encode_lease_key(instance_id, step_id)?;
                batch.delete(&ks, &key);
            }
            DbWriterMessage::UpsertTimer {
                instance_id,
                timer_id,
                fire_at,
            } => {
                let ks = self.open_keyspace(TIMERS_PARTITION)?;
                let key = Self::encode_timer_key(instance_id, timer_id)?;
                let value = fire_at.as_u64().to_be_bytes().to_vec();
                batch.insert(&ks, &key, &value);
            }
            DbWriterMessage::DeleteTimer { instance_id, timer_id } => {
                let ks = self.open_keyspace(TIMERS_PARTITION)?;
                let key = Self::encode_timer_key(instance_id, timer_id)?;
                batch.delete(&ks, &key);
            }
            DbWriterMessage::RecordEffect { effect } => {
                let ks = self.open_keyspace(EFFECTS_PARTITION)?;
                let key = effect.intent_id.as_bytes();
                let value = serde_json::to_vec(effect)
                    .map_err(|e| TransactionError::StorageCommitFailed(format!("serialize effect: {e}")))?;
                batch.insert(&ks, &key, &value);
            }
            DbWriterMessage::TakeSnapshot {
                instance_id,
                sequence_number,
                snapshot_data,
            } => {
                let ks = self.open_keyspace(SNAPSHOTS_PARTITION)?;
                let key = Self::encode_event_key(instance_id, sequence_number)?;
                let value = snapshot_data.serialize();
                batch.insert(&ks, &key, &value);
            }
            DbWriterMessage::AtomicTransition {
                step_id,
                instance_status,
                timer_ops,
                snapshot,
                event,
            } => {
                let mut batch = self.db.batch();

                if let Some(ref status) = instance_status {
                    let ks = self.open_keyspace(INSTANCES_PARTITION)?;
                    let key = event.instance_id.as_bytes();
                    let mut value = Vec::with_capacity(1);
                    value.push(*status as u8);
                    batch.insert(&ks, &key, &value);
                }

                if let Some(ref step) = step_id {
                    let ks = self.open_keyspace(LEASES_PARTITION)?;
                    let key = Self::encode_lease_key(&InstanceId::parse(&event.instance_id).unwrap(), step)?;
                    let value = vec![];
                    batch.insert(&ks, &key, &value);
                }

                for op in timer_ops {
                    match op {
                        TimerOp::Upsert { timer_id, fire_at } => {
                            let ks = self.open_keyspace(TIMERS_PARTITION)?;
                            let key = Self::encode_timer_key(
                                &InstanceId::parse(&event.instance_id).unwrap(),
                                timer_id,
                            )?;
                            let value = fire_at.as_u64().to_be_bytes().to_vec();
                            batch.insert(&ks, &key, &value);
                        }
                        TimerOp::Delete { timer_id } => {
                            let ks = self.open_keyspace(TIMERS_PARTITION)?;
                            let key = Self::encode_timer_key(
                                &InstanceId::parse(&event.instance_id).unwrap(),
                                timer_id,
                            )?;
                            batch.delete(&ks, &key);
                        }
                    }
                }

                if let Some(ref snap) = snapshot {
                    let ks = self.open_keyspace(SNAPSHOTS_PARTITION)?;
                    let key = Self::encode_event_key(
                        &InstanceId::parse(&event.instance_id).unwrap(),
                        &SequenceNumber::new_unchecked(event.sequence),
                    )?;
                    let value = snap.serialize();
                    batch.insert(&ks, &key, &value);
                }

                let ks = self.open_keyspace(EVENTS_PARTITION)?;
                let key = Self::encode_event_key(
                    &InstanceId::parse(&event.instance_id).unwrap(),
                    &SequenceNumber::new_unchecked(event.sequence),
                )?;
                let value = serde_json::to_vec(event)
                    .map_err(|e| TransactionError::StorageCommitFailed(format!("serialize event: {e}")))?;
                batch.insert(&ks, &key, &value);

                batch.commit().map_err(|e| TransactionError::StorageCommitFailed(format!("batch commit: {e}")))?;
                return Ok(());
            }
        }
        Ok(())
    }
}

impl TransactionCommitter for FjallDbWriter {
    fn commit_batch(&self, messages: Vec<DbWriterMessage>) -> Result<(), TransactionError> {
        if messages.is_empty() {
            return Ok(());
        }

        let mut batch = self.db.batch();

        for msg in &messages {
            self.commit_message(&mut batch, msg)?;
        }

        batch.commit().map_err(|e| {
            tracing::error!("fjall batch commit failed: {}", e);
            TransactionError::StorageCommitFailed(format!("batch commit: {e}"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_db() -> (Arc<fjall::Database>, TempDir) {
        let temp_dir = TempDir::new().expect("create temp dir");
        let db = fjall::Database::open(fjall::Config::new(temp_dir.path()))
            .expect("open test db");
        (Arc::new(db), temp_dir)
    }

    #[test]
    fn commit_single_append_event() {
        let (db, _temp) = create_test_db();
        let writer = FjallDbWriter::new(db);

        let instance_id = InstanceId::parse("01ARYZ6S410000000000000000").expect("valid");
        let msg = DbWriterMessage::AppendEvent {
            instance_id,
            sequence_number: SequenceNumber::new_unchecked(1),
            idempotency_key: IdempotencyKey::parse("key-1").expect("valid"),
        };

        let result = writer.commit_batch(vec![msg]);
        assert!(result.is_ok());
    }

    #[test]
    fn commit_multiple_messages_as_atomic_batch() {
        let (db, _temp) = create_test_db();
        let writer = FjallDbWriter::new(db);

        let instance_id = InstanceId::parse("01ARYZ6S410000000000000000").expect("valid");
        let step_id = StepId::parse("step-1").expect("valid");
        let fence = FenceToken::new(1).expect("valid");
        let timer_id = TimerId::parse("timer-1").expect("valid");
        let fire_at = FireAtMs::try_from(1712200000000u64).expect("valid");

        let msg1 = DbWriterMessage::AppendEvent {
            instance_id,
            sequence_number: SequenceNumber::new_unchecked(1),
            idempotency_key: IdempotencyKey::parse("key-1").expect("valid"),
        };

        let msg2 = DbWriterMessage::AcquireLease {
            instance_id,
            step_id,
            fence,
        };

        let msg3 = DbWriterMessage::UpsertTimer {
            instance_id,
            timer_id,
            fire_at,
        };

        let result = writer.commit_batch(vec![msg1, msg2, msg3]);
        assert!(result.is_ok());
    }

    #[test]
    fn empty_batch_succeeds() {
        let (db, _temp) = create_test_db();
        let writer = FjallDbWriter::new(db);

        let result = writer.commit_batch(vec![]);
        assert!(result.is_ok());
    }
}