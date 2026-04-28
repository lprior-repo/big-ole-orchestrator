//! Encoding/decoding helpers for content addresses, pack index entries, and blob records.

use super::error::BlobStoreError;
use super::record::BlobRecord;
use super::types::{ContentAddress, PackIndexEntry};

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
