//! Blob publication types per ADR-040.
//!
//! Architecture: Data layer only — pure types, no I/O, no async.
//!
//! This module defines:
//! - [`BlobRef`]: Immutable reference to a canonical payload blob
//! - [`BlobStatus`]: Lifecycle state of a blob through the publication pipeline
//! - [`OutputRef`]: Discriminated union for step output data

use serde::{Deserialize, Serialize};

/// Maximum bytes allowed for inline output data (routing-critical small payloads).
pub const INLINED_MAX_BYTES: usize = 4096;

// ---------------------------------------------------------------------------
// BlobRef — immutable reference to a canonical payload blob
// ---------------------------------------------------------------------------

/// Immutable reference to a canonical payload blob.
///
/// Invariant: blob_id is a valid ULID, size_bytes > 0, content_hash is valid lowercase hex.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlobRef {
    blob_id: String,
    size_bytes: u64,
    content_hash: String,
}

impl BlobRef {
    /// Construct a new `BlobRef`.
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if blob_id is empty/invalid ULID, size_bytes is zero,
    /// or content_hash is empty/invalid hex.
    pub fn new(
        blob_id: &str,
        size_bytes: u64,
        content_hash: &str,
    ) -> Result<Self, crate::ParseError> {
        // Validate blob_id (ULID)
        if blob_id.is_empty() {
            return Err(crate::ParseError::Empty {
                type_name: "BlobRef.blob_id",
            });
        }
        if blob_id.len() != 26 {
            return Err(crate::ParseError::InvalidFormat {
                type_name: "BlobRef.blob_id",
                reason: format!("expected 26 characters, got {}", blob_id.len()),
            });
        }
        if ulid::Ulid::from_string(blob_id).is_err() {
            return Err(crate::ParseError::InvalidFormat {
                type_name: "BlobRef.blob_id",
                reason: "not a valid ULID".to_string(),
            });
        }

        // Validate size_bytes
        if size_bytes == 0 {
            return Err(crate::ParseError::ZeroValue {
                type_name: "BlobRef.size_bytes",
            });
        }

        // Validate content_hash (lowercase hex)
        if content_hash.is_empty() {
            return Err(crate::ParseError::Empty {
                type_name: "BlobRef.content_hash",
            });
        }
        let invalid =
            crate::types::extract_invalid_chars(content_hash, crate::types::is_lowercase_hex);
        if !invalid.is_empty() {
            return Err(crate::ParseError::InvalidCharacters {
                type_name: "BlobRef.content_hash",
                invalid_chars: invalid,
            });
        }
        if !content_hash.len().is_multiple_of(2) {
            return Err(crate::ParseError::InvalidFormat {
                type_name: "BlobRef.content_hash",
                reason: "hex string has odd length".to_string(),
            });
        }
        if content_hash.len() < 8 {
            return Err(crate::ParseError::InvalidFormat {
                type_name: "BlobRef.content_hash",
                reason: "hex string must be at least 8 characters".to_string(),
            });
        }

        Ok(Self {
            blob_id: blob_id.to_string(),
            size_bytes,
            content_hash: content_hash.to_string(),
        })
    }

    #[must_use]
    pub fn blob_id(&self) -> &str {
        &self.blob_id
    }

    #[must_use]
    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    #[must_use]
    pub fn content_hash(&self) -> &str {
        &self.content_hash
    }
}

// ---------------------------------------------------------------------------
// BlobStatus — lifecycle state of a blob
// ---------------------------------------------------------------------------

/// Lifecycle state of a blob through the publication pipeline.
///
/// State machine:
///   Pending → DurablyStored → Published (terminal)
///   Pending → Failed (terminal)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BlobStatus {
    /// Blob has been accepted but not yet durably written.
    Pending,
    /// Blob has been durably written to storage.
    DurablyStored,
    /// Blob has been published and is referenceable by output_ref.
    Published,
    /// Blob persistence failed.
    Failed,
}

impl BlobStatus {
    /// Returns true if transitioning from `self` to `target` is valid.
    ///
    /// Valid transitions:
    /// - Pending → DurablyStored
    /// - Pending → Failed
    /// - DurablyStored → Published
    #[must_use]
    pub fn can_transition_to(self, target: Self) -> bool {
        matches!(
            (self, target),
            (Self::Pending, Self::DurablyStored)
                | (Self::Pending, Self::Failed)
                | (Self::DurablyStored, Self::Published)
        )
    }

    /// Returns all 4 variants in declared order.
    #[must_use]
    pub const fn all_variants() -> &'static [BlobStatus; 4] {
        &[
            BlobStatus::Pending,
            BlobStatus::DurablyStored,
            BlobStatus::Published,
            BlobStatus::Failed,
        ]
    }
}

// ---------------------------------------------------------------------------
// OutputRef — discriminated union for step output data
// ---------------------------------------------------------------------------

/// Discriminated union for step output data.
///
/// Either small inline routing-critical data or a reference to a canonical blob.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OutputRef {
    /// Small bounded data stored directly in the control plane.
    Inline(Vec<u8>),
    /// Reference to a canonical payload blob.
    BlobRef(BlobRef),
}

impl OutputRef {
    /// Construct an inline `OutputRef`.
    ///
    /// # Errors
    ///
    /// Returns `ParseError::ExceedsMaxLength` if `data.len() > INLINED_MAX_BYTES`.
    pub fn inline(data: Vec<u8>) -> Result<Self, crate::ParseError> {
        if data.len() > INLINED_MAX_BYTES {
            return Err(crate::ParseError::ExceedsMaxLength {
                type_name: "OutputRef.inline",
                max: INLINED_MAX_BYTES,
                actual: data.len(),
            });
        }
        Ok(Self::Inline(data))
    }

    /// Construct a blob-reference `OutputRef`.
    #[must_use]
    pub fn blob_ref(blob: BlobRef) -> Self {
        Self::BlobRef(blob)
    }

    #[must_use]
    pub fn is_inline(&self) -> bool {
        matches!(self, Self::Inline(_))
    }

    #[must_use]
    pub fn is_blob_ref(&self) -> bool {
        matches!(self, Self::BlobRef(_))
    }

    #[must_use]
    pub fn as_inline(&self) -> Option<&[u8]> {
        match self {
            Self::Inline(data) => Some(data),
            Self::BlobRef(_) => None,
        }
    }

    #[must_use]
    pub fn as_blob_ref(&self) -> Option<&BlobRef> {
        match self {
            Self::Inline(_) => None,
            Self::BlobRef(blob) => Some(blob),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests — TDD RED PHASE (compile but fail)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use serde_json::json;

    // =========================================================================
    // BlobRef::new — Happy Path
    // =========================================================================

    #[test]
    fn blobref_constructs_with_all_valid_fields() {
        let blob = BlobRef::new(
            "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
            1024,
            "abcdef0123456789abcdef0123456789",
        );
        let blob = blob.expect("BlobRef should construct with valid fields");
        assert_eq!(blob.blob_id(), "01H5JQX7K3R4T6V8W0X2Y4Z6A8");
        assert_eq!(blob.size_bytes(), 1024);
        assert_eq!(blob.content_hash(), "abcdef0123456789abcdef0123456789");
    }

    #[test]
    fn blobref_constructs_with_minimum_valid_content_hash() {
        let blob = BlobRef::new("01H5JQX7K3R4T6V8W0X2Y4Z6A8", 1, "abcdef01");
        let blob = blob.expect("BlobRef should construct with 8-char hash");
        assert_eq!(blob.content_hash(), "abcdef01");
        assert_eq!(blob.size_bytes(), 1);
    }

    // =========================================================================
    // BlobRef::new — Error Paths
    // =========================================================================

    #[test]
    fn blobref_rejects_empty_blob_id() {
        let result = BlobRef::new("", 1024, "abcdef0123456789abcdef0123456789");
        assert_eq!(
            result,
            Err(crate::ParseError::Empty {
                type_name: "BlobRef.blob_id"
            })
        );
    }

    #[test]
    fn blobref_rejects_invalid_ulid_blob_id() {
        let result = BlobRef::new("not-a-ulid", 1024, "abcdef0123456789abcdef0123456789");
        assert!(matches!(
            result,
            Err(crate::ParseError::InvalidFormat {
                type_name: "BlobRef.blob_id",
                ..
            })
        ));
    }

    #[test]
    fn blobref_rejects_blob_id_with_wrong_length() {
        let result = BlobRef::new("01H5JQX7", 1024, "abcdef0123456789abcdef0123456789");
        assert!(matches!(
            result,
            Err(crate::ParseError::InvalidFormat {
                type_name: "BlobRef.blob_id",
                ..
            })
        ));
    }

    #[test]
    fn blobref_rejects_empty_content_hash() {
        let result = BlobRef::new("01H5JQX7K3R4T6V8W0X2Y4Z6A8", 1024, "");
        assert_eq!(
            result,
            Err(crate::ParseError::Empty {
                type_name: "BlobRef.content_hash"
            })
        );
    }

    #[test]
    fn blobref_rejects_non_hex_content_hash() {
        let result = BlobRef::new("01H5JQX7K3R4T6V8W0X2Y4Z6A8", 1024, "ghijklmnop");
        assert_eq!(
            result,
            Err(crate::ParseError::InvalidCharacters {
                type_name: "BlobRef.content_hash",
                invalid_chars: "ghijklmnop".to_string()
            })
        );
    }

    #[test]
    fn blobref_rejects_odd_length_content_hash() {
        let result = BlobRef::new("01H5JQX7K3R4T6V8W0X2Y4Z6A8", 1024, "abcde");
        assert!(matches!(
            result,
            Err(crate::ParseError::InvalidFormat {
                type_name: "BlobRef.content_hash",
                ..
            })
        ));
    }

    #[test]
    fn blobref_rejects_short_content_hash() {
        let result = BlobRef::new("01H5JQX7K3R4T6V8W0X2Y4Z6A8", 1024, "ab");
        assert!(matches!(
            result,
            Err(crate::ParseError::InvalidFormat {
                type_name: "BlobRef.content_hash",
                ..
            })
        ));
    }

    #[test]
    fn blobref_rejects_zero_size_bytes() {
        let result = BlobRef::new(
            "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
            0,
            "abcdef0123456789abcdef0123456789",
        );
        assert_eq!(
            result,
            Err(crate::ParseError::ZeroValue {
                type_name: "BlobRef.size_bytes"
            })
        );
    }

    // =========================================================================
    // BlobRef — Accessors
    // =========================================================================

    #[test]
    fn blobref_exposes_all_fields_via_accessors() {
        let blob = BlobRef::new(
            "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
            42,
            "abcdef0123456789abcdef0123456789",
        )
        .expect("should construct");
        assert_eq!(blob.blob_id(), "01H5JQX7K3R4T6V8W0X2Y4Z6A8");
        assert_eq!(blob.size_bytes(), 42);
        assert_eq!(blob.content_hash(), "abcdef0123456789abcdef0123456789");
    }

    // =========================================================================
    // BlobRef — Serde Roundtrip
    // =========================================================================

    #[test]
    fn blobref_serde_roundtrips() {
        let blob = BlobRef::new(
            "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
            1024,
            "abcdef0123456789abcdef0123456789",
        )
        .expect("should construct");
        let json_str = serde_json::to_string(&blob).expect("serialize");
        let recovered: BlobRef = serde_json::from_str(&json_str).expect("deserialize");
        assert_eq!(blob, recovered);
    }

    #[test]
    fn blobref_serializes_to_expected_json_structure() {
        let blob = BlobRef::new(
            "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
            1024,
            "abcdef0123456789abcdef0123456789",
        )
        .expect("should construct");
        let json_val = serde_json::to_value(&blob).expect("serialize");
        assert_eq!(json_val["blob_id"], json!("01H5JQX7K3R4T6V8W0X2Y4Z6A8"));
        assert_eq!(json_val["size_bytes"], json!(1024));
        assert_eq!(
            json_val["content_hash"],
            json!("abcdef0123456789abcdef0123456789")
        );
    }

    // =========================================================================
    // BlobStatus — State Machine Transitions
    // =========================================================================

    #[test]
    fn pending_can_transition_to_durably_stored() {
        assert!(BlobStatus::Pending.can_transition_to(BlobStatus::DurablyStored));
    }

    #[test]
    fn pending_can_transition_to_failed() {
        assert!(BlobStatus::Pending.can_transition_to(BlobStatus::Failed));
    }

    #[test]
    fn pending_cannot_skip_to_published() {
        assert!(!BlobStatus::Pending.can_transition_to(BlobStatus::Published));
    }

    #[test]
    fn pending_cannot_transition_to_itself() {
        assert!(!BlobStatus::Pending.can_transition_to(BlobStatus::Pending));
    }

    #[test]
    fn durably_stored_can_transition_to_published() {
        assert!(BlobStatus::DurablyStored.can_transition_to(BlobStatus::Published));
    }

    #[test]
    fn durably_stored_cannot_revert_to_pending() {
        assert!(!BlobStatus::DurablyStored.can_transition_to(BlobStatus::Pending));
    }

    #[test]
    fn durably_stored_cannot_transition_to_itself() {
        assert!(!BlobStatus::DurablyStored.can_transition_to(BlobStatus::DurablyStored));
    }

    #[test]
    fn durably_stored_cannot_transition_to_failed() {
        assert!(!BlobStatus::DurablyStored.can_transition_to(BlobStatus::Failed));
    }

    #[test]
    fn published_is_terminal_state() {
        let variants = BlobStatus::all_variants();
        for &target in variants {
            assert!(
                !BlobStatus::Published.can_transition_to(target),
                "Published should not transition to {:?}",
                target
            );
        }
    }

    #[test]
    fn failed_is_terminal_state() {
        let variants = BlobStatus::all_variants();
        for &target in variants {
            assert!(
                !BlobStatus::Failed.can_transition_to(target),
                "Failed should not transition to {:?}",
                target
            );
        }
    }

    // =========================================================================
    // BlobStatus — all_variants
    // =========================================================================

    #[test]
    fn blob_status_all_variants_returns_four_in_declared_order() {
        let variants = BlobStatus::all_variants();
        assert_eq!(variants.len(), 4);
        assert_eq!(
            variants,
            &[
                BlobStatus::Pending,
                BlobStatus::DurablyStored,
                BlobStatus::Published,
                BlobStatus::Failed,
            ]
        );
    }

    // =========================================================================
    // BlobStatus — Serde Roundtrip
    // =========================================================================

    #[rstest]
    #[case(BlobStatus::Pending)]
    #[case(BlobStatus::DurablyStored)]
    #[case(BlobStatus::Published)]
    #[case(BlobStatus::Failed)]
    fn blob_status_serde_roundtrips(#[case] status: BlobStatus) {
        let json_str = serde_json::to_string(&status).expect("serialize");
        let recovered: BlobStatus = serde_json::from_str(&json_str).expect("deserialize");
        assert_eq!(status, recovered);
    }

    // =========================================================================
    // OutputRef::inline — Happy Path
    // =========================================================================

    #[test]
    fn outputref_inline_constructs_when_within_max() {
        let data = vec![1u8; 100];
        let result = OutputRef::inline(data.clone()).expect("should construct");
        assert!(result.is_inline());
        assert!(!result.is_blob_ref());
        assert_eq!(result.as_inline(), Some(data.as_slice()));
        assert_eq!(result.as_blob_ref(), None);
    }

    #[test]
    fn outputref_inline_accepts_exactly_max_bytes() {
        let data = vec![0u8; INLINED_MAX_BYTES];
        let result = OutputRef::inline(data.clone()).expect("should accept exactly max");
        assert_eq!(result.as_inline(), Some(data.as_slice()));
    }

    #[test]
    fn outputref_inline_accepts_empty_bytes() {
        let result = OutputRef::inline(vec![]).expect("should accept empty");
        assert!(result.is_inline());
        assert_eq!(result.as_inline(), Some(&[][..]));
    }

    // =========================================================================
    // OutputRef::inline — Error Path
    // =========================================================================

    #[test]
    fn outputref_inline_rejects_when_exceeds_max() {
        let data = vec![0u8; INLINED_MAX_BYTES + 1];
        let result = OutputRef::inline(data);
        assert_eq!(
            result,
            Err(crate::ParseError::ExceedsMaxLength {
                type_name: "OutputRef.inline",
                max: INLINED_MAX_BYTES,
                actual: INLINED_MAX_BYTES + 1,
            })
        );
    }

    #[test]
    fn outputref_inline_rejects_huge_data() {
        let data = vec![0u8; 1_000_000];
        let result = OutputRef::inline(data);
        assert_eq!(
            result,
            Err(crate::ParseError::ExceedsMaxLength {
                type_name: "OutputRef.inline",
                max: INLINED_MAX_BYTES,
                actual: 1_000_000,
            })
        );
    }

    // =========================================================================
    // OutputRef::blob_ref — Construction
    // =========================================================================

    #[test]
    fn outputref_blob_ref_constructs_from_valid_blobref() {
        let blob = BlobRef::new(
            "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
            1024,
            "abcdef0123456789abcdef0123456789",
        )
        .expect("should construct");
        let output = OutputRef::blob_ref(blob.clone());
        assert!(!output.is_inline());
        assert!(output.is_blob_ref());
        assert_eq!(output.as_inline(), None);
        assert_eq!(output.as_blob_ref(), Some(&blob));
    }

    // =========================================================================
    // OutputRef — Discriminators
    // =========================================================================

    #[test]
    fn outputref_discriminators_return_correct_values_for_inline() {
        let output = OutputRef::inline(vec![1, 2, 3]).expect("should construct");
        assert!(output.is_inline());
        assert!(!output.is_blob_ref());
        assert_eq!(output.as_inline(), Some(&[1, 2, 3][..]));
        assert_eq!(output.as_blob_ref(), None);
    }

    #[test]
    fn outputref_discriminators_return_correct_values_for_blob_ref() {
        let blob = BlobRef::new(
            "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
            1024,
            "abcdef0123456789abcdef0123456789",
        )
        .expect("should construct");
        let output = OutputRef::blob_ref(blob);
        assert!(!output.is_inline());
        assert!(output.is_blob_ref());
        assert_eq!(output.as_inline(), None);
        assert!(output.as_blob_ref().is_some());
    }

    // =========================================================================
    // OutputRef — Serde Roundtrip
    // =========================================================================

    #[test]
    fn outputref_inline_serde_roundtrips() {
        let output = OutputRef::inline(vec![10, 20, 30]).expect("should construct");
        let json_str = serde_json::to_string(&output).expect("serialize");
        let recovered: OutputRef = serde_json::from_str(&json_str).expect("deserialize");
        assert_eq!(output, recovered);
    }

    #[test]
    fn outputref_blobref_variant_serde_roundtrips() {
        let blob = BlobRef::new(
            "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
            1024,
            "abcdef0123456789abcdef0123456789",
        )
        .expect("should construct");
        let output = OutputRef::blob_ref(blob);
        let json_str = serde_json::to_string(&output).expect("serialize");
        let recovered: OutputRef = serde_json::from_str(&json_str).expect("deserialize");
        assert_eq!(output, recovered);
    }

    // =========================================================================
    // INLINED_MAX_BYTES — Constant
    // =========================================================================

    #[test]
    fn inlined_max_bytes_is_4096() {
        assert_eq!(INLINED_MAX_BYTES, 4096);
    }

    // =========================================================================
    // BlobRef — Equality & Clone
    // =========================================================================

    #[test]
    fn blobref_equality_works_for_same_values() {
        let a = BlobRef::new(
            "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
            100,
            "abcdef0123456789abcdef0123456789",
        )
        .expect("should construct");
        let b = BlobRef::new(
            "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
            100,
            "abcdef0123456789abcdef0123456789",
        )
        .expect("should construct");
        assert_eq!(a, b);
    }

    #[test]
    fn blobref_inequality_works_for_different_values() {
        let a = BlobRef::new(
            "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
            100,
            "abcdef0123456789abcdef0123456789",
        )
        .expect("should construct");
        let b = BlobRef::new(
            "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
            200,
            "abcdef0123456789abcdef0123456789",
        )
        .expect("should construct");
        assert_ne!(a, b);
    }

    #[test]
    fn blobref_clone_produces_equal_value() {
        let blob = BlobRef::new(
            "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
            100,
            "abcdef0123456789abcdef0123456789",
        )
        .expect("should construct");
        assert_eq!(blob.clone(), blob);
    }

    // =========================================================================
    // BlobStatus — Equality
    // =========================================================================

    #[test]
    fn blob_status_equality_works() {
        assert_eq!(BlobStatus::Pending, BlobStatus::Pending);
        assert_eq!(BlobStatus::Published, BlobStatus::Published);
        assert_ne!(BlobStatus::Pending, BlobStatus::Published);
    }

    // =========================================================================
    // OutputRef — Equality & Clone
    // =========================================================================

    #[test]
    fn outputref_equality_works_for_inline() {
        let a = OutputRef::inline(vec![1, 2]).expect("should construct");
        let b = OutputRef::inline(vec![1, 2]).expect("should construct");
        assert_eq!(a, b);
    }

    #[test]
    fn outputref_inequality_works_for_different_inline_data() {
        let a = OutputRef::inline(vec![1, 2]).expect("should construct");
        let b = OutputRef::inline(vec![3, 4]).expect("should construct");
        assert_ne!(a, b);
    }
}
