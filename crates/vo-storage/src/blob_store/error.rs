//! Errors from content-addressed blob store operations.

/// Errors from content-addressed blob store operations.
#[non_exhaustive]
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum BlobStoreError {
    #[error("content not found: {content_addr}")]
    ContentNotFound { content_addr: String },
    #[error("pack file not found: {pack_file_id}")]
    PackFileNotFound { pack_file_id: String },
    #[error("duplicate content: {content_addr}")]
    DuplicateContent { content_addr: String },
    #[error("corrupt pack index: {reason}")]
    CorruptPackIndex { reason: String },
    #[error("corrupt pack file {pack_file_id}: {reason}")]
    CorruptPackFile {
        pack_file_id: String,
        reason: String,
    },
    #[error("checksum mismatch for {content_addr}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        content_addr: String,
        expected: String,
        actual: String,
    },
    #[error("serialization failed: {reason}")]
    SerializationFailed { reason: String },
    #[error("deserialization failed: {reason}")]
    DeserializationFailed { reason: String },
    #[error("storage error: {reason}")]
    Storage { reason: String },
    #[error("invalid argument: {reason}")]
    InvalidArgument { reason: String },
    #[error("GC cycle already in progress")]
    GcCycleInProgress,
    #[error("pack file {pack_file_id} full (max {max_size_bytes} bytes)")]
    PackFileFull {
        pack_file_id: String,
        max_size_bytes: u64,
    },
    #[error("invalid publication status for {content_addr}: current={current_status}, attempted={attempted_operation}")]
    InvalidPublicationStatus {
        content_addr: String,
        current_status: String,
        attempted_operation: String,
    },
    #[error("blob {content_addr} is not durably stored, cannot publish")]
    NotDurablyStored { content_addr: String },
}

impl BlobStoreError {
    #[must_use]
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::Storage { .. }
                | Self::DuplicateContent { .. }
                | Self::GcCycleInProgress
                | Self::PackFileFull { .. }
        )
    }

    #[must_use]
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            Self::CorruptPackIndex { .. }
                | Self::CorruptPackFile { .. }
                | Self::ChecksumMismatch { .. }
                | Self::InvalidArgument { .. }
        )
    }
}
