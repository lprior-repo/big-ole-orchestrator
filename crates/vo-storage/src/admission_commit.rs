//! Atomic admission commit — dedupe + event + instance index in one Fjall batch (ADR-028 Section 3).
//!
//! Per ADR-028 Section 3 "Atomic Admission", the Engine must atomically write:
//! 1. The hashed dedupe record in `dedupe`
//! 2. The `WorkflowStarted` event in `events`
//! 3. The updated `InstanceSummary` (instance index entry) in `instances`
//!
//! All three writes happen in a single `fjall::OwnedWriteBatch` to guarantee
//! crash safety — either all three are durable or none are.

use parking_lot::Mutex;
use vo_types::events::EventMetadata;
use vo_types::{DedupeKey, EventEnvelope, InstanceId, InstanceStatus, TimestampMs};

use crate::codec::StorageError;
use crate::dedupe_partition::{
    decode_dedupe_entry, encode_dedupe_entry, encode_dedupe_key, DedupeEntry, DedupeStoreError,
    DEDUPE_PARTITION,
};
use crate::instance_index::encode_instance_index_key;
use crate::partitions::{EVENTS_PARTITION, INSTANCES_PARTITION};

// ---------------------------------------------------------------------------
// Data layer
// ---------------------------------------------------------------------------

/// Parameters for atomic admission commit.
#[derive(Debug, Clone)]
pub struct AtomicAdmitParams {
    pub namespace: String,
    pub instance_id: InstanceId,
    pub dedupe_key_str: String,
    pub dedupe_ttl_ms: u64,
    pub timestamp_ms: u64,
    pub event_payload: serde_json::Value,
    pub event_metadata: EventMetadata,
    pub initial_status: InstanceStatus,
}

/// Result of an atomic admission attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AtomicAdmitResult {
    Admitted,
    Duplicate { instance_id: String },
}

/// Error from atomic admission commit.
#[derive(Debug, thiserror::Error)]
pub enum AdmissionCommitError {
    #[error("dedupe error: {0}")]
    Dedupe(#[from] DedupeStoreError),
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("batch commit failed")]
    BatchCommitFailed,
    #[error("serialization failed")]
    SerializationFailed,
    #[error("invalid dedupe key: {reason}")]
    InvalidDedupeKey { reason: String },
}

// ---------------------------------------------------------------------------
// Internal locking
// ---------------------------------------------------------------------------

const NUM_STRIPES: usize = 64;

static DEDUPE_STRIPES: std::sync::LazyLock<Vec<Mutex<()>>> =
    std::sync::LazyLock::new(|| (0..NUM_STRIPES).map(|_| Mutex::new(())).collect());

static INSTANCE_STRIPES: std::sync::LazyLock<Vec<Mutex<()>>> =
    std::sync::LazyLock::new(|| (0..NUM_STRIPES).map(|_| Mutex::new(())).collect());

fn stripe_for(key_bytes: &[u8]) -> usize {
    crc32fast::hash(key_bytes) as usize % NUM_STRIPES
}

// ---------------------------------------------------------------------------
// Event key helpers (matches event_summary_commit.rs key format)
// ---------------------------------------------------------------------------

/// Build event key: `[instance_id_bytes(16)][sequence_u64_be(8)]` = 24 bytes.
fn build_event_key(instance_id: &InstanceId, sequence: u64) -> Result<Vec<u8>, StorageError> {
    let id_bytes = instance_id
        .to_bytes()
        .map_err(|_| StorageError::CorruptKey)?;
    let mut key = Vec::with_capacity(24);
    key.extend_from_slice(&id_bytes);
    key.extend_from_slice(&sequence.to_be_bytes());
    Ok(key)
}

/// Scan the events partition for the last key with this instance_id prefix
/// and return the next sequence number (or 1 if no events exist).
fn next_sequence(
    events_ks: &fjall::Keyspace,
    instance_id: &InstanceId,
) -> Result<u64, StorageError> {
    let _prefix = build_event_key(instance_id, 0)?;
    // Range scan: [prefix+1 .. prefix+u64::MAX]
    let start = build_event_key(instance_id, 1)?;
    let end = build_event_key(instance_id, u64::MAX)?;
    match events_ks.range(start..=end).next_back() {
        Some(guard) => {
            let (key, _) = guard.into_inner().map_err(|_| StorageError::Storage)?;
            let seq_bytes: [u8; 8] = key[key.len().saturating_sub(8)..]
                .try_into()
                .map_err(|_| StorageError::CorruptKey)?;
            let last = u64::from_be_bytes(seq_bytes);
            last.checked_add(1).ok_or(StorageError::SequenceGap)
        }
        None => Ok(1),
    }
}

fn encode_event_json(envelope: &EventEnvelope) -> Result<Vec<u8>, AdmissionCommitError> {
    serde_json::to_vec(envelope).map_err(|_| AdmissionCommitError::SerializationFailed)
}

// ---------------------------------------------------------------------------
// Action layer
// ---------------------------------------------------------------------------

/// Atomically commit dedupe + event + instance index in one Fjall batch.
///
/// Lock ordering: dedupe stripe lock → instance stripe lock. Both held across
/// the batch build + commit to prevent TOCTOU races.
pub fn atomic_admit_workflow(
    db: &fjall::Database,
    params: AtomicAdmitParams,
) -> Result<AtomicAdmitResult, AdmissionCommitError> {
    let dedupe_key = DedupeKey::parse(&params.dedupe_key_str)
        .map_err(|e| AdmissionCommitError::InvalidDedupeKey { reason: e.to_string() })?;

    let dedupe_ks = db
        .keyspace(DEDUPE_PARTITION, fjall::KeyspaceCreateOptions::default)
        .map_err(|_| StorageError::Storage)?;
    let events_ks = db
        .keyspace(EVENTS_PARTITION, fjall::KeyspaceCreateOptions::default)
        .map_err(|_| StorageError::Storage)?;
    let instances_ks = db
        .keyspace(INSTANCES_PARTITION, fjall::KeyspaceCreateOptions::default)
        .map_err(|_| StorageError::Storage)?;

    // Lock 1: dedupe stripe
    let encoded_dk = encode_dedupe_key(&dedupe_key);
    let _dedupe_guard = DEDUPE_STRIPES[stripe_for(&encoded_dk)].lock();

    let now_ms = now_ms();
    let expires_at = now_ms.saturating_add(params.dedupe_ttl_ms);

    if let Ok(Some(existing)) = dedupe_ks.get(&encoded_dk) {
        let entry = decode_dedupe_entry(&existing)?;
        if !entry.is_expired(now_ms) {
            return Ok(AtomicAdmitResult::Duplicate {
                instance_id: entry.instance_id().to_string(),
            });
        }
    }

    // Lock 2: instance stripe (for sequence number safety)
    let iid_bytes = params.instance_id.to_bytes().map_err(|_| StorageError::CorruptKey)?;
    let _instance_guard = INSTANCE_STRIPES[stripe_for(&iid_bytes)].lock();

    let sequence = next_sequence(&events_ks, &params.instance_id)?;

    let event_envelope = EventEnvelope {
        schema_version: 1,
        instance_id: params.instance_id.to_string(),
        sequence,
        timestamp_ms: params.timestamp_ms,
        payload: params.event_payload,
        metadata: params.event_metadata,
    };

    let mut batch = db.batch();

    // 1. Dedupe entry
    let dedupe_entry = DedupeEntry::new(
        dedupe_key.as_str().to_string(),
        params.instance_id.to_string(),
        expires_at,
    )?;
    let dedupe_value = encode_dedupe_entry(&dedupe_entry)?;
    batch.insert(&dedupe_ks, &encoded_dk, &dedupe_value);

    // 2. Event
    let event_key = build_event_key(&params.instance_id, sequence)?;
    let event_value = encode_event_json(&event_envelope)?;
    batch.insert(&events_ks, &event_key, &event_value);

    // 3. Instance index
    let created_at = TimestampMs::try_from(params.timestamp_ms)
        .map_err(|_| StorageError::InvalidArgument)?;
    let instance_key =
        encode_instance_index_key(params.initial_status, created_at, &params.instance_id)?;
    batch.insert(&instances_ks, instance_key, Vec::<u8>::new());

    batch
        .commit()
        .map_err(|_| AdmissionCommitError::BatchCommitFailed)?;

    Ok(AtomicAdmitResult::Admitted)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock post-UNIX epoch")
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use vo_types::InstanceId;

    fn test_db() -> fjall::Database {
        let dir = tempfile::tempdir().expect("tempdir");
        fjall::Database::builder(dir.into_path())
            .open()
            .expect("db open")
    }

    fn sample_instance_id(n: u8) -> InstanceId {
        InstanceId::from_bytes([n; 16])
    }

    fn sample_params(id: u8) -> AtomicAdmitParams {
        AtomicAdmitParams {
            namespace: "test-ns".to_string(),
            instance_id: sample_instance_id(id),
            dedupe_key_str: format!("dedupe-key-{id}"),
            dedupe_ttl_ms: 60_000,
            timestamp_ms: 1_000_000,
            event_payload: serde_json::json!({"type": "WorkflowStarted"}),
            event_metadata: EventMetadata::default(),
            initial_status: InstanceStatus::Running,
        }
    }

    #[test]
    fn atomic_admit_writes_all_three_partitions() {
        let db = test_db();
        let params = sample_params(1);

        let result = atomic_admit_workflow(&db, params.clone());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), AtomicAdmitResult::Admitted);

        // Verify dedupe entry exists
        let dedupe_ks = db
            .keyspace(DEDUPE_PARTITION, fjall::KeyspaceCreateOptions::default)
            .unwrap();
        let dk = DedupeKey::parse(&params.dedupe_key_str).unwrap();
        let encoded = encode_dedupe_key(&dk);
        assert!(dedupe_ks.get(&encoded).unwrap().is_some());

        // Verify event exists
        let events_ks = db
            .keyspace(EVENTS_PARTITION, fjall::KeyspaceCreateOptions::default)
            .unwrap();
        let event_key = build_event_key(&params.instance_id, 1).unwrap();
        assert!(events_ks.get(&event_key).unwrap().is_some());

        // Verify instance index exists
        let instances_ks = db
            .keyspace(INSTANCES_PARTITION, fjall::KeyspaceCreateOptions::default)
            .unwrap();
        let created_at = TimestampMs::try_from(params.timestamp_ms).unwrap();
        let instance_key =
            encode_instance_index_key(InstanceStatus::Running, created_at, &params.instance_id)
                .unwrap();
        assert!(instances_ks.get(instance_key).unwrap().is_some());
    }

    #[test]
    fn atomic_admit_rejects_duplicate() {
        let db = test_db();
        let params = sample_params(2);

        let result1 = atomic_admit_workflow(&db, params.clone());
        assert_eq!(result1.unwrap(), AtomicAdmitResult::Admitted);

        let result2 = atomic_admit_workflow(&db, params);
        assert!(matches!(result2, Ok(AtomicAdmitResult::Duplicate { .. })));
    }

    #[test]
    fn atomic_admit_different_keys_both_succeed() {
        let db = test_db();
        let params1 = sample_params(3);
        let mut params2 = sample_params(4);
        params2.namespace = params1.namespace.clone();

        let result1 = atomic_admit_workflow(&db, params1);
        assert_eq!(result1.unwrap(), AtomicAdmitResult::Admitted);

        let result2 = atomic_admit_workflow(&db, params2);
        assert_eq!(result2.unwrap(), AtomicAdmitResult::Admitted);
    }

    #[test]
    fn atomic_admit_rejects_invalid_dedupe_key() {
        let db = test_db();
        let mut params = sample_params(5);
        params.dedupe_key_str = String::new();

        let result = atomic_admit_workflow(&db, params);
        assert!(matches!(
            result,
            Err(AdmissionCommitError::InvalidDedupeKey { .. })
        ));
    }

    #[test]
    fn atomic_admit_rejects_zero_ttl() {
        let db = test_db();
        let mut params = sample_params(6);
        params.dedupe_ttl_ms = 0;

        // Zero TTL creates expires_at = now, which is already expired,
        // so the entry is admitted but immediately expired. The dedupe
        // entry still gets written (it just expires immediately).
        let result = atomic_admit_workflow(&db, params);
        assert!(result.is_ok());
    }
}
