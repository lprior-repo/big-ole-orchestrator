//! Content-Addressed Storage Port
//!
//! Defines the interface for distributed content-addressed blob storage.
//! Implementors must be Send + Sync.

use async_trait::async_trait;
use vo_storage::blob_store::{BlobRecord, BlobStoreError, ContentAddress, PackFileId};

#[async_trait]
pub trait ContentAddressedStorage: Send + Sync {
    async fn store(&self, data: &[u8]) -> Result<ContentAddress, ContentAddressedStorageError>;

    async fn store_streaming<R>(&self, reader: R) -> Result<ContentAddress, ContentAddressedStorageError>
    where
        R: tokio::io::AsyncRead + Send + Unpin + 'static;

    async fn retrieve(&self, addr: &ContentAddress) -> Result<Vec<u8>, ContentAddressedStorageError>;

    async fn retrieve_streaming<W>(
        &self,
        addr: &ContentAddress,
        writer: W,
    ) -> Result<(), ContentAddressedStorageError>
    where
        W: tokio::io::AsyncWrite + Send + Unpin + 'static;

    async fn contains(&self, addr: &ContentAddress) -> Result<bool, ContentAddressedStorageError>;

    async fn increment_ref_count(
        &self,
        addr: &ContentAddress,
    ) -> Result<u64, ContentAddressedStorageError>;

    async fn decrement_ref_count(
        &self,
        addr: &ContentAddress,
    ) -> Result<u64, ContentAddressedStorageError>;

    async fn get_metadata(
        &self,
        addr: &ContentAddress,
    ) -> Result<BlobRecord, ContentAddressedStorageError>;

    async fn list_gc_candidates(
        &self,
        now_ms: u64,
    ) -> Result<Vec<ContentAddress>, ContentAddressedStorageError>;

    async fn run_gc(&self, now_ms: u64) -> Result<u64, ContentAddressedStorageError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentAddressedStorageError {
    ContentNotFound(String),
    PackFileNotFound(String),
    DuplicateContent(String),
    CorruptPackIndex(String),
    CorruptPackFile { pack_file_id: String, reason: String },
    ChecksumMismatch {
        content_addr: String,
        expected: String,
        actual: String,
    },
    SerializationFailed(String),
    DeserializationFailed(String),
    Storage(String),
    InvalidArgument(String),
    GcCycleInProgress,
    PackFileFull { pack_file_id: String, max_size_bytes: u64 },
}

impl ContentAddressedStorageError {
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::Storage(_)
                | Self::DuplicateContent(_)
                | Self::GcCycleInProgress
                | Self::PackFileFull { .. }
        )
    }

    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            Self::CorruptPackIndex(_)
                | Self::CorruptPackFile { .. }
                | Self::ChecksumMismatch { .. }
                | Self::InvalidArgument(_)
        )
    }
}

impl std::fmt::Display for ContentAddressedStorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ContentNotFound(addr) => write!(f, "Content not found: {}", addr),
            Self::PackFileNotFound(id) => write!(f, "Pack file not found: {}", id),
            Self::DuplicateContent(addr) => write!(f, "Duplicate content: {}", addr),
            Self::CorruptPackIndex(s) => write!(f, "Corrupt pack index: {}", s),
            Self::CorruptPackFile {
                pack_file_id,
                reason,
            } => {
                write!(f, "Corrupt pack file {}: {}", pack_file_id, reason)
            }
            Self::ChecksumMismatch {
                content_addr,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "Checksum mismatch for {}: expected {}, got {}",
                    content_addr, expected, actual
                )
            }
            Self::SerializationFailed(s) => write!(f, "Serialization failed: {}", s),
            Self::DeserializationFailed(s) => write!(f, "Deserialization failed: {}", s),
            Self::Storage(s) => write!(f, "Storage error: {}", s),
            Self::InvalidArgument(s) => write!(f, "Invalid argument: {}", s),
            Self::GcCycleInProgress => write!(f, "GC cycle already in progress"),
            Self::PackFileFull {
                pack_file_id,
                max_size_bytes,
            } => {
                write!(f, "Pack file {} full (max {} bytes)", pack_file_id, max_size_bytes)
            }
        }
    }
}

impl std::error::Error for ContentAddressedStorageError {}

impl From<BlobStoreError> for ContentAddressedStorageError {
    fn from(err: BlobStoreError) -> Self {
        match err {
            BlobStoreError::ContentNotFound { content_addr } => {
                Self::ContentNotFound(content_addr)
            }
            BlobStoreError::PackFileNotFound { pack_file_id } => {
                Self::PackFileNotFound(pack_file_id)
            }
            BlobStoreError::DuplicateContent { content_addr } => {
                Self::DuplicateContent(content_addr)
            }
            BlobStoreError::CorruptPackIndex { reason } => Self::CorruptPackIndex(reason),
            BlobStoreError::CorruptPackFile {
                pack_file_id,
                reason,
            } => Self::CorruptPackFile {
                pack_file_id,
                reason,
            },
            BlobStoreError::ChecksumMismatch {
                content_addr,
                expected,
                actual,
            } => Self::ChecksumMismatch {
                content_addr,
                expected,
                actual,
            },
            BlobStoreError::SerializationFailed { reason } => {
                Self::SerializationFailed(reason)
            }
            BlobStoreError::DeserializationFailed { reason } => {
                Self::DeserializationFailed(reason)
            }
            BlobStoreError::Storage { reason } => Self::Storage(reason),
            BlobStoreError::InvalidArgument { reason } => Self::InvalidArgument(reason),
            BlobStoreError::GcCycleInProgress => Self::GcCycleInProgress,
            BlobStoreError::PackFileFull {
                pack_file_id,
                max_size_bytes,
            } => Self::PackFileFull {
                pack_file_id,
                max_size_bytes,
            },
            _ => Self::Storage(format!("Unknown BlobStoreError variant: {:?}", err)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_addressed_storage_error_is_transient() {
        assert!(ContentAddressedStorageError::Storage("test".to_string()).is_transient());
        assert!(ContentAddressedStorageError::DuplicateContent(
            "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08".to_string()
        )
        .is_transient());
        assert!(!ContentAddressedStorageError::InvalidArgument("test".to_string()).is_transient());
    }

    #[test]
    fn content_addressed_storage_error_is_fatal() {
        assert!(ContentAddressedStorageError::CorruptPackIndex("test".to_string()).is_fatal());
        assert!(ContentAddressedStorageError::InvalidArgument("test".to_string()).is_fatal());
        assert!(!ContentAddressedStorageError::Storage("test".to_string()).is_fatal());
    }
}
