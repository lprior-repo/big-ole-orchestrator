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

pub mod blob_record;
pub mod content_address;
pub mod error;
pub mod fjall_store;
pub mod pack_index;
pub mod r#trait;

pub use blob_record::{decode_blob_record, encode_blob_record, BlobRecord};
pub use content_address::{
    decode_content_address, encode_content_address, validate_content_address, ContentAddress,
};
pub use error::BlobStoreError;
pub use pack_index::{
    decode_pack_index_entry, encode_pack_index_entry, PackFileId, PackIndexEntry,
};
pub use r#trait::{BlobStore, BLOB_RECORD_PARTITION, BLOB_STORE_PARTITION};
