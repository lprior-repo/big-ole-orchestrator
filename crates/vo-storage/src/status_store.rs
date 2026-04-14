//! Persistence layer for `RegistrationStatus` using Fjall.
//!
//! Architecture: Data (`StatusStoreError`) → Calc (`encode_status`, `decode_status`)
//! → Actions (`read_registration_status`, `write_registration_status`, `load_all_statuses`).
//!
//! The `workflows` partition stores `WorkflowName` -> JSON-encoded `RegistrationStatus`.

use vo_types::{RegistrationStatus, WorkflowName};

// ---------------------------------------------------------------------------
// Data layer — error enum
// ---------------------------------------------------------------------------

/// Errors from the registration status persistence layer.
#[non_exhaustive]
#[derive(Debug, PartialEq, Eq)]
pub enum StatusStoreError {
    /// Fjall storage operation failed.
    Storage { reason: String },

    /// Stored value could not be decoded into a valid `RegistrationStatus`.
    CorruptValue { reason: String },
}

impl std::fmt::Display for StatusStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Storage { reason } => write!(f, "storage error: {reason}"),
            Self::CorruptValue { reason } => write!(f, "corrupt status value: {reason}"),
        }
    }
}

impl std::error::Error for StatusStoreError {}

// ---------------------------------------------------------------------------
// Calc layer — pure encode/decode
// ---------------------------------------------------------------------------

/// Encode a `RegistrationStatus` to JSON bytes.
///
/// # Errors
/// Returns `StatusStoreError::CorruptValue` if serialization fails (should not happen
/// for a well-defined enum, but we handle it explicitly per zero-panic rule).
pub fn encode_status(status: RegistrationStatus) -> Result<Vec<u8>, StatusStoreError> {
    serde_json::to_vec(&status).map_err(|e| StatusStoreError::CorruptValue {
        reason: e.to_string(),
    })
}

/// Decode JSON bytes into a `RegistrationStatus`.
///
/// # Errors
/// Returns `StatusStoreError::CorruptValue` if the bytes are not valid JSON
/// or do not represent a known `RegistrationStatus` variant.
pub fn decode_status(bytes: &[u8]) -> Result<RegistrationStatus, StatusStoreError> {
    serde_json::from_slice(bytes).map_err(|e| StatusStoreError::CorruptValue {
        reason: e.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Actions layer — Fjall operations
// ---------------------------------------------------------------------------

/// The partition name used for workflow registration statuses.
pub const WORKFLOWS_PARTITION: &str = "workflows";

/// Read the registration status for a single workflow from Fjall.
///
/// # Returns
/// - `Ok(Some(status))` if the workflow has a persisted status
/// - `Ok(None)` if no entry exists (default: Active)
///
/// # Errors
/// Returns `StatusStoreError::Storage` on Fjall failure.
/// Returns `StatusStoreError::CorruptValue` if the stored value is malformed.
pub fn read_registration_status(
    partition: &fjall::Keyspace,
    workflow_name: &WorkflowName,
) -> Result<Option<RegistrationStatus>, StatusStoreError> {
    let key = workflow_name.as_str().as_bytes();
    match partition.get(key) {
        Ok(Some(value_bytes)) => {
            let status = decode_status(&value_bytes)?;
            Ok(Some(status))
        }
        Ok(None) => Ok(None),
        Err(e) => Err(StatusStoreError::Storage {
            reason: e.to_string(),
        }),
    }
}

/// Write a registration status for a workflow to Fjall.
///
/// # Errors
/// Returns `StatusStoreError::Storage` on Fjall failure.
/// Returns `StatusStoreError::CorruptValue` if serialization fails.
pub fn write_registration_status(
    partition: &fjall::Keyspace,
    workflow_name: &WorkflowName,
    status: RegistrationStatus,
) -> Result<(), StatusStoreError> {
    let key = workflow_name.as_str().as_bytes();
    let value = encode_status(status)?;
    partition
        .insert(key, &value)
        .map_err(|e| StatusStoreError::Storage {
            reason: e.to_string(),
        })
}

/// Load all persisted registration statuses from Fjall.
///
/// Returns only non-Active entries (Active is the default and need not be persisted).
///
/// # Errors
/// Returns `StatusStoreError::Storage` on Fjall scan failure.
/// Returns `StatusStoreError::CorruptValue` if any stored key or value is malformed.
pub fn load_all_statuses(
    partition: &fjall::Keyspace,
) -> Result<Vec<(WorkflowName, RegistrationStatus)>, StatusStoreError> {
    partition
        .iter()
        .map(|item| {
            let (key_bytes, value_bytes) =
                item.into_inner().map_err(|e| StatusStoreError::Storage {
                    reason: e.to_string(),
                })?;

            let key_str =
                std::str::from_utf8(&key_bytes).map_err(|e| StatusStoreError::CorruptValue {
                    reason: format!("invalid UTF-8 key: {e}"),
                })?;

            let workflow_name =
                WorkflowName::parse(key_str).map_err(|e| StatusStoreError::CorruptValue {
                    reason: format!("invalid workflow name: {e}"),
                })?;

            let status = decode_status(&value_bytes)?;

            Ok((workflow_name, status))
        })
        .filter_map(
            |result: Result<(WorkflowName, RegistrationStatus), StatusStoreError>| {
                match result {
                    // Only include non-Active entries (Active is default)
                    Ok((_, RegistrationStatus::Active)) => None,
                    Ok(pair) => Some(Ok(pair)),
                    Err(e) => Some(Err(e)),
                }
            },
        )
        .collect()
}

// ---------------------------------------------------------------------------
// Unit tests — Calc layer
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn setup_partition() -> (tempfile::TempDir, fjall::Database, fjall::Keyspace) {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        let partition = db
            .keyspace(WORKFLOWS_PARTITION, fjall::KeyspaceCreateOptions::default)
            .unwrap();
        (dir, db, partition)
    }

    fn workflow_name(value: &str) -> WorkflowName {
        WorkflowName::parse(value).unwrap()
    }

    #[test]
    fn encode_status_returns_json_bytes_for_active() {
        let bytes = encode_status(RegistrationStatus::Active).unwrap();
        assert_eq!(bytes, b"\"Active\"");
    }

    #[test]
    fn encode_status_returns_json_bytes_for_quarantined() {
        let bytes = encode_status(RegistrationStatus::Quarantined).unwrap();
        assert_eq!(bytes, b"\"Quarantined\"");
    }

    #[test]
    fn encode_status_returns_json_bytes_for_deactivated() {
        let bytes = encode_status(RegistrationStatus::Deactivated).unwrap();
        assert_eq!(bytes, b"\"Deactivated\"");
    }

    #[test]
    fn decode_status_returns_active_from_valid_json() {
        let result = decode_status(b"\"Active\"").unwrap();
        assert_eq!(result, RegistrationStatus::Active);
    }

    #[test]
    fn decode_status_returns_quarantined_from_valid_json() {
        let result = decode_status(b"\"Quarantined\"").unwrap();
        assert_eq!(result, RegistrationStatus::Quarantined);
    }

    #[test]
    fn decode_status_returns_deactivated_from_valid_json() {
        let result = decode_status(b"\"Deactivated\"").unwrap();
        assert_eq!(result, RegistrationStatus::Deactivated);
    }

    #[test]
    fn status_store_error_display_returns_exact_message_for_storage() {
        let error = StatusStoreError::Storage {
            reason: "disk offline".to_string(),
        };
        assert_eq!(error.to_string(), "storage error: disk offline");
    }

    #[test]
    fn status_store_error_display_returns_exact_message_for_corrupt_value() {
        let error = StatusStoreError::CorruptValue {
            reason: "bad json".to_string(),
        };
        assert_eq!(error.to_string(), "corrupt status value: bad json");
    }

    #[test]
    fn decode_status_returns_error_for_invalid_json() {
        let result = decode_status(b"not-json");
        match result {
            Err(StatusStoreError::CorruptValue { reason }) => {
                assert!(!reason.is_empty());
            }
            other => panic!("expected CorruptValue, got {other:?}"),
        }
    }

    #[test]
    fn decode_status_returns_error_for_unknown_variant() {
        let result = decode_status(b"\"Unknown\"");
        match result {
            Err(StatusStoreError::CorruptValue { reason }) => {
                assert!(!reason.is_empty());
            }
            other => panic!("expected CorruptValue, got {other:?}"),
        }
    }

    #[test]
    #[allow(clippy::needless_for_each)]
    fn encode_then_decode_is_identity_for_all_variants() {
        let variants = [
            RegistrationStatus::Active,
            RegistrationStatus::Deactivated,
            RegistrationStatus::Quarantined,
        ];
        variants.into_iter().for_each(|status| {
            let bytes = encode_status(status).unwrap();
            let decoded = decode_status(&bytes).unwrap();
            assert_eq!(decoded, status);
        });
    }

    #[test]
    fn read_registration_status_returns_none_when_partition_has_no_entry() {
        let (_dir, _keyspace, partition) = setup_partition();
        let result = read_registration_status(&partition, &workflow_name("new-workflow"));
        assert_eq!(result, Ok(None));
    }

    #[test]
    fn read_registration_status_returns_written_status_for_existing_entry() {
        let (_dir, _keyspace, partition) = setup_partition();
        let wf = workflow_name("existing-workflow");

        let write_result =
            write_registration_status(&partition, &wf, RegistrationStatus::Quarantined);
        assert_eq!(write_result, Ok(()));

        let read_result = read_registration_status(&partition, &wf);
        assert_eq!(read_result, Ok(Some(RegistrationStatus::Quarantined)));
    }

    #[test]
    fn load_all_statuses_returns_only_non_active_entries_in_key_order() {
        let (_dir, _keyspace, partition) = setup_partition();
        let active = workflow_name("wf-active");
        let deactivated = workflow_name("wf-deactivated");
        let quarantined = workflow_name("wf-quarantined");

        assert_eq!(
            write_registration_status(&partition, &quarantined, RegistrationStatus::Quarantined),
            Ok(())
        );
        assert_eq!(
            write_registration_status(&partition, &deactivated, RegistrationStatus::Deactivated),
            Ok(())
        );
        assert_eq!(
            write_registration_status(&partition, &active, RegistrationStatus::Active),
            Ok(())
        );

        let result = load_all_statuses(&partition);
        assert_eq!(
            result,
            Ok(vec![
                (deactivated, RegistrationStatus::Deactivated),
                (quarantined, RegistrationStatus::Quarantined),
            ])
        );
    }

    #[test]
    fn load_all_statuses_returns_corrupt_value_when_key_is_not_utf8() {
        let (_dir, _keyspace, partition) = setup_partition();
        partition.insert([0xFF_u8], b"\"Quarantined\"").unwrap();

        let result = load_all_statuses(&partition);
        assert_eq!(
            result,
            Err(StatusStoreError::CorruptValue {
                reason: "invalid UTF-8 key: invalid utf-8 sequence of 1 bytes from index 0"
                    .to_string(),
            })
        );
    }

    #[test]
    fn load_all_statuses_returns_corrupt_value_when_stored_status_is_invalid_json() {
        let (_dir, _keyspace, partition) = setup_partition();
        partition.insert(b"wf-corrupt", b"not-valid-json").unwrap();

        let result = load_all_statuses(&partition);
        assert_eq!(
            result,
            Err(StatusStoreError::CorruptValue {
                reason: "expected value at line 1 column 1".to_string(),
            })
        );
    }
}
