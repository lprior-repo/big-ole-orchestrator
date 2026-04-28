//! Blob metadata record and status transitions.

use serde::{Deserialize, Serialize};
use vo_types::BlobStatus;

use super::error::BlobStoreError;
use super::types::ContentAddress;

/// Persisted blob metadata record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobRecord {
    content_addr: ContentAddress,
    size_bytes: u64,
    reference_count: u64,
    created_at_ms: u64,
    expires_at_ms: Option<u64>,
    status: BlobStatus,
}

impl BlobRecord {
    /// Construct a new `BlobRecord` with `Pending` status.
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::InvalidArgument` if `reference_count` is zero
    /// or `created_at_ms` is zero.
    pub fn new(
        content_addr: ContentAddress,
        size_bytes: u64,
        reference_count: u64,
        created_at_ms: u64,
        expires_at_ms: Option<u64>,
    ) -> Result<Self, BlobStoreError> {
        if reference_count == 0 {
            return Err(BlobStoreError::InvalidArgument {
                reason: "reference_count must be non-zero".to_string(),
            });
        }
        if created_at_ms == 0 {
            return Err(BlobStoreError::InvalidArgument {
                reason: "created_at_ms must be non-zero".to_string(),
            });
        }
        Ok(Self {
            content_addr,
            size_bytes,
            reference_count,
            created_at_ms,
            expires_at_ms,
            status: BlobStatus::Pending,
        })
    }

    /// Construct a new `BlobRecord` with explicit status.
    #[must_use]
    pub const fn with_status(
        content_addr: ContentAddress,
        size_bytes: u64,
        reference_count: u64,
        created_at_ms: u64,
        expires_at_ms: Option<u64>,
        status: BlobStatus,
    ) -> Self {
        Self {
            content_addr,
            size_bytes,
            reference_count,
            created_at_ms,
            expires_at_ms,
            status,
        }
    }

    #[must_use]
    pub const fn content_addr(&self) -> &ContentAddress {
        &self.content_addr
    }

    #[must_use]
    pub const fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    #[must_use]
    pub const fn reference_count(&self) -> u64 {
        self.reference_count
    }

    #[must_use]
    pub const fn created_at_ms(&self) -> u64 {
        self.created_at_ms
    }

    #[must_use]
    pub const fn expires_at_ms(&self) -> Option<u64> {
        self.expires_at_ms
    }

    #[must_use]
    pub const fn status(&self) -> BlobStatus {
        self.status
    }

    /// Check if this record has expired given the current timestamp.
    #[must_use]
    pub const fn is_expired(&self, now_ms: u64) -> bool {
        match self.expires_at_ms {
            Some(expires) => now_ms >= expires,
            None => false,
        }
    }

    /// Check if this record is eligible for garbage collection.
    /// A record is GC-eligible when it has expired AND has no references.
    #[must_use]
    pub const fn is_gc_eligible(&self, now_ms: u64) -> bool {
        self.reference_count == 0 && self.is_expired(now_ms)
    }

    /// Increment reference count, saturating at `u64::MAX`.
    #[must_use]
    pub const fn increment_ref_count(&self) -> u64 {
        self.reference_count.saturating_add(1)
    }

    /// Decrement reference count, saturating at zero.
    #[must_use]
    pub const fn decrement_ref_count(&self) -> u64 {
        self.reference_count.saturating_sub(1)
    }

    /// Check if transitioning to the target status is valid per ADR-040.
    ///
    /// Valid transitions:
    /// - Pending → `DurablyStored`
    /// - Pending → Failed
    /// - `DurablyStored` → Published
    #[must_use]
    pub fn can_transition_to(&self, target: BlobStatus) -> bool {
        self.status.can_transition_to(target)
    }
}
