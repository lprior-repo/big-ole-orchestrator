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
// OutputPolicy — whether an output is required for replay or optional for UX
// ---------------------------------------------------------------------------

/// Policy determining whether a step output blob is required for replay.
///
/// Per ADR-040 §3 "Failure Semantics":
/// - Required outputs: replay depends on them, blob failure blocks step completion
/// - Optional outputs: only needed for operator UX, blob failure allows completion
///   with only routing_projection (inline data)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OutputPolicy {
    /// Output is required for exact-once replay.
    /// Blob failure prevents step completion (retry or fail per policy).
    Required,
    /// Output is only needed for operator UX, not required for replay.
    /// Blob failure allows step completion with only inline routing data.
    Optional,
}

impl OutputPolicy {
    /// Returns true if this policy allows step completion when blob fails.
    ///
    /// Per ADR-040: Optional outputs permit completion with routing_projection
    /// only when blob persistence fails.
    #[must_use]
    pub fn permits_completion_on_blob_failure(self) -> bool {
        matches!(self, Self::Optional)
    }

    /// Returns true if replay requires this output to be durable.
    #[must_use]
    pub fn is_required_for_replay(self) -> bool {
        matches!(self, Self::Required)
    }
}

// ---------------------------------------------------------------------------
// BlobFailureAction — result of applying failure rules
// ---------------------------------------------------------------------------

/// Result of applying optional-output failure rules.
///
/// Per ADR-040 §3, when blob persistence fails:
/// - If output is Required: step stays incomplete (retry or fail)
/// - If output is Optional: step may complete with routing_projection only
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BlobFailureAction {
    /// Step must stay incomplete. Blob failure is blocking.
    /// The step may be retried or failed according to retry policy.
    BlockStep,
    /// Step may complete with only inline routing data (no output_ref).
    /// The blob failure is non-blocking because output is optional.
    CompleteWithInline,
}

impl OutputPolicy {
    /// Determine the failure action when a blob fails to persist.
    ///
    /// # Arguments
    ///
    /// * `blob_status` - The final status of the blob (typically `Failed`)
    ///
    /// # Returns
    ///
    /// * `BlobFailureAction::BlockStep` if blob is Required and failed
    /// * `BlobFailureAction::CompleteWithInline` if blob is Optional and failed
    ///
    /// # Logic (per ADR-040 §3)
    ///
    /// - If blob persistence fails before publication and output is Required,
    ///   the step stays incomplete and may be retried or failed.
    /// - If blob is Optional for operator UX but not replay, the Engine may
    ///   complete the step with only `routing_projection` and no `output_ref`.
    #[must_use]
    pub fn blob_failure_action(self, blob_status: BlobStatus) -> BlobFailureAction {
        if blob_status == BlobStatus::Failed && self.permits_completion_on_blob_failure() {
            BlobFailureAction::CompleteWithInline
        } else {
            BlobFailureAction::BlockStep
        }
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

    /// Classify output data as inline or blob-ref based on size.
    ///
    /// Small data (≤ `INLINED_MAX_BYTES`) is stored inline.
    /// Large data exceeds the inline limit and cannot be classified without
    /// a `BlobRef` — callers should construct `BlobRef` externally and use
    /// [`OutputRef::blob_ref`] instead.
    ///
    /// # Errors
    ///
    /// Returns `ParseError::ExceedsMaxLength` if `data.len() > INLINED_MAX_BYTES`.
    pub fn classify(data: Vec<u8>) -> Result<Self, crate::ParseError> {
        Self::inline(data)
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
