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
}

impl BlobRecord {
    /// Construct a new `BlobRecord`.
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
        })
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

    /// Check if this record has expired given the current timestamp.
    #[must_use]
    pub const fn is_expired(&self, now_ms: u64) -> bool {
        match self.expires_at_ms {
            Some(expires) => now_ms >= expires,
            None => false,
        }
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
}

// ---------------------------------------------------------------------------
// Data Layer — BlobStoreError
// ---------------------------------------------------------------------------

/// Errors from content-addressed blob store operations.
#[non_exhaustive]
#[derive(Debug, PartialEq, Eq)]
pub enum BlobStoreError {
    /// No blob exists for the given content address.
    ContentNotFound { content_addr: String },
    /// Referenced pack file does not exist.
    PackFileNotFound { pack_file_id: String },
    /// Content already exists (dedup violation on strict insert).
    DuplicateContent { content_addr: String },
    /// Pack index entry is malformed.
    CorruptPackIndex { reason: String },
    /// Pack file data does not match content address.
    CorruptPackFile {
        pack_file_id: String,
        reason: String,
    },
    /// Computed SHA-256 does not match declared content address.
    ChecksumMismatch {
        content_addr: String,
        expected: String,
        actual: String,
    },
    /// Blob metadata serialization failed.
    SerializationFailed { reason: String },
    /// Blob metadata deserialization failed.
    DeserializationFailed { reason: String },
    /// Underlying storage operation failed.
    Storage { reason: String },
    /// Invalid input argument.
    InvalidArgument { reason: String },
    /// GC cycle already running (prevents concurrent GC).
    GcCycleInProgress,
    /// Pack file has reached maximum size (forces new pack).
    PackFileFull {
        pack_file_id: String,
        max_size_bytes: u64,
    },
}

impl fmt::Display for BlobStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContentNotFound { content_addr } => {
                write!(f, "content not found: {content_addr}")
            }
            Self::PackFileNotFound { pack_file_id } => {
                write!(f, "pack file not found: {pack_file_id}")
            }
            Self::DuplicateContent { content_addr } => {
                write!(f, "duplicate content: {content_addr}")
            }
            Self::CorruptPackIndex { reason } => {
                write!(f, "corrupt pack index: {reason}")
            }
            Self::CorruptPackFile {
                pack_file_id,
                reason,
            } => {
                write!(f, "corrupt pack file {pack_file_id}: {reason}")
            }
            Self::ChecksumMismatch {
                content_addr,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "checksum mismatch for {content_addr}: expected {expected}, got {actual}"
                )
            }
            Self::SerializationFailed { reason } => {
                write!(f, "serialization failed: {reason}")
            }
            Self::DeserializationFailed { reason } => {
                write!(f, "deserialization failed: {reason}")
            }
            Self::Storage { reason } => {
                write!(f, "storage error: {reason}")
            }
            Self::InvalidArgument { reason } => {
                write!(f, "invalid argument: {reason}")
            }
            Self::GcCycleInProgress => {
                write!(f, "GC cycle already in progress")
            }
            Self::PackFileFull {
                pack_file_id,
                max_size_bytes,
            } => {
                write!(
                    f,
                    "pack file {pack_file_id} full (max {max_size_bytes} bytes)"
                )
            }
        }
    }
}

impl std::error::Error for BlobStoreError {}

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
/// Returns `BlobStoreError::DeserializationFailed` if the bytes are not valid JSON
/// or do not represent a valid `PackIndexEntry`.
pub fn decode_pack_index_entry(bytes: &[u8]) -> Result<PackIndexEntry, BlobStoreError> {
    serde_json::from_slice(bytes).map_err(|e| BlobStoreError::DeserializationFailed {
        reason: e.to_string(),
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
}
