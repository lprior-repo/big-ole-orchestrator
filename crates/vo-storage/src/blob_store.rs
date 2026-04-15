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
                reason: "content address must be lowercase hex (0-9, a-f)".to_string(),
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
