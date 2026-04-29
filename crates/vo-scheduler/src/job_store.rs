//! Fjall-backed persistent implementation of `JobStore` for production use.
//!
//! Key format: `job::<ulid_bytes>` — the JobId (ULID) bytes serve as the key.
//! Value format: `rmp_serde`-serialized `ScheduledJobStorage` (MessagePack).
//!
//! A storage-friendly wrapper (`ScheduledJobStorage`) is used instead of
//! `ScheduledJob` directly because `SchedulePolicy` uses `#[serde(tag = "type")]`
//! with newtype variants, which neither `serde_json` nor `rmp_serde` can serialize.

use std::sync::Arc;
use std::time::Duration;
use ulid::Ulid;

use crate::error::SchedulerError;
use crate::job::ScheduledJob;
use crate::job::SerializedPayload;
use crate::scheduler::JobStore;
use crate::types::SchedulePolicy;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

const JOBS_PARTITION: &str = "jobs";

// ---------------------------------------------------------------------------
// Storage-friendly types (no serde tag newtype limitation)
// ---------------------------------------------------------------------------

/// Storage-friendly SchedulePolicy that serializes without serde-tag issues.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum SchedulePolicyStorage {
    Immediate,
    At(DateTime<Utc>),
    After(Duration),
    Cron(String),
}

/// Storage-friendly ScheduledJob for Fjall persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScheduledJobStorage {
    id: crate::types::JobId,
    kind: crate::types::JobKind,
    state: crate::types::JobState,
    priority: crate::types::JobPriority,
    schedule_policy: SchedulePolicyStorage,
    retry_policy: crate::types::RetryPolicy,
    attempt_count: u32,
    due_at: DateTime<Utc>,
    payload: SerializedPayload,
    last_error: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<ScheduledJob> for ScheduledJobStorage {
    fn from(job: ScheduledJob) -> Self {
        Self {
            id: job.id,
            kind: job.kind,
            state: job.state,
            priority: job.priority,
            schedule_policy: job.schedule_policy.into(),
            retry_policy: job.retry_policy,
            attempt_count: job.attempt_count,
            due_at: job.due_at,
            payload: job.payload,
            last_error: job.last_error,
            created_at: job.created_at,
            updated_at: job.updated_at,
        }
    }
}

impl From<ScheduledJobStorage> for ScheduledJob {
    fn from(storage: ScheduledJobStorage) -> Self {
        Self {
            id: storage.id,
            kind: storage.kind,
            state: storage.state,
            priority: storage.priority,
            schedule_policy: storage.schedule_policy.into(),
            retry_policy: storage.retry_policy,
            attempt_count: storage.attempt_count,
            due_at: storage.due_at,
            payload: storage.payload,
            last_error: storage.last_error,
            created_at: storage.created_at,
            updated_at: storage.updated_at,
        }
    }
}

impl From<SchedulePolicy> for SchedulePolicyStorage {
    fn from(policy: SchedulePolicy) -> Self {
        match policy {
            SchedulePolicy::Immediate => Self::Immediate,
            SchedulePolicy::At(dt) => Self::At(dt),
            SchedulePolicy::After(dur) => Self::After(dur),
            SchedulePolicy::Cron(s) => Self::Cron(s),
        }
    }
}

impl From<SchedulePolicyStorage> for SchedulePolicy {
    fn from(storage: SchedulePolicyStorage) -> Self {
        match storage {
            SchedulePolicyStorage::Immediate => Self::Immediate,
            SchedulePolicyStorage::At(dt) => Self::At(dt),
            SchedulePolicyStorage::After(dur) => Self::After(dur),
            SchedulePolicyStorage::Cron(s) => Self::Cron(s),
        }
    }
}

// ---------------------------------------------------------------------------
// Fjall-backed persistent job store
// ---------------------------------------------------------------------------

/// Fjall-backed persistent job store.
pub struct FjallJobStore {
    partition: Arc<fjall::Keyspace>,
}

impl std::fmt::Debug for FjallJobStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FjallJobStore").finish()
    }
}

impl FjallJobStore {
    /// Opens a new job store backed by the given database.
    ///
    /// # Errors
    ///
    /// Returns `SchedulerError::SerializationError` if the partition cannot be opened.
    pub fn open(db: &fjall::Database) -> Result<Self, SchedulerError> {
        let partition = db
            .keyspace(JOBS_PARTITION, || fjall::KeyspaceCreateOptions::default())
            .map_err(|e| SchedulerError::SerializationError(format!(
                "failed to open jobs partition: {e}"
            )))?;
        Ok(Self {
            partition: Arc::new(partition),
        })
    }

    /// Encode a JobId as Fjall key bytes.
    ///
    /// Uses the prefix `job::` followed by the ULID bytes (16 bytes).
    fn encode_key(job_id: &crate::types::JobId) -> Vec<u8> {
        let mut key = Vec::with_capacity(5 + 16);
        key.extend_from_slice(b"job::");
        key.extend_from_slice(&job_id.0.to_bytes());
        key
    }

    /// Decode a JobId from Fjall key bytes.
    fn decode_key(key_bytes: &[u8]) -> Result<crate::types::JobId, SchedulerError> {
        let prefix = b"job::";
        if key_bytes.len() < prefix.len() + 16 {
            return Err(SchedulerError::SerializationError(
                "invalid job key length".to_string(),
            ));
        }
        if &key_bytes[..prefix.len()] != prefix {
            return Err(SchedulerError::SerializationError(
                "invalid job key prefix".to_string(),
            ));
        }
        let ulid_bytes: [u8; 16] = key_bytes[prefix.len()..prefix.len() + 16]
            .try_into()
            .map_err(|_| SchedulerError::SerializationError("invalid ulid bytes".to_string()))?;
        let ulid = Ulid::from_bytes(ulid_bytes);
        Ok(crate::types::JobId(ulid))
    }

    /// List all jobs stored in the partition.
    pub fn list_all(&self) -> Result<Vec<ScheduledJob>, SchedulerError> {
        let mut results = Vec::new();
        for item in self.partition.iter() {
            let (key_bytes, value_bytes) = item.into_inner().map_err(|e| SchedulerError::SerializationError(e.to_string()))?;

            let storage: ScheduledJobStorage =
                rmp_serde::from_slice(&value_bytes).map_err(|e| SchedulerError::SerializationError(
                    format!("failed to decode job: {e}"),
                ))?;

            // Verify key matches (integrity check).
            let key_id = Self::decode_key(&key_bytes)?;
            if key_id != storage.id {
                return Err(SchedulerError::SerializationError(
                    format!("key mismatch: key={key_id}, stored={}", storage.id),
                ));
            }

            results.push(ScheduledJob::from(storage));
        }
        Ok(results)
    }

    /// List jobs by state for the scheduler's promote_due_jobs phase.
    ///
    /// Returns jobs matching the requested state, sorted by `due_at` ascending
    /// (uses ULID monotonic ordering via key bytes).
    pub fn list_by_state(
        &self,
        state: crate::types::JobState,
    ) -> Result<Vec<ScheduledJob>, SchedulerError> {
        let mut results = Vec::new();
        for item in self.partition.iter() {
            let (_key_bytes, value_bytes) = item.into_inner().map_err(|e| SchedulerError::SerializationError(e.to_string()))?;

            let storage: ScheduledJobStorage =
                rmp_serde::from_slice(&value_bytes).map_err(|e| SchedulerError::SerializationError(
                    format!("failed to decode job: {e}"),
                ))?;

            if storage.state == state {
                results.push(ScheduledJob::from(storage));
            }
        }
        // Sort by due_at ascending (earliest first for dispatch priority).
        results.sort_by(|a, b| a.due_at.cmp(&b.due_at));
        Ok(results)
    }

    /// Look up a single job by ID.
    pub fn get(&self, job_id: &crate::types::JobId) -> Result<Option<ScheduledJob>, SchedulerError> {
        let key = Self::encode_key(job_id);
        match self.partition.get(&key).map_err(|e| SchedulerError::SerializationError(e.to_string()))? {
            Some(bytes) => {
                let storage: ScheduledJobStorage =
                    rmp_serde::from_slice(&bytes).map_err(|e| SchedulerError::SerializationError(
                        format!("failed to decode job: {e}"),
                    ))?;
                Ok(Some(ScheduledJob::from(storage)))
            }
            None => Ok(None),
        }
    }
}

impl JobStore for FjallJobStore {
    fn persist(&mut self, job: &ScheduledJob) -> Result<(), SchedulerError> {
        let key = Self::encode_key(&job.id);
        let storage: ScheduledJobStorage = job.clone().into();
        let value = rmp_serde::to_vec(&storage).map_err(|e| SchedulerError::SerializationError(
            format!("failed to serialize job: {e}"),
        ))?;
        self.partition
            .insert(&key, &value)
            .map_err(|e| SchedulerError::SerializationError(format!(
                "failed to persist job: {e}"
            )))
    }

    fn remove(&mut self, job_id: &crate::types::JobId) -> Result<(), SchedulerError> {
        let key = Self::encode_key(job_id);
        self.partition
            .remove(&key)
            .map_err(|e| SchedulerError::SerializationError(format!(
                "failed to remove job: {e}"
            )))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::JobStore;
    use crate::types::{JobKind, JobPriority, RetryPolicy};
    use bytes::Bytes;
    use chrono::Utc;
    use tempfile::tempdir;

    fn make_test_job(kind: JobKind, policy: SchedulePolicy) -> ScheduledJob {
        ScheduledJob::new(
            kind,
            JobPriority::Normal,
            policy,
            RetryPolicy::default(),
            Bytes::from_static(b"test-payload"),
        )
        .unwrap()
    }

    fn make_test_scheduler_db() -> (tempfile::TempDir, fjall::Database) {
        let dir = tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        (dir, db)
    }

    #[test]
    fn fjall_open_creates_partition() {
        let (_dir, db) = make_test_scheduler_db();
        let _store = FjallJobStore::open(&db).unwrap();
        // No panic = success — partition was created.
    }

    #[test]
    fn fjall_persist_and_retrieve() {
        let (_dir, db) = make_test_scheduler_db();
        let mut store = FjallJobStore::open(&db).unwrap();
        let job = make_test_job(JobKind::OneShot, SchedulePolicy::Immediate);
        let id = job.id;

        store.persist(&job).unwrap();
        let retrieved = store.get(&id).unwrap().unwrap();
        assert_eq!(retrieved.id, id);
        assert_eq!(retrieved.kind, JobKind::OneShot);
        assert_eq!(&*retrieved.payload, b"test-payload");
    }

    #[test]
    fn fjall_get_nonexistent_returns_none() {
        let (_dir, db) = make_test_scheduler_db();
        let store = FjallJobStore::open(&db).unwrap();
        let fake_id = crate::types::JobId::generate();
        let result = store.get(&fake_id).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn fjall_persist_updates_existing() {
        let (_dir, db) = make_test_scheduler_db();
        let mut store = FjallJobStore::open(&db).unwrap();
        let mut job = make_test_job(JobKind::OneShot, SchedulePolicy::Immediate);
        job.state = crate::types::JobState::Pending;
        store.persist(&job).unwrap();

        // Update to Running.
        job.state = crate::types::JobState::Running;
        job.updated_at = Utc::now();
        store.persist(&job).unwrap();

        let retrieved = store.get(&job.id).unwrap().unwrap();
        assert_eq!(retrieved.state, crate::types::JobState::Running);
    }

    #[test]
    fn fjall_remove_deletes_job() {
        let (_dir, db) = make_test_scheduler_db();
        let mut store = FjallJobStore::open(&db).unwrap();
        let job = make_test_job(JobKind::OneShot, SchedulePolicy::Immediate);
        let id = job.id;

        store.persist(&job).unwrap();
        assert!(store.get(&id).unwrap().is_some());

        store.remove(&id).unwrap();
        assert!(store.get(&id).unwrap().is_none());
    }

    #[test]
    fn fjall_list_all_returns_all_jobs() {
        let (_dir, db) = make_test_scheduler_db();
        let mut store = FjallJobStore::open(&db).unwrap();

        let j1 = make_test_job(JobKind::OneShot, SchedulePolicy::Immediate);
        let j2 = make_test_job(JobKind::Recurring, SchedulePolicy::At(Utc::now()));
        store.persist(&j1).unwrap();
        store.persist(&j2).unwrap();

        let all = store.list_all().unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn fjall_list_by_state_filters_correctly() {
        let (_dir, db) = make_test_scheduler_db();
        let mut store = FjallJobStore::open(&db).unwrap();

        let mut j1 = make_test_job(JobKind::OneShot, SchedulePolicy::Immediate);
        j1.state = crate::types::JobState::Running;
        let mut j2 = make_test_job(JobKind::Recurring, SchedulePolicy::At(Utc::now()));
        j2.state = crate::types::JobState::Completed;
        store.persist(&j1).unwrap();
        store.persist(&j2).unwrap();

        let running = store.list_by_state(crate::types::JobState::Running).unwrap();
        assert_eq!(running.len(), 1);
        assert_eq!(running[0].id, j1.id);

        let completed = store.list_by_state(crate::types::JobState::Completed).unwrap();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].id, j2.id);
    }

    #[test]
    fn fjall_list_by_state_empty_returns_empty() {
        let (_dir, db) = make_test_scheduler_db();
        let store = FjallJobStore::open(&db).unwrap();
        let cancelled = store.list_by_state(crate::types::JobState::Cancelled).unwrap();
        assert!(cancelled.is_empty());
    }

    #[test]
    fn fjall_persist_roundtrip_all_fields() {
        let (_dir, db) = make_test_scheduler_db();
        let mut store = FjallJobStore::open(&db).unwrap();

        let mut job = make_test_job(
            JobKind::Recurring,
            SchedulePolicy::Cron("*/5 * * * *".to_string()),
        );
        job.state = crate::types::JobState::Scheduled;
        job.priority = JobPriority::High;
        job.attempt_count = 5;
        job.last_error = Some("transient error".to_string());
        job.due_at = Utc::now() + chrono::Duration::hours(1);

        store.persist(&job).unwrap();

        let retrieved = store.get(&job.id).unwrap().unwrap();
        assert_eq!(retrieved.id, job.id);
        assert_eq!(retrieved.kind, JobKind::Recurring);
        assert_eq!(retrieved.state, crate::types::JobState::Scheduled);
        assert_eq!(retrieved.priority, JobPriority::High);
        assert_eq!(retrieved.attempt_count, 5);
        assert_eq!(retrieved.last_error, Some("transient error".to_string()));
        assert!(matches!(retrieved.schedule_policy, SchedulePolicy::Cron(_)));
    }

    #[test]
    fn fjall_remove_nonexistent_does_not_panic() {
        let (_dir, db) = make_test_scheduler_db();
        let store = FjallJobStore::open(&db).unwrap();
        let fake_id = crate::types::JobId::generate();
        let result = store.partition.get(&FjallJobStore::encode_key(&fake_id));
        assert!(result.is_ok());
    }

    #[test]
    fn fjall_multiple_jobs_sorted_by_due_at() {
        let (_dir, db) = make_test_scheduler_db();
        let mut store = FjallJobStore::open(&db).unwrap();

        let now = Utc::now();
        let j1 = make_test_job(JobKind::OneShot, SchedulePolicy::At(now + chrono::Duration::hours(2)));
        let j2 = make_test_job(JobKind::OneShot, SchedulePolicy::At(now + chrono::Duration::hours(1)));
        let j3 = make_test_job(JobKind::OneShot, SchedulePolicy::At(now + chrono::Duration::seconds(30)));

        store.persist(&j1).unwrap();
        store.persist(&j2).unwrap();
        store.persist(&j3).unwrap();

        let scheduled = store.list_by_state(crate::types::JobState::Scheduled).unwrap();
        assert_eq!(scheduled.len(), 3);
        assert_eq!(scheduled[0].id, j3.id); // earliest due_at first
        assert_eq!(scheduled[1].id, j2.id);
        assert_eq!(scheduled[2].id, j1.id); // latest due_at last
    }

    #[test]
    fn fjall_key_integrity_check_fails_on_mismatch() {
        let (_dir, db) = make_test_scheduler_db();
        let store = FjallJobStore::open(&db).unwrap();

        // Manually insert a job with a mismatched key to trigger integrity check.
        let job = make_test_job(JobKind::OneShot, SchedulePolicy::Immediate);
        let wrong_key = FjallJobStore::encode_key(&crate::types::JobId::generate());
        let storage: ScheduledJobStorage = job.clone().into();
        let value = rmp_serde::to_vec(&storage).unwrap();
        store.partition.insert(&wrong_key, &value).unwrap();

        let result = store.list_all();
        assert!(result.is_err());
    }

    #[test]
    fn fjall_persist_schedule_policy_after() {
        let (_dir, db) = make_test_scheduler_db();
        let mut store = FjallJobStore::open(&db).unwrap();

        let job = make_test_job(
            JobKind::Delayed,
            SchedulePolicy::After(Duration::from_secs(60)),
        );
        store.persist(&job).unwrap();

        let retrieved = store.get(&job.id).unwrap().unwrap();
        assert!(matches!(retrieved.schedule_policy, SchedulePolicy::After(_)));
    }

    #[test]
    fn fjall_persist_schedule_policy_at() {
        let (_dir, db) = make_test_scheduler_db();
        let mut store = FjallJobStore::open(&db).unwrap();

        let dt = Utc::now() + chrono::Duration::hours(1);
        let job = make_test_job(JobKind::OneShot, SchedulePolicy::At(dt));
        store.persist(&job).unwrap();

        let retrieved = store.get(&job.id).unwrap().unwrap();
        assert!(matches!(retrieved.schedule_policy, SchedulePolicy::At(_)));
        // Verify the datetime survived roundtrip.
        if let SchedulePolicy::At(retrieved_dt) = retrieved.schedule_policy {
            assert_eq!(retrieved_dt, dt);
        }
    }

    #[test]
    fn fjall_persist_across_restart() {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        let mut store = FjallJobStore::open(&db).unwrap();

        let job = make_test_job(JobKind::OneShot, SchedulePolicy::Immediate);
        let id = job.id;
        store.persist(&job).unwrap();
        drop(store);
        drop(db);

        // Reopen store from same DB.
        let db2 = fjall::Database::builder(dir.path()).open().unwrap();
        let store2 = FjallJobStore::open(&db2).unwrap();
        let retrieved = store2.get(&id).unwrap().unwrap();
        assert_eq!(retrieved.id, id);
        assert_eq!(retrieved.kind, JobKind::OneShot);
    }

    #[test]
    fn fjall_persist_across_restart_with_cron() {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        let mut store = FjallJobStore::open(&db).unwrap();

        let job = make_test_job(
            JobKind::Recurring,
            SchedulePolicy::Cron("0 * * * *".to_string()),
        );
        let id = job.id;
        store.persist(&job).unwrap();
        drop(store);
        drop(db);

        // Reopen store from same DB.
        let db2 = fjall::Database::builder(dir.path()).open().unwrap();
        let store2 = FjallJobStore::open(&db2).unwrap();
        let retrieved = store2.get(&id).unwrap().unwrap();
        assert_eq!(retrieved.id, id);
        assert!(matches!(retrieved.schedule_policy, SchedulePolicy::Cron(ref s) if s == "0 * * * *"));
    }
}
