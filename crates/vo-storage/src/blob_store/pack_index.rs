//! Pack file index types and encoding.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::content_address::ContentAddress;
use super::error::BlobStoreError;

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
