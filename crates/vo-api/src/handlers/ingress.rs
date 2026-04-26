//! Ingress admission handler -- ADR-028 exactly-once ingress deduplication.
//!
//! Provides the `admit_ingress` function that performs atomic check-and-insert
//! against the `DedupeStore` before allowing workflow start or signal delivery.
//! This module also exposes `admit_signal` for signal/approval deduplication.

#[cfg(test)]
#[path = "ingress_tests.rs"]
mod ingress_tests;

use vo_storage::dedupe_partition::{AdmissionResult, DedupeStore};
use vo_types::{DedupeKey, InstanceId};

/// Default retention window for dedupe records: 1 hour in milliseconds.
pub const DEFAULT_DEDUPE_TTL_MS: u64 = 3_600_000;

/// Result of an ingress admission check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngressAdmission {
    /// The request is new and has been admitted.
    Admitted,
    /// The request is a duplicate of a previously admitted request.
    Duplicate { existing_instance_id: String },
}

/// Error from ingress admission.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IngressAdmissionError {
    #[error("dedupe storage error: {reason}")]
    Storage { reason: String },
    #[error("invalid dedupe key: {reason}")]
    InvalidDedupeKey { reason: String },
}

impl From<vo_storage::dedupe_partition::DedupeStoreError> for IngressAdmissionError {
    fn from(e: vo_storage::dedupe_partition::DedupeStoreError) -> Self {
        match e {
            vo_storage::dedupe_partition::DedupeStoreError::Storage { reason } => {
                IngressAdmissionError::Storage { reason }
            }
            vo_storage::dedupe_partition::DedupeStoreError::Codec { reason } => {
                IngressAdmissionError::Storage { reason }
            }
            vo_storage::dedupe_partition::DedupeStoreError::InvalidArgument => {
                IngressAdmissionError::InvalidDedupeKey {
                    reason: "invalid argument".to_string(),
                }
            }
            // non_exhaustive: catch any future variants
            _ => IngressAdmissionError::Storage {
                reason: e.to_string(),
            },
        }
    }
}

/// Atomically check dedupe and admit a workflow start request.
///
/// This function:
/// 1. Parses the raw dedupe key string into a validated `DedupeKey`.
/// 2. Calls `DedupeStore::check_and_insert` atomically.
/// 3. Returns `Admitted` if this is the first occurrence, or
///    `Duplicate` with the existing instance ID if seen before.
///
/// # Errors
///
/// Returns `IngressAdmissionError::InvalidDedupeKey` if the key is empty or too long.
/// Returns `IngressAdmissionError::Storage` if the underlying storage fails.
pub fn admit_ingress(
    store: &dyn DedupeStore,
    dedupe_key_str: &str,
    instance_id: &InstanceId,
    ttl_ms: u64,
) -> Result<IngressAdmission, IngressAdmissionError> {
    let dedupe_key = DedupeKey::parse(dedupe_key_str).map_err(|e| {
        IngressAdmissionError::InvalidDedupeKey {
            reason: e.to_string(),
        }
    })?;

    let effective_ttl = if ttl_ms == 0 {
        DEFAULT_DEDUPE_TTL_MS
    } else {
        ttl_ms
    };

    let result = store
        .check_and_insert(&dedupe_key, instance_id, effective_ttl)
        .map_err(IngressAdmissionError::from)?;

    match result {
        AdmissionResult::Admitted => Ok(IngressAdmission::Admitted),
        AdmissionResult::Duplicate { instance_id: existing } => {
            Ok(IngressAdmission::Duplicate {
                existing_instance_id: existing,
            })
        }
    }
}

/// Atomically check dedupe and admit a signal/approval delivery request.
///
/// Signals and approvals use the same dedupe mechanism but with a
/// composite key combining the target instance and signal name to
/// ensure per-instance deduplication.
///
/// # Errors
///
/// Same error conditions as `admit_ingress`.
pub fn admit_signal(
    store: &dyn DedupeStore,
    instance_id: &InstanceId,
    signal_name: &str,
    dedupe_key_str: &str,
    ttl_ms: u64,
) -> Result<IngressAdmission, IngressAdmissionError> {
    // Create a composite dedupe key: sig:instance_id:signal_name:raw_key
    // This ensures per-instance, per-signal-type deduplication.
    let composite_key = format!("sig:{}:{signal_name}:{dedupe_key_str}", instance_id.as_str());
    admit_ingress(store, &composite_key, instance_id, ttl_ms)
}
