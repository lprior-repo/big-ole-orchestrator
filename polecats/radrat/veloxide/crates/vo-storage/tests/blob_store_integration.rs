use std::cell::RefCell;
use std::collections::HashMap;
use vo_storage::blob_store::{BlobRecord, BlobStore, BlobStoreError, ContentAddress, PackFileId};

struct InMemoryBlobStore {
    blobs: RefCell<HashMap<ContentAddress, Vec<u8>>>,
    records: RefCell<HashMap<ContentAddress, BlobRecord>>,
    pack_index: RefCell<HashMap<ContentAddress, (PackFileId, u64, u64)>>,
    gc_in_progress: RefCell<bool>,
}

impl InMemoryBlobStore {
    fn new() -> Self {
        Self {
            blobs: RefCell::new(HashMap::new()),
            records: RefCell::new(HashMap::new()),
            pack_index: RefCell::new(HashMap::new()),
            gc_in_progress: RefCell::new(false),
        }
    }
}

impl BlobStore for InMemoryBlobStore {
    fn store(&self, _data: &[u8]) -> Result<ContentAddress, BlobStoreError> {
        Err(BlobStoreError::Storage {
            reason: "not implemented".to_string(),
        })
    }

    fn store_streaming<R>(&self, _reader: R) -> Result<ContentAddress, BlobStoreError>
    where
        R: tokio::io::AsyncRead + Send + Unpin + 'static,
    {
        Err(BlobStoreError::Storage {
            reason: "not implemented".to_string(),
        })
    }

    fn retrieve(&self, addr: &ContentAddress) -> Result<Vec<u8>, BlobStoreError> {
        self.blobs
            .borrow()
            .get(addr)
            .cloned()
            .ok_or_else(|| BlobStoreError::ContentNotFound {
                content_addr: addr.to_string(),
            })
    }

    fn retrieve_streaming<W>(
        &self,
        _addr: &ContentAddress,
        _writer: W,
    ) -> Result<(), BlobStoreError>
    where
        W: tokio::io::AsyncWrite + Send + Unpin + 'static,
    {
        Err(BlobStoreError::Storage {
            reason: "not implemented".to_string(),
        })
    }

    fn contains(&self, addr: &ContentAddress) -> Result<bool, BlobStoreError> {
        Ok(self.blobs.borrow().contains_key(addr))
    }

    fn increment_ref_count(&self, addr: &ContentAddress) -> Result<u64, BlobStoreError> {
        let record = self
            .records
            .borrow()
            .get(addr)
            .ok_or_else(|| BlobStoreError::ContentNotFound {
                content_addr: addr.to_string(),
            })?
            .clone();

        let new_count = record.increment_ref_count();
        let new_record = BlobRecord::new(
            record.content_addr().clone(),
            record.size_bytes(),
            new_count,
            record.created_at_ms(),
            record.expires_at_ms(),
        )
        .map_err(|e| BlobStoreError::InvalidArgument {
            reason: e.to_string(),
        })?;

        self.records.borrow_mut().insert(addr.clone(), new_record);
        Ok(new_count)
    }

    fn decrement_ref_count(&self, addr: &ContentAddress) -> Result<u64, BlobStoreError> {
        let record = self
            .records
            .borrow()
            .get(addr)
            .ok_or_else(|| BlobStoreError::ContentNotFound {
                content_addr: addr.to_string(),
            })?
            .clone();

        let new_count = record.decrement_ref_count();
        let new_record = BlobRecord::new(
            record.content_addr().clone(),
            record.size_bytes(),
            new_count,
            record.created_at_ms(),
            record.expires_at_ms(),
        )
        .map_err(|e| BlobStoreError::InvalidArgument {
            reason: e.to_string(),
        })?;

        self.records.borrow_mut().insert(addr.clone(), new_record);
        Ok(new_count)
    }

    fn get_metadata(&self, addr: &ContentAddress) -> Result<BlobRecord, BlobStoreError> {
        self.records
            .borrow()
            .get(addr)
            .cloned()
            .ok_or_else(|| BlobStoreError::ContentNotFound {
                content_addr: addr.to_string(),
            })
    }

    fn list_gc_candidates(&self, now_ms: u64) -> Result<Vec<ContentAddress>, BlobStoreError> {
        Ok(self
            .records
            .borrow()
            .iter()
            .filter(|(_, record)| record.reference_count() == 0 && record.is_expired(now_ms))
            .map(|(addr, _)| addr.clone())
            .collect())
    }

    fn run_gc(&self, now_ms: u64) -> Result<u64, BlobStoreError> {
        if *self.gc_in_progress.borrow() {
            return Err(BlobStoreError::GcCycleInProgress);
        }

        *self.gc_in_progress.borrow_mut() = true;

        let candidates: Vec<ContentAddress> = self
            .records
            .borrow()
            .iter()
            .filter(|(_, record)| record.reference_count() == 0 && record.is_expired(now_ms))
            .map(|(addr, _)| addr.clone())
            .collect();

        let count = candidates.len() as u64;

        for addr in &candidates {
            self.blobs.borrow_mut().remove(addr);
            self.records.borrow_mut().remove(addr);
            self.pack_index.borrow_mut().remove(addr);
        }

        *self.gc_in_progress.borrow_mut() = false;

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_SHA256: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";

    #[test]
    fn blob_store_contains_returns_false_for_unknown_content() {
        let store = InMemoryBlobStore::new();
        let addr = ContentAddress::new(VALID_SHA256).unwrap();
        let result = store.contains(&addr);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn blob_store_retrieve_returns_content_not_found_for_unknown() {
        let store = InMemoryBlobStore::new();
        let addr = ContentAddress::new(VALID_SHA256).unwrap();
        let result = store.retrieve(&addr);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            BlobStoreError::ContentNotFound { .. }
        ));
    }

    #[test]
    fn blob_store_get_metadata_returns_content_not_found_for_unknown() {
        let store = InMemoryBlobStore::new();
        let addr = ContentAddress::new(VALID_SHA256).unwrap();
        let result = store.get_metadata(&addr);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            BlobStoreError::ContentNotFound { .. }
        ));
    }

    #[test]
    fn blob_store_increment_ref_count_returns_error_for_unknown() {
        let store = InMemoryBlobStore::new();
        let addr = ContentAddress::new(VALID_SHA256).unwrap();
        let result = store.increment_ref_count(&addr);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            BlobStoreError::ContentNotFound { .. }
        ));
    }

    #[test]
    fn blob_store_decrement_ref_count_returns_error_for_unknown() {
        let store = InMemoryBlobStore::new();
        let addr = ContentAddress::new(VALID_SHA256).unwrap();
        let result = store.decrement_ref_count(&addr);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            BlobStoreError::ContentNotFound { .. }
        ));
    }

    #[test]
    fn blob_store_list_gc_candidates_returns_empty_when_no_blobs() {
        let store = InMemoryBlobStore::new();
        let result = store.list_gc_candidates(2000);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn blob_store_run_gc_returns_zero_when_no_blobs() {
        let store = InMemoryBlobStore::new();
        let result = store.run_gc(2000);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn blob_store_store_returns_not_implemented() {
        let store = InMemoryBlobStore::new();
        let result = store.store(b"hello");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            BlobStoreError::Storage { .. }
        ));
    }

    #[test]
    fn blob_store_retrieve_streaming_returns_not_implemented() {
        let store = InMemoryBlobStore::new();
        use tokio::io::AsyncWriteExt;
        let addr = ContentAddress::new(VALID_SHA256).unwrap();
        let result = store.retrieve_streaming(&addr, tokio::io::sink());
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            BlobStoreError::Storage { .. }
        ));
    }

    #[test]
    fn blob_store_store_streaming_returns_not_implemented() {
        let store = InMemoryBlobStore::new();
        let result = store.store_streaming(tokio::io::empty());
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            BlobStoreError::Storage { .. }
        ));
    }
}
