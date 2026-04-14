//! Content-addressed blob storage with SHA-256 dedup, pack files, and lazy GC.
//!
//! Architecture: Data → Calc → Actions
//!
//! ## Data Layer
//!
//! - [`ContentAddress`]: SHA-256 content hash (64 lowercase hex chars)
//! - [`PackFileId`]: Unique identifier for a pack file
//! - [`PackIndexEntry`]: Maps content address to pack file location
//! - [`BlobRecord`]: Persisted blob metadata
//! - [`BlobStoreError`]: Error taxonomy
//!
//! ## Calc Layer
//!
//! - [`encode_content_address`], [`decode_content_address`]: Content address encoding
//! - [`encode_pack_index_entry`], [`decode_pack_index_entry`]: Pack index encoding
//! - [`validate_content_address`]: Validates SHA-256 hex format
//!
//! ## Actions Layer
//!
//! - [`BlobStore`] trait: Storage interface for content-addressed blobs
//!
//! ## Invariants
//!
//! 1. Content address is always a valid 64-char lowercase hex SHA-256 hash
//! 2. Pack index entry uniquely maps content address → pack file + offset
//! 3. Blob record is immutable once written (append-only pack files)
//! 4. GC only collects blobs with zero reference count and expired TTL
//! 5. Streaming upload/download never buffers full blob in memory
//!
//! ## Error Taxonomy
//!
//! [`BlobStoreError`] variants:
//! - `ContentNotFound`: No blob exists for the given content address
//! - `PackFileNotFound`: Referenced pack file does not exist
//! - `DuplicateContent`: Content already exists (dedup violation on strict insert)
//! - `CorruptPackIndex`: Pack index entry is malformed
//! - `CorruptPackFile`: Pack file data does not match content address
//! - `ChecksumMismatch`: Computed SHA-256 does not match declared content address
//! - `SerializationFailed`: Blob metadata serialization failed
//! - `DeserializationFailed`: Blob metadata deserialization failed
//! - `Storage`: Underlying storage operation failed
//! - `InvalidArgument`: Invalid input argument
//! - `GcCycleInProgress`: GC cycle already running (prevents concurrent GC)
//! - `PackFileFull`: Pack file has reached maximum size (forces new pack)

use std::fmt;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use vo_types::BlobStatus;

// ---------------------------------------------------------------------------
// Data Layer — ContentAddress
// ---------------------------------------------------------------------------

/// Content address based on SHA-256 hash (64 lowercase hex characters).
///
/// # Invariant
///
/// `content_addr` is always exactly 64 characters of lowercase hex (0-9, a-f),
/// representing a full SHA-256 digest.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[expect(clippy::unsafe_derive_deserialize)]
#[derive(Deserialize)]
pub struct ContentAddress(String);

impl ContentAddress {
    const LENGTH: usize = 64;

    /// Construct a `ContentAddress` from a hex string.
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::InvalidArgument` if the string is not exactly
    /// 64 lowercase hex characters.
    pub fn new(addr: impl AsRef<str>) -> Result<Self, BlobStoreError> {
        let s = addr.as_ref();
        if s.len() != Self::LENGTH {
            return Err(BlobStoreError::InvalidArgument {
                reason: format!(
                    "content address must be {} chars, got {}",
                    Self::LENGTH,
                    s.len()
                ),
            });
        }
        if !s
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        {
            return Err(BlobStoreError::InvalidArgument {
                reason: "content address must be lowercase hex".to_string(),
            });
        }
        Ok(Self(s.to_string()))
    }

    /// Construct a `ContentAddress` from raw SHA-256 bytes.
    #[must_use]
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self(bytes.iter().fold(String::with_capacity(64), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        }))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn as_bytes(&self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        for (i, chunk) in self.0.as_bytes().chunks(2).enumerate() {
            let hex_str = unsafe { std::str::from_utf8_unchecked(chunk) };
            bytes[i] = unsafe { u8::from_str_radix(hex_str, 16).unwrap_unchecked() };
        }
        bytes
    }
}

impl fmt::Display for ContentAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// Data Layer — PackFileId
// ---------------------------------------------------------------------------

/// Unique identifier for a pack file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PackFileId(String);

impl PackFileId {
    /// Construct a new `PackFileId`.
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::InvalidArgument` if the string is empty.
    pub fn new(id: impl AsRef<str>) -> Result<Self, BlobStoreError> {
        let s = id.as_ref();
        if s.is_empty() {
            return Err(BlobStoreError::InvalidArgument {
                reason: "pack file ID cannot be empty".to_string(),
            });
        }
        Ok(Self(s.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PackFileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// Data Layer — PackIndexEntry
// ---------------------------------------------------------------------------

/// Location of a blob within a pack file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackIndexEntry {
    content_addr: ContentAddress,
    pack_file_id: PackFileId,
    offset_bytes: u64,
    size_bytes: u64,
}

impl PackIndexEntry {
    /// Construct a new `PackIndexEntry`.
    #[must_use]
    pub const fn new(
        content_addr: ContentAddress,
        pack_file_id: PackFileId,
        offset_bytes: u64,
        size_bytes: u64,
    ) -> Self {
        Self {
            content_addr,
            pack_file_id,
            offset_bytes,
            size_bytes,
        }
    }

    #[must_use]
    pub const fn content_addr(&self) -> &ContentAddress {
        &self.content_addr
    }

    #[must_use]
    pub const fn pack_file_id(&self) -> &PackFileId {
        &self.pack_file_id
    }

    #[must_use]
    pub const fn offset_bytes(&self) -> u64 {
        self.offset_bytes
    }

    #[must_use]
    pub const fn size_bytes(&self) -> u64 {
        self.size_bytes
    }
}

// ---------------------------------------------------------------------------
// Data Layer — BlobRecord
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Data Layer — BlobStoreError
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Calc Layer — Content Address Encoding
// ---------------------------------------------------------------------------

/// Encode a `ContentAddress` as UTF-8 bytes for use as a storage key.
#[must_use]
pub fn encode_content_address(addr: &ContentAddress) -> Vec<u8> {
    addr.as_str().as_bytes().to_vec()
}

/// Decode UTF-8 bytes into a `ContentAddress`.
///
/// # Errors
///
/// Returns `BlobStoreError::CorruptPackIndex` if bytes are not valid UTF-8
/// or if the resulting string is not a valid content address.
pub fn decode_content_address(bytes: &[u8]) -> Result<ContentAddress, BlobStoreError> {
    let s = std::str::from_utf8(bytes).map_err(|e| BlobStoreError::CorruptPackIndex {
        reason: format!("invalid UTF-8: {e}"),
    })?;
    ContentAddress::new(s).map_err(|e| BlobStoreError::CorruptPackIndex {
        reason: e.to_string(),
    })
}

/// Validate that a string is a valid SHA-256 content address (64 lowercase hex chars).
///
/// # Errors
///
/// Returns `BlobStoreError::InvalidArgument` if the string is not a valid content address.
pub fn validate_content_address(addr: &str) -> Result<(), BlobStoreError> {
    ContentAddress::new(addr).map(|_| ())
}

// ---------------------------------------------------------------------------
// Calc Layer — Pack Index Entry Encoding
// ---------------------------------------------------------------------------

/// Encode a `PackIndexEntry` to JSON bytes for storage.
///
/// # Errors
///
/// Returns `BlobStoreError::SerializationFailed` if the entry cannot be serialized to JSON.
pub fn encode_pack_index_entry(entry: &PackIndexEntry) -> Result<Vec<u8>, BlobStoreError> {
    serde_json::to_vec(entry).map_err(|e| BlobStoreError::SerializationFailed {
        reason: e.to_string(),
    })
}

/// Decode JSON bytes into a `PackIndexEntry`.
///
/// # Errors
///
/// Returns `BlobStoreError::CorruptPackIndex` if the bytes are not valid JSON
/// or do not represent a valid `PackIndexEntry`.
pub fn decode_pack_index_entry(bytes: &[u8]) -> Result<PackIndexEntry, BlobStoreError> {
    serde_json::from_slice(bytes).map_err(|e| BlobStoreError::CorruptPackIndex {
        reason: format!("JSON parse error: {e}"),
    })
}

/// Encode a `BlobRecord` to JSON bytes for storage.
///
/// # Errors
///
/// Returns `BlobStoreError::SerializationFailed` if the record cannot be serialized to JSON.
pub fn encode_blob_record(record: &BlobRecord) -> Result<Vec<u8>, BlobStoreError> {
    serde_json::to_vec(record).map_err(|e| BlobStoreError::SerializationFailed {
        reason: e.to_string(),
    })
}

/// Decode JSON bytes into a `BlobRecord`.
///
/// # Errors
///
/// Returns `BlobStoreError::DeserializationFailed` if the bytes are not valid JSON
/// or do not represent a valid `BlobRecord`.
pub fn decode_blob_record(bytes: &[u8]) -> Result<BlobRecord, BlobStoreError> {
    serde_json::from_slice(bytes).map_err(|e| BlobStoreError::DeserializationFailed {
        reason: e.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Actions Layer — BlobStore Trait
// ---------------------------------------------------------------------------

/// Partition name for the blob store pack index.
pub const BLOB_STORE_PARTITION: &str = "blob_store";

/// Partition name for blob records.
pub const BLOB_RECORD_PARTITION: &str = "blob_records";

/// Storage interface for content-addressed blob storage with SHA-256 dedup.
///
/// # Streaming Semantics
///
/// Implementations must support streaming upload and download without buffering
/// the full blob in memory. The `store_streaming` method accepts an async
/// reader that yields chunks; the implementation computes SHA-256 incrementally
/// and writes to the pack file as chunks arrive.
///
/// # GC Semantics
///
/// Implementations must support lazy GC that only collects blobs when:
/// 1. Reference count drops to zero
/// 2. TTL has expired (if set)
///
/// GC must not run concurrently with itself (`GcCycleInProgress` error).
pub trait BlobStore {
    /// Store a blob from a byte slice, computing SHA-256 for content address.
    ///
    /// If the content already exists (dedup), returns `BlobStoreError::DuplicateContent`.
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::DuplicateContent` if content already exists.
    /// Returns `BlobStoreError::Storage` if the underlying storage fails.
    fn store(&self, data: &[u8]) -> Result<ContentAddress, BlobStoreError>;

    /// Store a blob from a streaming source, computing SHA-256 incrementally.
    ///
    /// The `reader` yields chunks of the blob data. The implementation must:
    /// 1. Compute SHA-256 as chunks arrive (without buffering full blob)
    /// 2. Write chunks to the current pack file
    /// 3. Return the final content address
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::DuplicateContent` if content already exists.
    /// Returns `BlobStoreError::ChecksumMismatch` if data read does not match declared address.
    /// Returns `BlobStoreError::Storage` if the underlying storage fails.
    fn store_streaming<R>(&self, reader: R) -> Result<ContentAddress, BlobStoreError>
    where
        R: tokio::io::AsyncRead + Send + Unpin + 'static;

    /// Retrieve a blob by content address into a byte vector.
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::ContentNotFound` if no blob exists for the address.
    /// Returns `BlobStoreError::Storage` if the underlying storage fails.
    fn retrieve(&self, addr: &ContentAddress) -> Result<Vec<u8>, BlobStoreError>;

    /// Retrieve a blob by content address via streaming.
    ///
    /// The `writer` receives chunks of the blob data. The implementation must:
    /// 1. Look up the pack file and offset from the pack index
    /// 2. Stream the blob data from the pack file to the writer
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::ContentNotFound` if no blob exists for the address.
    /// Returns `BlobStoreError::PackFileNotFound` if the pack file is missing.
    /// Returns `BlobStoreError::ChecksumMismatch` if data read does not match declared address.
    /// Returns `BlobStoreError::Storage` if the underlying storage fails.
    fn retrieve_streaming<W>(&self, addr: &ContentAddress, writer: W) -> Result<(), BlobStoreError>
    where
        W: tokio::io::AsyncWrite + Send + Unpin + 'static;

    /// Check if a blob exists for the given content address.
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::Storage` if the underlying storage lookup fails.
    fn contains(&self, addr: &ContentAddress) -> Result<bool, BlobStoreError>;

    /// Increment the reference count for a blob.
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::ContentNotFound` if no blob exists.
    /// Returns `BlobStoreError::Storage` if the underlying storage fails.
    fn increment_ref_count(&self, addr: &ContentAddress) -> Result<u64, BlobStoreError>;

    /// Decrement the reference count for a blob.
    ///
    /// When reference count reaches zero, the blob becomes eligible for GC
    /// (if TTL has expired or no TTL is set).
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::ContentNotFound` if no blob exists.
    /// Returns `BlobStoreError::Storage` if the underlying storage fails.
    fn decrement_ref_count(&self, addr: &ContentAddress) -> Result<u64, BlobStoreError>;

    /// Get blob metadata by content address.
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::ContentNotFound` if no blob exists.
    /// Returns `BlobStoreError::Storage` if the underlying storage fails.
    fn get_metadata(&self, addr: &ContentAddress) -> Result<BlobRecord, BlobStoreError>;

    /// List all blobs eligible for GC (reference count = 0 and expired).
    ///
    /// Returns content addresses of blobs that are safe to collect.
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::Storage` if the underlying storage lookup fails.
    fn list_gc_candidates(&self, now_ms: u64) -> Result<Vec<ContentAddress>, BlobStoreError>;

    /// Run lazy GC to collect unreferenced and expired blobs.
    ///
    /// Returns the count of blobs collected.
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::GcCycleInProgress` if GC is already running.
    /// Returns `BlobStoreError::Storage` if the underlying storage fails.
    fn run_gc(&self, now_ms: u64) -> Result<u64, BlobStoreError>;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_SHA256: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";
    const VALID_SHA256_2: &str = "ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

    #[test]
    fn content_address_constructs_with_valid_hex() {
        let s = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";
        assert_eq!(s.len(), 64, "string length should be 64");
        let addr = ContentAddress::new(s);
        assert!(addr.is_ok(), "ContentAddress::new failed: {:?}", addr);
        let addr = addr.unwrap();
        assert_eq!(addr.as_str(), s);
    }

    #[test]
    fn content_address_rejects_wrong_length() {
        let result = ContentAddress::new("abcdef");
        assert!(result.is_err());
    }

    #[test]
    fn content_address_rejects_uppercase_hex() {
        let result =
            ContentAddress::new("ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef01234567");
        assert!(result.is_err());
    }

    #[test]
    fn content_address_from_bytes_roundtrip() {
        let original = [
            0x9f_u8, 0x86, 0xd0, 0x81, 0x88, 0x84, 0xc7, 0xd6, 0x59, 0xa2, 0xfe, 0xaa, 0x0c, 0x55,
            0xad, 0x01, 0x5a, 0x3b, 0xf4, 0xf1, 0xb2, 0xb0, 0xb8, 0x22, 0xcd, 0x15, 0xd6, 0xc1,
            0x5b, 0x0f, 0x00, 0xa0,
        ];
        let addr = ContentAddress::from_bytes(&original);
        let bytes = addr.as_bytes();
        assert_eq!(bytes, original);
    }

    #[test]
    fn blob_record_constructs_with_valid_fields() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
        let record = BlobRecord::new(content_addr, 1024, 1, 1000, Some(2000));
        assert!(record.is_ok());
        let record = record.unwrap();
        assert_eq!(record.size_bytes(), 1024);
        assert_eq!(record.reference_count(), 1);
        assert!(record.expires_at_ms.is_some());
        assert_eq!(record.status(), BlobStatus::Pending);
    }

    #[test]
    fn blob_record_with_status_constructs_with_explicit_status() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
        let record = BlobRecord::with_status(
            content_addr.clone(),
            1024,
            1,
            1000,
            Some(2000),
            BlobStatus::DurablyStored,
        );
        assert_eq!(record.status(), BlobStatus::DurablyStored);
        assert_eq!(record.content_addr(), &content_addr);
        assert_eq!(record.size_bytes(), 1024);
    }

    #[test]
    fn blob_record_can_transition_from_pending_to_durably_stored() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
        let record = BlobRecord::new(content_addr, 1024, 1, 1000, None).unwrap();
        assert!(record.can_transition_to(BlobStatus::DurablyStored));
    }

    #[test]
    fn blob_record_can_transition_from_pending_to_failed() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
        let record = BlobRecord::new(content_addr, 1024, 1, 1000, None).unwrap();
        assert!(record.can_transition_to(BlobStatus::Failed));
    }

    #[test]
    fn blob_record_can_transition_from_durably_stored_to_published() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
        let record =
            BlobRecord::with_status(content_addr, 1024, 1, 1000, None, BlobStatus::DurablyStored);
        assert!(record.can_transition_to(BlobStatus::Published));
    }

    #[test]
    fn blob_record_cannot_skip_to_published_from_pending() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
        let record = BlobRecord::new(content_addr, 1024, 1, 1000, None).unwrap();
        assert!(!record.can_transition_to(BlobStatus::Published));
    }

    #[test]
    fn blob_record_published_is_terminal() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
        let record =
            BlobRecord::with_status(content_addr, 1024, 1, 1000, None, BlobStatus::Published);
        assert!(!record.can_transition_to(BlobStatus::Pending));
        assert!(!record.can_transition_to(BlobStatus::DurablyStored));
        assert!(!record.can_transition_to(BlobStatus::Failed));
    }

    #[test]
    fn blob_record_failed_is_terminal() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
        let record = BlobRecord::with_status(content_addr, 1024, 1, 1000, None, BlobStatus::Failed);
        assert!(!record.can_transition_to(BlobStatus::Pending));
        assert!(!record.can_transition_to(BlobStatus::DurablyStored));
        assert!(!record.can_transition_to(BlobStatus::Published));
    }

    #[test]
    fn blob_record_rejects_zero_reference_count() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
        let result = BlobRecord::new(content_addr, 1024, 0, 1000, None);
        assert!(result.is_err());
    }

    #[test]
    fn blob_record_expired_check() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
        let record = BlobRecord::new(content_addr, 1024, 1, 1000, Some(1500)).unwrap();
        assert!(!record.is_expired(1000));
        assert!(!record.is_expired(1499));
        assert!(record.is_expired(1500));
        assert!(record.is_expired(2000));
    }

    #[test]
    fn blob_record_never_expires_without_ttl() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
        let record = BlobRecord::new(content_addr, 1024, 1, 1000, None).unwrap();
        assert!(!record.is_expired(u64::MAX));
    }

    #[test]
    fn blob_record_increment_decrement_ref_count() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
        let record = BlobRecord::new(content_addr, 1024, 2, 1000, None).unwrap();
        assert_eq!(record.increment_ref_count(), 3);
        assert_eq!(record.decrement_ref_count(), 1);
    }

    #[test]
    fn pack_index_entry_accessors() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
        let pack_id = PackFileId::new("pack-001").unwrap();
        let entry = PackIndexEntry::new(content_addr.clone(), pack_id.clone(), 100, 512);
        assert_eq!(entry.content_addr(), &content_addr);
        assert_eq!(entry.pack_file_id(), &pack_id);
        assert_eq!(entry.offset_bytes(), 100);
        assert_eq!(entry.size_bytes(), 512);
    }

    #[test]
    fn blob_store_error_display() {
        let err = BlobStoreError::ContentNotFound {
            content_addr: "abc".to_string(),
        };
        assert!(err.to_string().contains("content not found"));
        assert!(err.to_string().contains("abc"));
    }

    #[test]
    fn blob_store_error_is_error() {
        let err = BlobStoreError::Storage {
            reason: "disk full".to_string(),
        };
        assert!(std::error::Error::source(&err).is_none());
    }

    #[test]
    fn encode_decode_content_address_roundtrip() {
        let addr = ContentAddress::new(VALID_SHA256).unwrap();
        let encoded = encode_content_address(&addr);
        let decoded = decode_content_address(&encoded).unwrap();
        assert_eq!(addr, decoded);
    }

    #[test]
    fn validate_content_address_accepts_valid() {
        assert!(validate_content_address(VALID_SHA256).is_ok());
    }

    #[test]
    fn validate_content_address_rejects_invalid() {
        assert!(validate_content_address("not-valid").is_err());
        assert!(validate_content_address("").is_err());
        assert!(validate_content_address(VALID_SHA256_2).is_err());
    }

    #[test]
    fn content_address_rejects_empty_string() {
        let result = ContentAddress::new("");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, BlobStoreError::InvalidArgument { .. }));
    }

    #[test]
    fn content_address_rejects_too_long() {
        let result = ContentAddress::new(
            "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08ab",
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, BlobStoreError::InvalidArgument { .. }));
    }

    #[test]
    fn content_address_rejects_non_hex_characters() {
        let result =
            ContentAddress::new("9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15g0f00a08");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, BlobStoreError::InvalidArgument { .. }));
        assert!(err.to_string().contains("lowercase hex"));
    }

    #[test]
    fn content_address_from_bytes_produces_correct_hex() {
        let hex_str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";
        let addr = ContentAddress::new(hex_str).unwrap();
        let bytes = addr.as_bytes();
        let roundtrip = ContentAddress::from_bytes(&bytes);
        assert_eq!(roundtrip.as_str(), hex_str);
    }

    #[test]
    fn content_address_as_str_returns_inner_string() {
        let addr = ContentAddress::new(VALID_SHA256).unwrap();
        assert_eq!(addr.as_str(), VALID_SHA256);
    }

    #[test]
    fn content_address_full_roundtrip() {
        let original = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";
        let addr = ContentAddress::new(original).unwrap();
        let bytes = addr.as_bytes();
        let roundtripped = ContentAddress::from_bytes(&bytes);
        assert_eq!(roundtripped.as_str(), original);
    }

    #[test]
    fn pack_file_id_new_accepts_non_empty() {
        let result = PackFileId::new("pack-001");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_str(), "pack-001");
    }

    #[test]
    fn pack_file_id_new_rejects_empty() {
        let result = PackFileId::new("");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, BlobStoreError::InvalidArgument { .. }));
        assert!(err.to_string().contains("cannot be empty"));
    }

    #[test]
    fn pack_file_id_as_str_returns_inner() {
        let id = PackFileId::new("pack-002").unwrap();
        assert_eq!(id.as_str(), "pack-002");
    }

    #[test]
    fn blob_record_rejects_zero_created_at() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
        let result = BlobRecord::new(content_addr, 1024, 1, 0, None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, BlobStoreError::InvalidArgument { .. }));
        assert!(err.to_string().contains("created_at_ms"));
    }

    #[test]
    fn blob_record_increment_saturates_at_max() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
        let record = BlobRecord::new(content_addr, 1024, u64::MAX, 1000, None).unwrap();
        assert_eq!(record.increment_ref_count(), u64::MAX);
    }

    #[test]
    fn blob_record_decrement_saturates_at_zero() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
        let record = BlobRecord::new(content_addr, 1024, 0, 1000, None);
        assert!(record.is_err());
    }

    #[test]
    fn blob_record_decrement_from_one_saturates_at_zero() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
        let record = BlobRecord::new(content_addr, 1024, 1, 1000, None).unwrap();
        assert_eq!(record.decrement_ref_count(), 0);
    }

    #[test]
    fn error_content_not_found_display() {
        let err = BlobStoreError::ContentNotFound {
            content_addr: "abc123".to_string(),
        };
        let s = err.to_string();
        assert!(s.contains("content not found"));
        assert!(s.contains("abc123"));
    }

    #[test]
    fn error_pack_file_not_found_display() {
        let err = BlobStoreError::PackFileNotFound {
            pack_file_id: "pack-001".to_string(),
        };
        let s = err.to_string();
        assert!(s.contains("pack file not found"));
        assert!(s.contains("pack-001"));
    }

    #[test]
    fn error_duplicate_content_display() {
        let err = BlobStoreError::DuplicateContent {
            content_addr: "def456".to_string(),
        };
        let s = err.to_string();
        assert!(s.contains("duplicate content"));
        assert!(s.contains("def456"));
    }

    #[test]
    fn error_corrupt_pack_index_display() {
        let err = BlobStoreError::CorruptPackIndex {
            reason: "missing field".to_string(),
        };
        let s = err.to_string();
        assert!(s.contains("corrupt pack index"));
        assert!(s.contains("missing field"));
    }

    #[test]
    fn error_corrupt_pack_file_display() {
        let err = BlobStoreError::CorruptPackFile {
            pack_file_id: "pack-002".to_string(),
            reason: "truncated".to_string(),
        };
        let s = err.to_string();
        assert!(s.contains("corrupt pack file pack-002"));
        assert!(s.contains("truncated"));
    }

    #[test]
    fn error_checksum_mismatch_display() {
        let err = BlobStoreError::ChecksumMismatch {
            content_addr: "abc".to_string(),
            expected: "expected_hash".to_string(),
            actual: "actual_hash".to_string(),
        };
        let s = err.to_string();
        assert!(s.contains("checksum mismatch"));
        assert!(s.contains("abc"));
        assert!(s.contains("expected_hash"));
        assert!(s.contains("actual_hash"));
    }

    #[test]
    fn error_serialization_failed_display() {
        let err = BlobStoreError::SerializationFailed {
            reason: "JSON error".to_string(),
        };
        let s = err.to_string();
        assert!(s.contains("serialization failed"));
        assert!(s.contains("JSON error"));
    }

    #[test]
    fn error_deserialization_failed_display() {
        let err = BlobStoreError::DeserializationFailed {
            reason: "invalid JSON".to_string(),
        };
        let s = err.to_string();
        assert!(s.contains("deserialization failed"));
        assert!(s.contains("invalid JSON"));
    }

    #[test]
    fn error_storage_display() {
        let err = BlobStoreError::Storage {
            reason: "disk full".to_string(),
        };
        let s = err.to_string();
        assert!(s.contains("storage error"));
        assert!(s.contains("disk full"));
    }

    #[test]
    fn error_invalid_argument_display() {
        let err = BlobStoreError::InvalidArgument {
            reason: "bad input".to_string(),
        };
        let s = err.to_string();
        assert!(s.contains("invalid argument"));
        assert!(s.contains("bad input"));
    }

    #[test]
    fn error_gc_cycle_in_progress_display() {
        let err = BlobStoreError::GcCycleInProgress;
        let s = err.to_string();
        assert!(s.contains("GC cycle already in progress"));
    }

    #[test]
    fn error_pack_file_full_display() {
        let err = BlobStoreError::PackFileFull {
            pack_file_id: "pack-003".to_string(),
            max_size_bytes: 1000,
        };
        let s = err.to_string();
        assert!(s.contains("pack file pack-003 full"));
        assert!(s.contains("1000"));
    }

    #[test]
    fn error_invalid_publication_status_display() {
        let err = BlobStoreError::InvalidPublicationStatus {
            content_addr: "abc123".to_string(),
            current_status: "Pending".to_string(),
            attempted_operation: "publish".to_string(),
        };
        let s = err.to_string();
        assert!(s.contains("invalid publication status"));
        assert!(s.contains("abc123"));
        assert!(s.contains("Pending"));
        assert!(s.contains("publish"));
    }

    #[test]
    fn error_not_durably_stored_display() {
        let err = BlobStoreError::NotDurablyStored {
            content_addr: "def456".to_string(),
        };
        let s = err.to_string();
        assert!(s.contains("not durably stored"));
        assert!(s.contains("def456"));
    }

    #[test]
    fn all_blob_store_error_variants_implement_error_trait() {
        fn assert_impl<T: std::error::Error>() {}
        assert_impl::<BlobStoreError>();
    }

    #[test]
    fn encode_content_address_produces_valid_utf8() {
        let addr = ContentAddress::new(VALID_SHA256).unwrap();
        let encoded = encode_content_address(&addr);
        let as_str = String::from_utf8(encoded.clone()).unwrap();
        assert_eq!(as_str, VALID_SHA256);
    }

    #[test]
    fn decode_content_address_rejects_invalid_utf8() {
        let invalid_utf8 = [0x80, 0x81, 0x82, 0x83];
        let result = decode_content_address(&invalid_utf8);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, BlobStoreError::CorruptPackIndex { .. }));
    }

    #[test]
    fn decode_content_address_rejects_invalid_format() {
        let invalid_format = b"not-a-valid-content-address".to_vec();
        let result = decode_content_address(&invalid_format);
        assert!(result.is_err());
    }

    #[test]
    fn decode_content_address_rejects_wrong_length() {
        let too_short = b"abc123".to_vec();
        let result = decode_content_address(&too_short);
        assert!(result.is_err());
    }

    #[test]
    fn encode_pack_index_entry_and_decode_roundtrip() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
        let pack_id = PackFileId::new("pack-001").unwrap();
        let entry = PackIndexEntry::new(content_addr, pack_id, 100, 512);
        let encoded = encode_pack_index_entry(&entry).unwrap();
        let decoded = decode_pack_index_entry(&encoded).unwrap();
        assert_eq!(decoded.content_addr(), entry.content_addr());
        assert_eq!(decoded.pack_file_id(), entry.pack_file_id());
        assert_eq!(decoded.offset_bytes(), entry.offset_bytes());
        assert_eq!(decoded.size_bytes(), entry.size_bytes());
    }

    #[test]
    fn encode_blob_record_and_decode_roundtrip() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
        let record = BlobRecord::new(content_addr, 1024, 1, 1000, Some(2000)).unwrap();
        let encoded = encode_blob_record(&record).unwrap();
        let decoded = decode_blob_record(&encoded).unwrap();
        assert_eq!(decoded.content_addr(), record.content_addr());
        assert_eq!(decoded.size_bytes(), record.size_bytes());
        assert_eq!(decoded.reference_count(), record.reference_count());
        assert_eq!(decoded.created_at_ms(), record.created_at_ms());
        assert_eq!(decoded.expires_at_ms(), record.expires_at_ms());
    }

    #[test]
    fn decode_pack_index_entry_rejects_invalid_json() {
        let invalid_json = b"not json".to_vec();
        let result = decode_pack_index_entry(&invalid_json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, BlobStoreError::CorruptPackIndex { .. }));
    }

    #[test]
    fn decode_blob_record_rejects_invalid_json() {
        let invalid_json = b"not json".to_vec();
        let result = decode_blob_record(&invalid_json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, BlobStoreError::DeserializationFailed { .. }));
    }
}

#[cfg(feature = "proptest")]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn content_address_byte_roundtrip(bytes: [u8; 32]) {
            let addr = ContentAddress::from_bytes(&bytes);
            let recovered = addr.as_bytes();
            prop_assert_eq!(recovered, bytes);
        }

        #[test]
        fn content_address_str_validity(hex in "[a-f0-9]{64}") {
            let addr = ContentAddress::new(&hex);
            prop_assert!(addr.is_ok());
            let addr = addr.unwrap();
            prop_assert_eq!(addr.as_str().len(), 64);
            prop_assert!(addr.as_str().chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        }

        #[test]
        fn content_address_from_bytes_to_str_roundtrip(bytes: [u8; 32]) {
            let addr = ContentAddress::from_bytes(&bytes);
            let hex_str = addr.as_str();
            let roundtripped = ContentAddress::new(hex_str).unwrap();
            prop_assert_eq!(addr, roundtripped);
        }

        #[test]
        fn pack_file_id_rejects_empty(id in "[a-zA-Z0-9_-]*") {
            if id.is_empty() {
                prop_assert!(PackFileId::new(&id).is_err());
            } else {
                prop_assert!(PackFileId::new(&id).is_ok());
            }
        }

        #[test]
        fn blob_record_ref_count_increment_saturates(ref_count: u64) {
            let content_addr = ContentAddress::new("9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08").unwrap();
            let record = BlobRecord::new(content_addr, 1024, ref_count, 1000, None).ok();
            if let Some(record) = record {
                let incremented = record.increment_ref_count();
                prop_assert!(incremented >= ref_count);
                if ref_count < u64::MAX {
                    prop_assert_eq!(incremented, ref_count + 1);
                } else {
                    prop_assert_eq!(incremented, u64::MAX);
                }
            }
        }

        #[test]
        fn blob_record_expiry_is_monotonic(
            ref_count in 1u64..100u64,
            created_at in 1u64..u64::MAX.saturating_sub(1000),
            expires_at in 1u64..u64::MAX,
        ) {
            let content_addr = ContentAddress::new("9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08").unwrap();
            let record = BlobRecord::new(content_addr, 1024, ref_count, created_at, Some(expires_at)).unwrap();
            let now = expires_at.saturating_sub(1);
            let later = expires_at.saturating_add(1);
            let is_expired_now = record.is_expired(now);
            let is_expired_later = record.is_expired(later);
            if is_expired_now {
                prop_assert!(is_expired_later);
            }
        }
    }

    proptest! {
        #[test]
        fn content_address_encoding_roundtrip(bytes: [u8; 32]) {
            let addr = ContentAddress::from_bytes(&bytes);
            let encoded = encode_content_address(&addr);
            let decoded = decode_content_address(&encoded).unwrap();
            prop_assert_eq!(addr, decoded);
        }
    }

    proptest! {
        #[test]
        fn pack_index_entry_roundtrip(
            content in "[a-f0-9]{64}",
            pack_id in "[a-zA-Z0-9_-]{1,100}",
            offset in 0u64..u64::MAX,
            size in 0u64..u64::MAX,
        ) {
            let content_addr = ContentAddress::new(&content).unwrap();
            let pack_id = PackFileId::new(&pack_id).unwrap();
            let entry = PackIndexEntry::new(content_addr, pack_id, offset, size);
            let encoded = encode_pack_index_entry(&entry).unwrap();
            let decoded = decode_pack_index_entry(&encoded).unwrap();
            prop_assert_eq!(decoded.content_addr(), entry.content_addr());
            prop_assert_eq!(decoded.pack_file_id(), entry.pack_file_id());
            prop_assert_eq!(decoded.offset_bytes(), entry.offset_bytes());
            prop_assert_eq!(decoded.size_bytes(), entry.size_bytes());
        }
    }

    #[test]
    fn blob_record_is_gc_eligible_requires_both_conditions() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();

        let record = BlobRecord::new(content_addr.clone(), 1024, 0, 1000, Some(2000)).unwrap();
        assert!(!record.is_gc_eligible(1500), "ref=0 but not expired yet");

        let record = BlobRecord::new(content_addr.clone(), 1024, 1, 1000, Some(1500)).unwrap();
        assert!(!record.is_gc_eligible(2000), "expired but ref=1");

        let record = BlobRecord::new(content_addr.clone(), 1024, 0, 1000, Some(1500)).unwrap();
        assert!(record.is_gc_eligible(1500), "both ref=0 and expired");

        let record = BlobRecord::new(content_addr.clone(), 1024, 0, 1000, Some(1500)).unwrap();
        assert!(!record.is_gc_eligible(1499), "expired at 1500, not at 1499");
    }

    #[test]
    fn blob_record_gc_eligible_without_ttl() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
        let record = BlobRecord::new(content_addr.clone(), 1024, 0, 1000, None).unwrap();
        assert!(
            !record.is_gc_eligible(u64::MAX),
            "no TTL means never expires, even with ref=0"
        );
    }

    #[test]
    fn blob_record_status_transition_consistency() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();

        let pending = BlobRecord::new(content_addr.clone(), 1024, 1, 1000, None).unwrap();
        assert!(pending.can_transition_to(BlobStatus::DurablyStored));
        assert!(pending.can_transition_to(BlobStatus::Failed));
        assert!(!pending.can_transition_to(BlobStatus::Published));

        let stored = BlobRecord::with_status(
            content_addr.clone(),
            1024,
            1,
            1000,
            None,
            BlobStatus::DurablyStored,
        );
        assert!(stored.can_transition_to(BlobStatus::Published));
        assert!(!stored.can_transition_to(BlobStatus::Pending));
        assert!(!stored.can_transition_to(BlobStatus::Failed));
    }

    #[test]
    fn blob_record_terminal_states_no_transitions() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();

        let published = BlobRecord::with_status(
            content_addr.clone(),
            1024,
            1,
            1000,
            None,
            BlobStatus::Published,
        );
        assert!(!published.can_transition_to(BlobStatus::Pending));
        assert!(!published.can_transition_to(BlobStatus::DurablyStored));
        assert!(!published.can_transition_to(BlobStatus::Failed));

        let failed = BlobRecord::with_status(
            content_addr.clone(),
            1024,
            1,
            1000,
            None,
            BlobStatus::Failed,
        );
        assert!(!failed.can_transition_to(BlobStatus::Pending));
        assert!(!failed.can_transition_to(BlobStatus::DurablyStored));
        assert!(!failed.can_transition_to(BlobStatus::Published));
    }

    #[test]
    fn blob_record_decrement_from_zero_returns_zero() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
        let record = BlobRecord::new(content_addr, 1024, 0, 1000, None);
        assert!(record.is_err(), "cannot create record with ref_count=0");
    }

    #[test]
    fn blob_record_reference_count_saturation_bounds() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();

        let record = BlobRecord::new(content_addr.clone(), 1024, u64::MAX - 1, 1000, None).unwrap();
        assert_eq!(record.increment_ref_count(), u64::MAX);

        let record = BlobRecord::new(content_addr.clone(), 1024, u64::MAX, 1000, None).unwrap();
        assert_eq!(record.increment_ref_count(), u64::MAX);
    }

    #[test]
    fn content_address_validity_invariant() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
        assert_eq!(content_addr.as_str().len(), 64);
        assert!(content_addr
            .as_str()
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn content_address_bytes_roundtrip_preserves_invariant() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
        let bytes = content_addr.as_bytes();
        let recovered = ContentAddress::from_bytes(&bytes);
        assert_eq!(recovered.as_str(), content_addr.as_str());
    }

    #[test]
    fn pack_index_entry_immutable_after_construction() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
        let pack_id = PackFileId::new("pack-001").unwrap();
        let entry = PackIndexEntry::new(content_addr.clone(), pack_id.clone(), 100, 512);

        assert_eq!(entry.content_addr(), &content_addr);
        assert_eq!(entry.pack_file_id(), &pack_id);
        assert_eq!(entry.offset_bytes(), 100);
        assert_eq!(entry.size_bytes(), 512);
    }

    #[test]
    fn blob_store_error_is_transient_classification() {
        let transient_errors = vec![
            BlobStoreError::Storage {
                reason: "disk full".to_string(),
            },
            BlobStoreError::DuplicateContent {
                content_addr: "abc".to_string(),
            },
            BlobStoreError::GcCycleInProgress,
            BlobStoreError::PackFileFull {
                pack_file_id: "pack-001".to_string(),
                max_size_bytes: 1000,
            },
        ];

        for err in transient_errors {
            assert!(err.is_transient(), "Expected {:?} to be transient", err);
        }

        let fatal_errors = vec![
            BlobStoreError::CorruptPackIndex {
                reason: "bad index".to_string(),
            },
            BlobStoreError::CorruptPackFile {
                pack_file_id: "pack-001".to_string(),
                reason: "truncated".to_string(),
            },
            BlobStoreError::ChecksumMismatch {
                content_addr: "abc".to_string(),
                expected: "def".to_string(),
                actual: "ghi".to_string(),
            },
            BlobStoreError::InvalidArgument {
                reason: "bad input".to_string(),
            },
        ];

        for err in fatal_errors {
            assert!(err.is_fatal(), "Expected {:?} to be fatal", err);
        }

        let not_transient_or_fatal = BlobStoreError::ContentNotFound {
            content_addr: "abc".to_string(),
        };
        assert!(!not_transient_or_fatal.is_transient());
        assert!(!not_transient_or_fatal.is_fatal());
    }

    #[test]
    fn blob_record_with_status_allows_direct_status_construction() {
        let content_addr = ContentAddress::new(VALID_SHA256).unwrap();

        let record = BlobRecord::with_status(
            content_addr.clone(),
            1024,
            1,
            1000,
            Some(2000),
            BlobStatus::Pending,
        );
        assert_eq!(record.status(), BlobStatus::Pending);

        let record = BlobRecord::with_status(
            content_addr.clone(),
            1024,
            1,
            1000,
            Some(2000),
            BlobStatus::DurablyStored,
        );
        assert_eq!(record.status(), BlobStatus::DurablyStored);

        let record = BlobRecord::with_status(
            content_addr.clone(),
            1024,
            1,
            1000,
            Some(2000),
            BlobStatus::Published,
        );
        assert_eq!(record.status(), BlobStatus::Published);

        let record = BlobRecord::with_status(
            content_addr.clone(),
            1024,
            1,
            1000,
            Some(2000),
            BlobStatus::Failed,
        );
        assert_eq!(record.status(), BlobStatus::Failed);
    }

    #[test]
    fn content_address_from_bytes_invalidates_uppercase() {
        let bytes = [
            0xAB_u8, 0xCD, 0xEF, 0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0x01, 0x23, 0x45,
            0x67, 0x89, 0xAB, 0xCD, 0xEF, 0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0x01,
            0x23, 0x45, 0x67, 0x89,
        ];
        let addr = ContentAddress::from_bytes(&bytes);
        let hex_str = addr.as_str();
        assert!(
            hex_str
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "from_bytes must produce lowercase hex"
        );
    }

    proptest! {
        #[test]
        fn blob_record_roundtrip(
            content in "[a-f0-9]{64}",
            size in 0u64..u64::MAX,
            ref_count in 1u64..u64::MAX,
            created_at in 1u64..u64::MAX,
            has_expiry in any::<bool>(),
            expires_offset in 0u64..10000u64,
        ) {
            let content_addr = ContentAddress::new(&content).unwrap();
            let expires_at_ms = if has_expiry {
                Some(created_at.saturating_add(expires_offset))
            } else {
                None
            };
            let record = BlobRecord::new(content_addr, size, ref_count, created_at, expires_at_ms).unwrap();
            let encoded = encode_blob_record(&record).unwrap();
            let decoded = decode_blob_record(&encoded).unwrap();
            prop_assert_eq!(decoded.content_addr(), record.content_addr());
            prop_assert_eq!(decoded.size_bytes(), record.size_bytes());
            prop_assert_eq!(decoded.reference_count(), record.reference_count());
            prop_assert_eq!(decoded.created_at_ms(), record.created_at_ms());
            prop_assert_eq!(decoded.expires_at_ms(), record.expires_at_ms());
        }
    }
}
