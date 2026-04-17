//! Lineage routing store — maps `lineage_id` → `LineageRecord` for continue-as-new (ADR-038).
//!
//! Architecture: Data (`LineageRecord`, `LineageStoreError`) → Calc (`encode_lineage_key`,
//! `decode_lineage_key`) → Actions (`get_active_epoch`, `upsert_lineage`, `record_rollover`).
//!
//! The `lineage` partition stores `lineage_id` -> JSON-encoded `LineageRecord` so the engine
//! can route signals and queries to the currently active epoch.

use crate::codec::StorageError;
use serde::{Deserialize, Serialize};
use vo_types::{Epoch, InstanceId};

// ---------------------------------------------------------------------------
// Data layer — types
// ---------------------------------------------------------------------------

/// Persisted record for a workflow lineage.
///
/// Maps a stable `lineage_id` to the currently active epoch and its backing
/// instance, enabling signal routing across continue-as-new boundaries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageRecord {
    /// Stable lineage identifier (persists across epoch rollovers).
    pub lineage_id: String,
    /// Currently active epoch number.
    pub active_epoch: Epoch,
    /// Instance ID backing the currently active epoch.
    pub active_instance_id: InstanceId,
    /// Instance ID of the previous epoch (set after first rollover).
    pub previous_instance_id: Option<InstanceId>,
}

// ---------------------------------------------------------------------------
// Data layer — error enum
// ---------------------------------------------------------------------------

/// Errors from the lineage store.
#[non_exhaustive]
#[derive(Debug, PartialEq, Eq)]
pub enum LineageStoreError {
    /// Fjall storage operation failed.
    Storage { reason: String },

    /// Stored value could not be decoded.
    CorruptValue { reason: String },
}

impl std::fmt::Display for LineageStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Storage { reason } => write!(f, "lineage store error: {reason}"),
            Self::CorruptValue { reason } => write!(f, "corrupt lineage record: {reason}"),
        }
    }
}

impl std::error::Error for LineageStoreError {}

impl From<StorageError> for LineageStoreError {
    fn from(e: StorageError) -> Self {
        Self::Storage {
            reason: e.to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Calc layer — pure encode/decode
// ---------------------------------------------------------------------------

/// The partition name used for lineage routing records.
pub const LINEAGE_PARTITION: &str = "lineage";

/// Decode JSON bytes into a `LineageRecord`.
///
/// # Errors
///
/// Returns `LineageStoreError::CorruptValue` if the bytes are not valid JSON
/// or do not represent a valid `LineageRecord`.
pub fn decode_lineage_record(bytes: &[u8]) -> Result<LineageRecord, LineageStoreError> {
    serde_json::from_slice(bytes).map_err(|e| LineageStoreError::CorruptValue {
        reason: e.to_string(),
    })
}

/// Encode a `LineageRecord` to JSON bytes.
///
/// # Errors
///
/// Returns `LineageStoreError::CorruptValue` if serialization fails.
pub fn encode_lineage_record(record: &LineageRecord) -> Result<Vec<u8>, LineageStoreError> {
    serde_json::to_vec(record).map_err(|e| LineageStoreError::CorruptValue {
        reason: e.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Actions layer — Fjall operations
// ---------------------------------------------------------------------------

/// Retrieve the lineage record for a given `lineage_id`.
///
/// # Returns
///
/// - `Ok(Some(record))` if the lineage exists.
/// - `Ok(None)` if no entry exists.
///
/// # Errors
///
/// Returns `LineageStoreError::Storage` on Fjall failure.
/// Returns `LineageStoreError::CorruptValue` if the stored value is malformed.
pub fn get_lineage_record(
    partition: &fjall::Keyspace,
    lineage_id: &str,
) -> Result<Option<LineageRecord>, LineageStoreError> {
    let key = lineage_id.as_bytes();
    match partition.get(key) {
        Ok(Some(value_bytes)) => {
            let record = decode_lineage_record(&value_bytes)?;
            Ok(Some(record))
        }
        Ok(None) => Ok(None),
        Err(e) => Err(LineageStoreError::Storage {
            reason: e.to_string(),
        }),
    }
}

/// Insert or update the lineage record for a given `lineage_id`.
///
/// # Errors
///
/// Returns `LineageStoreError::Storage` on Fjall failure.
/// Returns `LineageStoreError::CorruptValue` if serialization fails.
pub fn upsert_lineage_record(
    partition: &fjall::Keyspace,
    lineage_id: &str,
    record: &LineageRecord,
) -> Result<(), LineageStoreError> {
    let key = lineage_id.as_bytes();
    let value = encode_lineage_record(record)?;
    partition
        .insert(key, &value)
        .map_err(|e| LineageStoreError::Storage {
            reason: e.to_string(),
        })
}

/// Atomically record a continue-as-new rollover: update the active epoch and
/// shift the previous instance.
///
/// # Errors
///
/// Returns `LineageStoreError::Storage` on Fjall failure.
/// Returns `LineageStoreError::CorruptValue` if serialization fails.
pub fn record_rollover(
    db: &fjall::Database,
    lineage_id: &str,
    new_epoch: Epoch,
    new_instance_id: InstanceId,
) -> Result<(), LineageStoreError> {
    let partition = db
        .keyspace(LINEAGE_PARTITION, fjall::KeyspaceCreateOptions::default)
        .map_err(|_| LineageStoreError::Storage {
            reason: "failed to open lineage partition".to_string(),
        })?;

    let key = lineage_id.as_bytes();

    // Read current record to get previous instance ID
    let previous_instance_id = match partition.get(key) {
        Ok(Some(bytes)) => {
            let current = decode_lineage_record(&bytes)?;
            Some(current.active_instance_id)
        }
        Ok(None) => None,
        Err(e) => {
            return Err(LineageStoreError::Storage {
                reason: e.to_string(),
            });
        }
    };

    let updated = LineageRecord {
        lineage_id: lineage_id.to_string(),
        active_epoch: new_epoch,
        active_instance_id: new_instance_id,
        previous_instance_id,
    };

    let value = encode_lineage_record(&updated)?;
    partition
        .insert(key, &value)
        .map_err(|e| LineageStoreError::Storage {
            reason: e.to_string(),
        })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn setup_partition() -> (tempfile::TempDir, fjall::Database, fjall::Keyspace) {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        let partition = db
            .keyspace(LINEAGE_PARTITION, fjall::KeyspaceCreateOptions::default)
            .unwrap();
        (dir, db, partition)
    }

    fn test_instance_id() -> InstanceId {
        InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap()
    }

    fn test_instance_id_2() -> InstanceId {
        InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap()
    }

    // -----------------------------------------------------------------------
    // Calc layer tests
    // -----------------------------------------------------------------------

    #[test]
    fn encode_lineage_record_returns_valid_json() {
        let record = LineageRecord {
            lineage_id: "lin-1".to_string(),
            active_epoch: Epoch::ZERO,
            active_instance_id: test_instance_id(),
            previous_instance_id: None,
        };
        let bytes = encode_lineage_record(&record).unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["lineage_id"], "lin-1");
        assert_eq!(json["active_epoch"], 0);
        assert_eq!(json["previous_instance_id"], serde_json::Value::Null);
    }

    #[test]
    fn decode_lineage_record_roundtrips_with_encode() {
        let original = LineageRecord {
            lineage_id: "lin-rt".to_string(),
            active_epoch: Epoch::new(3),
            active_instance_id: test_instance_id(),
            previous_instance_id: Some(test_instance_id_2()),
        };
        let bytes = encode_lineage_record(&original).unwrap();
        let restored = decode_lineage_record(&bytes).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn decode_lineage_record_returns_corrupt_value_for_invalid_json() {
        let result = decode_lineage_record(b"not-json");
        assert!(matches!(
            result,
            Err(LineageStoreError::CorruptValue { .. })
        ));
    }

    // -----------------------------------------------------------------------
    // Actions layer tests
    // -----------------------------------------------------------------------

    #[test]
    fn get_lineage_record_returns_none_when_not_found() {
        let (_dir, _ks, partition) = setup_partition();
        let result = get_lineage_record(&partition, "nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn upsert_then_get_returns_stored_record() {
        let (_dir, _ks, partition) = setup_partition();
        let record = LineageRecord {
            lineage_id: "lin-1".to_string(),
            active_epoch: Epoch::ZERO,
            active_instance_id: test_instance_id(),
            previous_instance_id: None,
        };
        upsert_lineage_record(&partition, "lin-1", &record).unwrap();
        let loaded = get_lineage_record(&partition, "lin-1").unwrap().unwrap();
        assert_eq!(loaded, record);
    }

    #[test]
    fn upsert_overwrites_previous_record() {
        let (_dir, _ks, partition) = setup_partition();
        let v1 = LineageRecord {
            lineage_id: "lin-1".to_string(),
            active_epoch: Epoch::ZERO,
            active_instance_id: test_instance_id(),
            previous_instance_id: None,
        };
        upsert_lineage_record(&partition, "lin-1", &v1).unwrap();

        let v2 = LineageRecord {
            lineage_id: "lin-1".to_string(),
            active_epoch: Epoch::new(1),
            active_instance_id: test_instance_id_2(),
            previous_instance_id: Some(test_instance_id()),
        };
        upsert_lineage_record(&partition, "lin-1", &v2).unwrap();

        let loaded = get_lineage_record(&partition, "lin-1").unwrap().unwrap();
        assert_eq!(loaded.active_epoch, Epoch::new(1));
        assert_eq!(loaded.active_instance_id, test_instance_id_2());
    }

    #[test]
    fn record_rollover_updates_epoch_and_shifts_instance() {
        let (dir, db, partition) = setup_partition();

        // Seed initial record
        let initial = LineageRecord {
            lineage_id: "lin-1".to_string(),
            active_epoch: Epoch::ZERO,
            active_instance_id: test_instance_id(),
            previous_instance_id: None,
        };
        upsert_lineage_record(&partition, "lin-1", &initial).unwrap();

        // Perform rollover
        record_rollover(&db, "lin-1", Epoch::new(1), test_instance_id_2()).unwrap();

        // Verify updated record
        let loaded = get_lineage_record(&partition, "lin-1").unwrap().unwrap();
        assert_eq!(loaded.active_epoch, Epoch::new(1));
        assert_eq!(loaded.active_instance_id, test_instance_id_2());
        assert_eq!(loaded.previous_instance_id, Some(test_instance_id()));

        drop(dir);
    }

    #[test]
    fn lineage_store_error_display_storage() {
        let err = LineageStoreError::Storage {
            reason: "disk full".to_string(),
        };
        assert_eq!(err.to_string(), "lineage store error: disk full");
    }

    #[test]
    fn lineage_store_error_display_corrupt_value() {
        let err = LineageStoreError::CorruptValue {
            reason: "bad json".to_string(),
        };
        assert_eq!(err.to_string(), "corrupt lineage record: bad json");
    }
}
