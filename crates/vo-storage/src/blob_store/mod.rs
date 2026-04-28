//! Content-addressed blob storage with SHA-256 dedup, pack files, and lazy GC.
//!
//! Architecture: Data → Calc → Actions
//!
//! ## Data Layer
//!
//! - [`content_address::ContentAddress`]: SHA-256 content hash (64 lowercase hex chars)
//! - [`pack_index::PackFileId`]: Unique identifier for a pack file
//! - [`pack_index::PackIndexEntry`]: Maps content address to pack file location
//! - [`blob_record::BlobRecord`]: Persisted blob metadata
//! - [`error::BlobStoreError`]: Error taxonomy
//!
//! ## Calc Layer
//!
//! - [`content_address::encode_content_address`], [`content_address::decode_content_address`]: Content address encoding
//! - [`pack_index::encode_pack_index_entry`], [`pack_index::decode_pack_index_entry`]: Pack index encoding
//! - [`blob_record::encode_blob_record`], [`blob_record::decode_blob_record`]: Blob record encoding
//! - [`content_address::validate_content_address`]: Validates SHA-256 hex format
//!
//! ## Actions Layer
//!
//! - [`trait_::BlobStore`] trait: Storage interface for content-addressed blobs
//!
//! ## Invariants
//!
//! 1. Content address is always a valid 64-char lowercase hex SHA-256 hash
//! 2. Pack index entry uniquely maps content address → pack file + offset
//! 3. Blob record is immutable once written (append-only pack files)
//! 4. GC only collects blobs with zero reference count and expired TTL
//! 5. Streaming upload/download never buffers full blob in memory

pub mod content_address;
pub mod error;
pub mod pack_index;
mod blob_record;

#[doc(hidden)]
pub mod trait_ {
    pub use super::blob_store_trait::*;
}
#[path = "trait.rs"]
mod blob_store_trait;

pub use content_address::ContentAddress;
pub use error::BlobStoreError;
pub use pack_index::PackFileId;
pub use pack_index::PackIndexEntry;
pub use blob_record::BlobRecord;
pub use blob_store_trait::BlobStore;

pub use content_address::encode_content_address;
pub use content_address::decode_content_address;
pub use content_address::validate_content_address;
pub use pack_index::encode_pack_index_entry;
pub use pack_index::decode_pack_index_entry;
pub use blob_record::encode_blob_record;
pub use blob_record::decode_blob_record;
pub use blob_store_trait::BLOB_STORE_PARTITION;
pub use blob_store_trait::BLOB_RECORD_PARTITION;
