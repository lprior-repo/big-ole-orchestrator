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
    partition: &fjall::PartitionHandle,
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
    partition: &fjall::PartitionHandle,
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
    partition: &fjall::PartitionHandle,
) -> Result<Vec<(WorkflowName, RegistrationStatus)>, StatusStoreError> {
    partition
        .iter()
        .map(|item| {
            let (key_bytes, value_bytes) = item.map_err(|e| StatusStoreError::Storage {
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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

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
    fn decode_status_returns_error_for_invalid_json() {
        let result = decode_status(b"not-json");
        assert!(result.is_err());
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
        assert!(result.is_err());
    }

    #[test]
    fn encode_then_decode_is_identity_for_all_variants() {
        let variants = [
            RegistrationStatus::Active,
            RegistrationStatus::Deactivated,
            RegistrationStatus::Quarantined,
        ];
        variants.iter().for_each(|&status| {
            let bytes = encode_status(status).unwrap();
            let decoded = decode_status(&bytes).unwrap();
            assert_eq!(decoded, status);
        });
    }
}
