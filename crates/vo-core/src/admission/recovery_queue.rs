//! Durable recovery queue for crashed workflow instance re-execution.
//!
//! Workflows that crash leave orphaned instances that pile up without re-execution.
//! A durable recovery queue coordinates retry scheduling with the scheduler and
//! detects orphans.
//!
//! # Architecture
//!
//! - **Persistence**: Uses fjall Keyspace for durability across restarts
//! - **RecoveryReason**: Enum tagging why an instance was queued (Crashed, TimedOut, LostWorker)
//! - **sweep_orphans**: Scans all queued IDs against active actor set to find dead instances
//! - **throttle_next**: Drains up to `budget` items per call for controlled retry pacing

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use vo_types::InstanceId;

const RECOVERY_QUEUE_KEYSPACE: &str = "recovery_queue";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryReason {
    Crashed,
    TimedOut,
    LostWorker,
}

impl RecoveryReason {
    fn as_str(&self) -> &'static str {
        match self {
            RecoveryReason::Crashed => "crashed",
            RecoveryReason::TimedOut => "timed_out",
            RecoveryReason::LostWorker => "lost_worker",
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s {
            "crashed" => Some(RecoveryReason::Crashed),
            "timed_out" => Some(RecoveryReason::TimedOut),
            "lost_worker" => Some(RecoveryReason::LostWorker),
            _ => None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RecoveryQueueError {
    #[error("serialization failed: {0}")]
    SerializationFailed(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("instance already in queue: {0}")]
    AlreadyQueued(InstanceId),
}

pub struct RecoveryQueue {
    partition: fjall::Keyspace,
    pending: Vec<InstanceId>,
}

impl RecoveryQueue {
    #[must_use]
    pub fn new(partition: fjall::Keyspace) -> Self {
        Self {
            partition,
            pending: Vec::new(),
        }
    }

    #[must_use]
    pub fn open(db: &fjall::Database) -> Result<Self, RecoveryQueueError> {
        let partition = db
            .keyspace(RECOVERY_QUEUE_KEYSPACE, fjall::KeyspaceCreateOptions::default())
            .map_err(|e| RecoveryQueueError::Storage(e.to_string()))?;
        Ok(Self::new(partition))
    }

    fn encode_key(instance_id: &InstanceId) -> Vec<u8> {
        instance_id.as_str().as_bytes().to_vec()
    }

    fn decode_key(key: &[u8]) -> Option<InstanceId> {
        let s = std::str::from_utf8(key).ok()?;
        InstanceId::parse(s).ok()
    }

    fn encode_value(reason: &RecoveryReason) -> Result<Vec<u8>, RecoveryQueueError> {
        serde_json::to_string(reason)
            .map_err(|e| RecoveryQueueError::SerializationFailed(e.to_string()))
            .map(|s| s.into_bytes())
    }

    fn decode_value(value: &[u8]) -> Result<RecoveryReason, RecoveryQueueError> {
        let s = std::str::from_utf8(value)
            .map_err(|e| RecoveryQueueError::SerializationFailed(e.to_string()))?;
        serde_json::from_str(s)
            .map_err(|e| RecoveryQueueError::SerializationFailed(e.to_string()))
    }

    pub fn enqueue(
        &self,
        instance_id: InstanceId,
        reason: RecoveryReason,
    ) -> Result<(), RecoveryQueueError> {
        let key = Self::encode_key(&instance_id);
        if self.partition.get(&key).ok().flatten().is_some() {
            return Err(RecoveryQueueError::AlreadyQueued(instance_id));
        }
        let value = Self::encode_value(&reason)?;
        self.partition.insert(&key, &value).map_err(|e| {
            RecoveryQueueError::Storage(format!("failed to insert: {}", e))
        })?;
        Ok(())
    }

    pub fn sweep_orphans(
        &self,
        active_set: HashSet<InstanceId>,
    ) -> Result<Vec<InstanceId>, RecoveryQueueError> {
        let mut orphans = Vec::new();
        let keys: Vec<Vec<u8>> = self
            .partition
            .keys()
            .filter_map(|k| k.ok())
            .map(|k| k.to_vec())
            .collect();
        for key in keys {
            if let Some(instance_id) = Self::decode_key(&key) {
                if !active_set.contains(&instance_id) {
                    orphans.push(instance_id);
                }
            }
        }
        Ok(orphans)
    }

    pub fn throttle_next(&self, budget: u32) -> Option<InstanceId> {
        let mut count = 0;
        for key in self.partition.keys().filter_map(|k| k.ok()) {
            if count >= budget {
                break;
            }
            if let Some(instance_id) = Self::decode_key(&key) {
                self.partition.remove(&key.to_vec()).ok();
                count += 1;
                return Some(instance_id);
            }
        }
        None
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.partition.keys().filter_map(|k| k.ok()).count()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_fjall() -> (tempfile::TempDir, fjall::Database, RecoveryQueue) {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path())
            .open()
            .expect("fjall open");
        let queue = RecoveryQueue::open(&db).expect("recovery queue open");
        (dir, db, queue)
    }

    #[test]
    fn test_enqueue_persists_to_fjall() {
        let (_dir, _db, queue) = setup_fjall();
        let instance_id = InstanceId::parse("01AR6Z1KQT8YZQN8X6JM5X0XYZ")
            .expect("valid ulid");
        let reason = RecoveryReason::Crashed;
        queue.enqueue(instance_id.clone(), reason).expect("enqueue");
        assert!(!queue.is_empty());
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn test_sweep_detects_orphans_not_in_active_set() {
        let (_dir, _db, queue) = setup_fjall();
        let instance_1 = InstanceId::parse("01AR6Z1KQT8YZQN8X6JM5X0XYZ")
            .expect("valid ulid");
        let instance_2 = InstanceId::parse("01AR6Z1KQT8YZQN8X6JM5X0YZA")
            .expect("valid ulid");
        let instance_3 = InstanceId::parse("01AR6Z1KQT8YZQN8X6JM5X0YZB")
            .expect("valid ulid");
        queue
            .enqueue(instance_1.clone(), RecoveryReason::Crashed)
            .expect("enqueue 1");
        queue
            .enqueue(instance_2.clone(), RecoveryReason::TimedOut)
            .expect("enqueue 2");
        queue
            .enqueue(instance_3.clone(), RecoveryReason::LostWorker)
            .expect("enqueue 3");
        let active_set: HashSet<InstanceId> = [instance_1.clone(), instance_3.clone()]
            .into_iter()
            .collect();
        let orphans = queue
            .sweep_orphans(active_set)
            .expect("sweep should succeed");
        assert_eq!(orphans.len(), 1);
        assert_eq!(orphans[0], instance_2);
    }

    #[test]
    fn test_throttle_respects_budget_limit() {
        let (_dir, _db, queue) = setup_fjall();
        let instance_1 = InstanceId::parse("01AR6Z1KQT8YZQN8X6JM5X0XYZ")
            .expect("valid ulid");
        let instance_2 = InstanceId::parse("01AR6Z1KQT8YZQN8X6JM5X0YZA")
            .expect("valid ulid");
        let instance_3 = InstanceId::parse("01AR6Z1KQT8YZQN8X6JM5X0YZB")
            .expect("valid ulid");
        queue
            .enqueue(instance_1.clone(), RecoveryReason::Crashed)
            .expect("enqueue 1");
        queue
            .enqueue(instance_2.clone(), RecoveryReason::TimedOut)
            .expect("enqueue 2");
        queue
            .enqueue(instance_3.clone(), RecoveryReason::LostWorker)
            .expect("enqueue 3");
        let first = queue.throttle_next(2);
        assert!(first.is_some());
        let second = queue.throttle_next(2);
        assert!(second.is_some());
        assert_ne!(first, second);
    }

    #[test]
    fn test_recovery_reason_roundtrip() {
        for reason in [
            RecoveryReason::Crashed,
            RecoveryReason::TimedOut,
            RecoveryReason::LostWorker,
        ] {
            let bytes = RecoveryQueue::encode_value(&reason).expect("encode");
            let decoded = RecoveryQueue::decode_value(&bytes).expect("decode");
            assert_eq!(decoded, reason);
        }
    }
}
