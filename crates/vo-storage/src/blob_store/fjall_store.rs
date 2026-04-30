//! Fjall-backed persistent blob store using LSM-tree storage with KV separation.
//!
//! Architecture: Data → Calc → Actions
//!
//! ## Data Layer
//!
//! - `FjallBlobStore`: Store struct holding Fjall keyspace
//! - `BlobStoreError`: Error taxonomy
//!
//! ## Calc Layer
//!
//! - Content address encoding (hex string to bytes)
//!
//! ## Actions Layer
//!
//! - `open()`: Open store with BLOB class partition config
//! - `get()`: Retrieve blob by content address
//! - `put()`: Store blob with content address as key
//! - `contains()`: Check if blob exists
//! - `delete()`: Remove blob by content address
//!
//! ## KV Separation
//!
//! Uses Fjall's built-in KV separation to efficiently store large blobs.
//! Blobs larger than 1KB threshold are automatically separated from keys,
//! with values stored in separate files (per Fjall's architecture).

use std::sync::Arc;

use sha2::Digest;

use super::content_address::{encode_content_address, ContentAddress};
use super::error::BlobStoreError;
use crate::partitions::{get_partition_config, PAYLOAD_BLOBS_PARTITION};

/// Fjall-backed blob store for encrypted canonical payload blobs.
///
/// Uses Fjall's KV separation feature for efficient large blob storage.
/// Each blob is stored with its content address (SHA-256 hex) as the key
/// and the raw blob bytes as the value.
pub struct FjallBlobStore {
    partition: Arc<fjall::Keyspace>,
}

impl std::fmt::Debug for FjallBlobStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FjallBlobStore").finish()
    }
}

impl FjallBlobStore {
    /// Open a Fjall blob store with BLOB class partition configuration.
    ///
    /// Uses `PartitionConfig::blob()` which enables KV separation for
    /// efficient storage of large blobs (>1KB threshold).
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::Storage` if the partition cannot be opened.
    #[allow(clippy::expect_used)]
    pub fn open(db: &fjall::Database) -> Result<Self, BlobStoreError> {
        let config = get_partition_config(PAYLOAD_BLOBS_PARTITION);
        let partition = db
            .keyspace(PAYLOAD_BLOBS_PARTITION, || config.to_fjall_options())
            .map_err(|e| BlobStoreError::Storage {
                reason: format!("failed to open payload_blobs partition: {e}"),
            })?;
        Ok(Self {
            partition: Arc::new(partition),
        })
    }

    /// Retrieve a blob by content address.
    ///
    /// Verifies integrity by computing SHA-256 of retrieved data and comparing
    /// against the content address. Returns `ChecksumMismatch` if verification fails.
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::ContentNotFound` if no blob exists.
    /// Returns `BlobStoreError::ChecksumMismatch` if integrity verification fails.
    /// Returns `BlobStoreError::Storage` if the underlying storage fails.
    pub fn get(&self, addr: &ContentAddress) -> Result<Vec<u8>, BlobStoreError> {
        let key = encode_content_address(addr);
        match self.partition.get(&key) {
            Ok(Some(value)) => {
                let computed = Self::compute_content_address(&value);
                if computed != *addr {
                    return Err(BlobStoreError::ChecksumMismatch {
                        content_addr: addr.to_string(),
                        expected: addr.to_string(),
                        actual: computed.to_string(),
                    });
                }
                Ok(value.to_vec())
            }
            Ok(None) => Err(BlobStoreError::ContentNotFound {
                content_addr: addr.to_string(),
            }),
            Err(e) => Err(BlobStoreError::Storage {
                reason: format!("failed to get blob: {e}"),
            }),
        }
    }

    /// Store a blob, returning its content address.
    ///
    /// The content address is computed as SHA-256 of the provided data.
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::DuplicateContent` if content already exists.
    /// Returns `BlobStoreError::Storage` if the underlying storage fails.
    pub fn put(&self, data: &[u8]) -> Result<ContentAddress, BlobStoreError> {
        let addr = Self::compute_content_address(data);
        let key = encode_content_address(&addr);

        if self.contains(&addr)? {
            return Err(BlobStoreError::DuplicateContent {
                content_addr: addr.to_string(),
            });
        }

        self.partition
            .insert(&key, data)
            .map_err(|e| BlobStoreError::Storage {
                reason: format!("failed to put blob: {e}"),
            })?;

        Ok(addr)
    }

    /// Check if a blob exists for the given content address.
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::Storage` if the check fails.
    pub fn contains(&self, addr: &ContentAddress) -> Result<bool, BlobStoreError> {
        let key = encode_content_address(addr);
        match self.partition.get(&key) {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(BlobStoreError::Storage {
                reason: format!("failed to check blob existence: {e}"),
            }),
        }
    }

    /// Delete a blob by content address.
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::ContentNotFound` if no blob exists.
    /// Returns `BlobStoreError::Storage` if the underlying storage fails.
    pub fn delete(&self, addr: &ContentAddress) -> Result<(), BlobStoreError> {
        let key = encode_content_address(addr);
        match self.partition.get(&key) {
            Ok(Some(_)) => {
                self.partition
                    .remove(&key)
                    .map_err(|e| BlobStoreError::Storage {
                        reason: format!("failed to delete blob: {e}"),
                    })?;
                Ok(())
            }
            Ok(None) => Err(BlobStoreError::ContentNotFound {
                content_addr: addr.to_string(),
            }),
            Err(e) => Err(BlobStoreError::Storage {
                reason: format!("failed to check before delete: {e}"),
            }),
        }
    }

    /// Compute SHA-256 content address for blob data.
    #[must_use]
    fn compute_content_address(data: &[u8]) -> ContentAddress {
        let digest = sha2::Sha256::digest(data);
        let bytes: [u8; 32] = digest.into();
        ContentAddress::from_bytes(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_temp_store() -> FjallBlobStore {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        FjallBlobStore::open(&db).unwrap()
    }

    #[test]
    fn put_and_get_roundtrip() {
        let store = make_temp_store();
        let data = b"hello, world!";
        let addr = store.put(data).unwrap();

        let retrieved = store.get(&addr).unwrap();
        assert_eq!(retrieved, data);
    }

    #[test]
    fn put_rejects_duplicate() {
        let store = make_temp_store();
        let data = b"unique content";
        let _ = store.put(data).unwrap();

        let result = store.put(data);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, BlobStoreError::DuplicateContent { .. }));
    }

    #[test]
    fn get_returns_not_found_for_missing() {
        let store = make_temp_store();
        let addr =
            ContentAddress::new("9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08")
                .unwrap();

        let result = store.get(&addr);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            BlobStoreError::ContentNotFound { .. }
        ));
    }

    #[test]
    fn contains_returns_true_for_existing() {
        let store = make_temp_store();
        let addr = store.put(b"exists").unwrap();
        assert!(store.contains(&addr).unwrap());
    }

    #[test]
    fn contains_returns_false_for_missing() {
        let store = make_temp_store();
        let addr =
            ContentAddress::new("9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08")
                .unwrap();
        assert!(!store.contains(&addr).unwrap());
    }

    #[test]
    fn delete_removes_blob() {
        let store = make_temp_store();
        let addr = store.put(b"to-delete").unwrap();
        assert!(store.contains(&addr).unwrap());

        store.delete(&addr).unwrap();
        assert!(!store.contains(&addr).unwrap());
    }

    #[test]
    fn delete_returns_not_found_for_missing() {
        let store = make_temp_store();
        let addr =
            ContentAddress::new("9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08")
                .unwrap();

        let result = store.delete(&addr);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            BlobStoreError::ContentNotFound { .. }
        ));
    }

    #[test]
    fn put_produces_correct_sha256() {
        let store = make_temp_store();
        let data = b"test";
        let addr = store.put(data).unwrap();

        let expected = sha2::Sha256::digest(data);
        let expected_hex: String = expected.iter().fold(String::new(), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        });
        assert_eq!(addr.as_str(), expected_hex);
    }

    #[test]
    fn store_persists_across_restart() {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        let store = FjallBlobStore::open(&db).unwrap();
        let data = b"persistent data";
        let addr = store.put(data).unwrap();
        drop(store);
        drop(db);

        let db2 = fjall::Database::builder(dir.path()).open().unwrap();
        let store2 = FjallBlobStore::open(&db2).unwrap();
        let retrieved = store2.get(&addr).unwrap();
        assert_eq!(retrieved, data);
    }

    #[test]
    fn get_verifies_integrity_on_retrieve() {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        let store = FjallBlobStore::open(&db).unwrap();
        let data = b"integrity test data";
        let addr = store.put(data).unwrap();

        let retrieved = store.get(&addr).unwrap();
        assert_eq!(retrieved, data);
    }

    #[test]
    fn get_checksum_mismatch_after_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        let store = FjallBlobStore::open(&db).unwrap();
        let data = b"original data";
        let addr = store.put(data).unwrap();

        // Corrupt the blob directly in the partition
        let corrupted = b"corrupted data!!!";
        let key = content_address::encode_content_address(&addr);
        store.partition.insert(&key, corrupted).unwrap();

        // Get should fail with checksum mismatch
        let result = store.get(&addr);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, BlobStoreError::ChecksumMismatch { .. }));
    }

    use std::fmt::Write;
}
