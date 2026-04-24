//! Pure validation of inline payload sizes against blob routing rules.
//!
//! # Architecture
//!
//! This module provides the admission-layer size gate that prevents
//! oversized payloads from entering the control plane as inline data.
//! Per ADR-040, payloads exceeding `INLINED_MAX_BYTES` must be
//! externalized through the blob publication pipeline.
//!
//! # Invariants
//!
//! - Any payload marked as valid inline is strictly ≤ `INLINED_MAX_BYTES`.
//! - The function is pure: no I/O, no side effects, no panics.

use vo_types::INLINED_MAX_BYTES;

/// Error returned when an inline payload exceeds the size limit.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("inline payload too large: {actual_size} bytes exceeds limit of {max_size} bytes")]
pub struct PayloadTooLarge {
    pub actual_size: usize,
    pub max_size: usize,
}

/// Validate that a payload qualifies for inline storage.
///
/// Returns `Ok(())` when `data.len() <= INLINED_MAX_BYTES`,
/// or `Err(PayloadTooLarge)` when the payload must be externalized
/// as a blob.
///
/// # Preconditions
///
/// - `data` is a concrete byte slice (no lazy evaluation).
///
/// # Postconditions
///
/// - `Ok(())` implies the payload is safe for inline control-plane storage.
/// - `Err(PayloadTooLarge)` carries the exact sizes for diagnostics.
///
/// # Errors
///
/// Returns `PayloadTooLarge` with `actual_size` and `max_size` fields
/// when `data.len() > INLINED_MAX_BYTES`.
pub fn validate_inline_size(data: &[u8]) -> Result<(), PayloadTooLarge> {
    if data.len() > INLINED_MAX_BYTES {
        return Err(PayloadTooLarge {
            actual_size: data.len(),
            max_size: INLINED_MAX_BYTES,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_payload_passes_validation() {
        assert_eq!(validate_inline_size(&[]), Ok(()));
    }

    #[test]
    fn small_payload_passes_validation() {
        let data = vec![0u8; 256];
        assert_eq!(validate_inline_size(&data), Ok(()));
    }

    #[test]
    fn payload_at_exact_threshold_passes() {
        let data = vec![0u8; INLINED_MAX_BYTES];
        assert_eq!(validate_inline_size(&data), Ok(()));
    }

    #[test]
    fn payload_one_byte_over_threshold_fails() {
        let data = vec![0u8; INLINED_MAX_BYTES + 1];
        let result = validate_inline_size(&data);
        assert_eq!(
            result,
            Err(PayloadTooLarge {
                actual_size: INLINED_MAX_BYTES + 1,
                max_size: INLINED_MAX_BYTES,
            })
        );
    }

    #[test]
    fn large_payload_fails_with_correct_sizes() {
        let data = vec![0u8; 1_000_000];
        let result = validate_inline_size(&data);
        assert_eq!(
            result,
            Err(PayloadTooLarge {
                actual_size: 1_000_000,
                max_size: INLINED_MAX_BYTES,
            })
        );
    }

    #[test]
    fn error_display_includes_sizes() {
        let err = PayloadTooLarge {
            actual_size: 5000,
            max_size: INLINED_MAX_BYTES,
        };
        let msg = err.to_string();
        assert!(msg.contains("5000"));
        assert!(msg.contains(&INLINED_MAX_BYTES.to_string()));
    }

    #[test]
    fn error_is_clone_and_eq() {
        let err = PayloadTooLarge {
            actual_size: 5000,
            max_size: INLINED_MAX_BYTES,
        };
        assert_eq!(err.clone(), err);
    }

    #[test]
    fn threshold_is_4096() {
        assert_eq!(INLINED_MAX_BYTES, 4096);
    }

    #[test]
    fn payload_just_under_threshold_passes() {
        let data = vec![0u8; INLINED_MAX_BYTES - 1];
        assert_eq!(validate_inline_size(&data), Ok(()));
    }
}
