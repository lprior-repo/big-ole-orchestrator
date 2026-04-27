//! Persistence layer for `FailureWindow` using Fjall.
//!
//! Architecture: Data → Calc (`SerializableFailureRecord`, encode/decode)
//! → Actions (`persist_failure_window`, `load_failure_window`).
//!
//! The `failure_windows` partition stores `WorkflowName` ->
//! JSON-encoded list of serializable failure records.
//!
//! # Time handling
//!
//! `std::time::Instant` cannot be serialized (monotonic clock, process-local).
//! On persist, we convert `Instant` to `Duration` offsets relative to a reference
//! instant. On load, we reconstruct `Instant` values relative to `now`.

use std::time::{Duration, Instant};

use vo_types::{BinaryHash, WorkflowName};

#[non_exhaustive]
#[derive(Debug, PartialEq, Eq)]
pub enum FailureWindowStoreError {
    Storage { reason: String },
    CorruptValue { reason: String },
}

impl std::fmt::Display for FailureWindowStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Storage { reason } => write!(f, "storage error: {reason}"),
            Self::CorruptValue { reason } => write!(f, "corrupt failure window: {reason}"),
        }
    }
}

impl std::error::Error for FailureWindowStoreError {}

/// A serializable representation of a failure record.
/// Stores the hash and age (duration since reference point) instead of `Instant`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct SerializableFailureRecord {
    hash: String,
    age_millis: u64,
}

pub const FAILURE_WINDOWS_PARTITION: &str = "failure_windows";

fn encode_failure_window(
    records: &[FailureRecordView],
) -> Result<Vec<u8>, FailureWindowStoreError> {
    let serializable: Vec<SerializableFailureRecord> = records
        .iter()
        .map(|r| SerializableFailureRecord {
            hash: r.hash.to_string(),
            age_millis: r.age.as_millis() as u64,
        })
        .collect();
    serde_json::to_vec(&serializable).map_err(|e| FailureWindowStoreError::CorruptValue {
        reason: e.to_string(),
    })
}

fn decode_failure_window(
    bytes: &[u8],
) -> Result<Vec<SerializableFailureRecord>, FailureWindowStoreError> {
    serde_json::from_slice(bytes).map_err(|e| FailureWindowStoreError::CorruptValue {
        reason: e.to_string(),
    })
}

/// A view of a failure record suitable for persistence.
#[derive(Debug, Clone)]
pub struct FailureRecordView {
    pub hash: BinaryHash,
    pub age: Duration,
}

/// Persist failure records for a workflow to Fjall.
///
/// `reference` is the `Instant` used to calculate age offsets.
/// Typically this is "now" at the time of persist.
///
/// # Errors
/// Returns `FailureWindowStoreError::Storage` on Fjall failure.
/// Returns `FailureWindowStoreError::CorruptValue` if serialization fails.
pub fn persist_failure_window(
    partition: &fjall::Keyspace,
    workflow_name: &WorkflowName,
    records: &[FailureRecordView],
) -> Result<(), FailureWindowStoreError> {
    let key = workflow_name.as_str().as_bytes();
    let value = encode_failure_window(records)?;
    if records.is_empty() {
        partition
            .remove(key)
            .map_err(|e| FailureWindowStoreError::Storage {
                reason: e.to_string(),
            })
    } else {
        partition
            .insert(key, &value)
            .map_err(|e| FailureWindowStoreError::Storage {
                reason: e.to_string(),
            })
    }
}

/// Load failure records for a workflow from Fjall.
///
/// `now` is the `Instant` used to reconstruct `Instant` values.
/// The age offsets stored in Fjall are subtracted from `now` to
/// produce the reconstructed `Instant` for each record.
///
/// # Errors
/// Returns `FailureWindowStoreError::Storage` on Fjall failure.
/// Returns `FailureWindowStoreError::CorruptValue` if deserialization fails.
pub fn load_failure_window(
    partition: &fjall::Keyspace,
    workflow_name: &WorkflowName,
    now: Instant,
) -> Result<Vec<FailureRecordView>, FailureWindowStoreError> {
    let key = workflow_name.as_str().as_bytes();
    match partition.get(key) {
        Ok(Some(value_bytes)) => {
            let serializable = decode_failure_window(&value_bytes)?;
            serializable
                .into_iter()
                .map(|s| {
                    let hash = BinaryHash::parse(&s.hash).map_err(|e| {
                        FailureWindowStoreError::CorruptValue {
                            reason: format!("invalid binary hash: {e}"),
                        }
                    })?;
                    let age = Duration::from_millis(s.age_millis);
                    Ok(FailureRecordView { hash, age })
                })
                .collect()
        }
        Ok(None) => Ok(Vec::new()),
        Err(e) => Err(FailureWindowStoreError::Storage {
            reason: e.to_string(),
        }),
    }
}

/// Load all persisted failure windows from Fjall.
///
/// # Errors
/// Returns `FailureWindowStoreError::Storage` on Fjall scan failure.
/// Returns `FailureWindowStoreError::CorruptValue` if any stored value is malformed.
pub fn load_all_failure_windows(
    partition: &fjall::Keyspace,
    now: Instant,
) -> Result<Vec<(WorkflowName, Vec<FailureRecordView>)>, FailureWindowStoreError> {
    partition
        .iter()
        .map(|item| {
            let (key_bytes, value_bytes) = item.into_inner().map_err(|e| {
                FailureWindowStoreError::Storage {
                    reason: e.to_string(),
                }
            })?;

            let key_str = std::str::from_utf8(&key_bytes).map_err(|e| {
                FailureWindowStoreError::CorruptValue {
                    reason: format!("invalid UTF-8 key: {e}"),
                }
            })?;

            let workflow_name = WorkflowName::parse(key_str).map_err(|e| {
                FailureWindowStoreError::CorruptValue {
                    reason: format!("invalid workflow name: {e}"),
                }
            })?;

            let serializable = decode_failure_window(&value_bytes)?;
            let records: Vec<FailureRecordView> = serializable
                .into_iter()
                .map(|s| {
                    let hash = BinaryHash::parse(&s.hash).map_err(|e| {
                        FailureWindowStoreError::CorruptValue {
                            reason: format!("invalid binary hash: {e}"),
                        }
                    })?;
                    let age = Duration::from_millis(s.age_millis);
                    Ok(FailureRecordView { hash, age })
                })
                .collect::<Result<Vec<_>, FailureWindowStoreError>>()?;

            Ok((workflow_name, records))
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn setup_partition() -> (tempfile::TempDir, fjall::Database, fjall::Keyspace) {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        let partition = db
            .keyspace(FAILURE_WINDOWS_PARTITION, fjall::KeyspaceCreateOptions::default)
            .unwrap();
        (dir, db, partition)
    }

    fn workflow_name(value: &str) -> WorkflowName {
        WorkflowName::parse(value).unwrap()
    }

    fn binary_hash(value: &str) -> BinaryHash {
        BinaryHash::parse(value).unwrap()
    }

    #[test]
    fn encode_decode_round_trip_with_records() {
        let records = vec![FailureRecordView {
            hash: binary_hash("abc123"),
            age: Duration::from_millis(5000),
        }];
        let encoded = encode_failure_window(&records).unwrap();
        let decoded = decode_failure_window(&encoded).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].hash, "abc123");
        assert_eq!(decoded[0].age_millis, 5000);
    }

    #[test]
    fn encode_decode_round_trip_empty() {
        let records: Vec<FailureRecordView> = vec![];
        let encoded = encode_failure_window(&records).unwrap();
        let decoded = decode_failure_window(&encoded).unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn persist_then_load_round_trip() {
        let (_dir, _db, partition) = setup_partition();
        let wf = workflow_name("test-workflow");
        let now = Instant::now();

        let records = vec![
            FailureRecordView {
                hash: binary_hash("hash1"),
                age: Duration::from_millis(1000),
            },
            FailureRecordView {
                hash: binary_hash("hash2"),
                age: Duration::from_millis(3000),
            },
        ];

        persist_failure_window(&partition, &wf, &records).unwrap();

        let loaded = load_failure_window(&partition, &wf, now).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].hash, binary_hash("hash1"));
        assert_eq!(loaded[0].age, Duration::from_millis(1000));
        assert_eq!(loaded[1].hash, binary_hash("hash2"));
        assert_eq!(loaded[1].age, Duration::from_millis(3000));
    }

    #[test]
    fn load_returns_empty_for_missing_workflow() {
        let (_dir, _db, partition) = setup_partition();
        let wf = workflow_name("nonexistent");
        let result = load_failure_window(&partition, &wf, Instant::now()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn persist_removes_entry_when_records_empty() {
        let (_dir, _db, partition) = setup_partition();
        let wf = workflow_name("remove-me");
        let now = Instant::now();

        let records = vec![FailureRecordView {
            hash: binary_hash("abc"),
            age: Duration::from_millis(100),
        }];
        persist_failure_window(&partition, &wf, &records).unwrap();
        assert!(!load_failure_window(&partition, &wf, now).unwrap().is_empty());

        persist_failure_window(&partition, &wf, &[]).unwrap();
        assert!(load_failure_window(&partition, &wf, now).unwrap().is_empty());
    }

    #[test]
    fn load_all_failure_windows_returns_all_workflows() {
        let (_dir, _db, partition) = setup_partition();
        let now = Instant::now();

        let wf1 = workflow_name("wf-a");
        let wf2 = workflow_name("wf-b");

        persist_failure_window(
            &partition,
            &wf1,
            &[FailureRecordView {
                hash: binary_hash("h1"),
                age: Duration::from_millis(100),
            }],
        )
        .unwrap();
        persist_failure_window(
            &partition,
            &wf2,
            &[FailureRecordView {
                hash: binary_hash("h2"),
                age: Duration::from_millis(200),
            }],
        )
        .unwrap();

        let all = load_all_failure_windows(&partition, now).unwrap();
        assert_eq!(all.len(), 2);
    }
}
