//! Filesystem-backed [`BlobStore`] implementation using content-addressed storage.
//!
//! Each blob is stored at `<base_dir>/blobs/<sha256_hex>` and its metadata
//! at `<base_dir>/meta/<sha256_hex>.json`. All writes use `sync_data` to
//! guarantee durability (bytes hit the disk before `store` returns success).
//!
//! # Invariants
//!
//! 1. File paths are deterministically derived from the blob hash (content addressing).
//! 2. Written bytes are fully synced to durable storage before success is returned.
//! 3. Written bytes are verified against their SHA-256 hash on retrieval.

mod integrity;
mod operations;

use std::path::PathBuf;

use crate::blob_store::{BlobRecord, BlobStore, BlobStoreError, ContentAddress};

#[derive(Debug)]
pub struct FsBlobStore {
    base_dir: PathBuf,
}

impl FsBlobStore {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map_or(1, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

fn block_on_sync<F, T>(f: F) -> T
where
    F: std::future::Future<Output = T>,
{
    let handle = tokio::runtime::Handle::current();
    tokio::task::block_in_place(|| handle.block_on(f))
}

impl BlobStore for FsBlobStore {
    fn store(&self, data: &[u8]) -> Result<ContentAddress, BlobStoreError> {
        block_on_sync(self.store_async(data))
    }

    fn stage_blob(&self, data: &[u8]) -> Result<ContentAddress, BlobStoreError> {
        block_on_sync(self.stage_blob_async(data))
    }

    fn store_streaming<R>(&self, reader: R) -> Result<ContentAddress, BlobStoreError>
    where
        R: tokio::io::AsyncRead + Send + Unpin + 'static,
    {
        block_on_sync(self.store_streaming_async(reader))
    }

    fn retrieve(&self, addr: &ContentAddress) -> Result<Vec<u8>, BlobStoreError> {
        block_on_sync(self.retrieve_async(addr))
    }

    fn retrieve_streaming<W>(&self, addr: &ContentAddress, writer: W) -> Result<(), BlobStoreError>
    where
        W: tokio::io::AsyncWrite + Send + Unpin + 'static,
    {
        block_on_sync(self.retrieve_streaming_async(addr, writer))
    }

    fn contains(&self, addr: &ContentAddress) -> Result<bool, BlobStoreError> {
        block_on_sync(self.contains_async(addr))
    }

    fn increment_ref_count(&self, addr: &ContentAddress) -> Result<u64, BlobStoreError> {
        block_on_sync(self.increment_ref_count_async(addr))
    }

    fn decrement_ref_count(&self, addr: &ContentAddress) -> Result<u64, BlobStoreError> {
        block_on_sync(self.decrement_ref_count_async(addr))
    }

    fn get_metadata(&self, addr: &ContentAddress) -> Result<BlobRecord, BlobStoreError> {
        block_on_sync(self.read_meta(addr))
    }

    fn list_gc_candidates(&self, now_ms: u64) -> Result<Vec<ContentAddress>, BlobStoreError> {
        block_on_sync(self.list_gc_candidates_async(now_ms))
    }

    fn run_gc(&self, now_ms: u64) -> Result<u64, BlobStoreError> {
        block_on_sync(self.run_gc_async(now_ms))
    }

    fn mark_durable(&self, addr: &ContentAddress) -> Result<(), BlobStoreError> {
        block_on_sync(self.mark_durable_async(addr))
    }

    fn publish(&self, addr: &ContentAddress) -> Result<(), BlobStoreError> {
        block_on_sync(self.publish_async(addr))
    }

    fn mark_failed(&self, addr: &ContentAddress) -> Result<(), BlobStoreError> {
        block_on_sync(self.mark_failed_async(addr))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs_store::integrity::*;
    use sha2::Digest;
    use vo_types::BlobStatus;

    fn make_temp_store() -> FsBlobStore {
        #[allow(deprecated)]
        let dir = tempfile::tempdir().expect("tempdir").into_path();
        FsBlobStore::new(dir)
    }

    #[tokio::test]
    async fn store_and_retrieve_roundtrip() {
        let store = make_temp_store();
        let data = b"hello, world!";
        let addr = store.store_async(data).await.expect("store");

        let retrieved = store.retrieve_async(&addr).await.expect("retrieve");
        assert_eq!(retrieved, data);
    }

    #[tokio::test]
    async fn store_rejects_duplicate() {
        let store = make_temp_store();
        let data = b"unique content";
        let _ = store.store_async(data).await;

        let result = store.store_async(data).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, BlobStoreError::DuplicateContent { .. }));
    }

    #[tokio::test]
    async fn retrieve_content_not_found() {
        let store = make_temp_store();
        let addr =
            ContentAddress::new("9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08")
                .unwrap();

        let result = store.retrieve_async(&addr).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            BlobStoreError::ContentNotFound { .. }
        ));
    }

    #[tokio::test]
    async fn contains_returns_true_for_existing() {
        let store = make_temp_store();
        let addr = store.store_async(b"exists").await.unwrap();
        assert!(store.contains_async(&addr).await.unwrap());
    }

    #[tokio::test]
    async fn contains_returns_true_for_nonexistent() {
        let store = make_temp_store();
        let addr =
            ContentAddress::new("9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08")
                .unwrap();
        assert!(!store.contains_async(&addr).await.unwrap());
    }

    #[tokio::test]
    async fn store_produces_correct_sha256() {
        let store = make_temp_store();
        let data = b"test";
        let addr = store.store_async(data).await.unwrap();

        let expected = sha2::Sha256::digest(data);
        let expected_hex: String = expected.iter().fold(String::new(), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        });
        assert_eq!(addr.as_str(), expected_hex);
    }

    #[tokio::test]
    async fn metadata_ref_count_increment_decrement() {
        let store = make_temp_store();
        let addr = store.store_async(b"refcount-test").await.unwrap();

        let new_count = store.increment_ref_count_async(&addr).await.unwrap();
        assert_eq!(new_count, 2);

        let meta = store.read_meta(&addr).await.unwrap();
        assert_eq!(meta.reference_count(), 2);

        let decremented = store.decrement_ref_count_async(&addr).await.unwrap();
        assert_eq!(decremented, 1);
    }

    #[tokio::test]
    async fn gc_collects_expired_zero_ref_blobs() {
        let store = make_temp_store();
        let addr = store.store_async(b"gc-me").await.unwrap();

        store.decrement_ref_count_async(&addr).await.unwrap();

        let meta = store.read_meta(&addr).await.unwrap();
        let record = BlobRecord::with_status(
            meta.content_addr().clone(),
            meta.size_bytes(),
            0,
            meta.created_at_ms(),
            Some(meta.created_at_ms()),
            meta.status(),
        );
        store.write_meta_file(&addr, &record).await.unwrap();

        let collected = store.run_gc_async(u64::MAX).await.unwrap();
        assert_eq!(collected, 1);

        let exists = store.contains_async(&addr).await.unwrap();
        assert!(!exists);
    }

    #[tokio::test]
    async fn gc_does_not_collect_active_blobs() {
        let store = make_temp_store();
        let addr = store.store_async(b"keep-me").await.unwrap();

        let collected = store.run_gc_async(u64::MAX).await.unwrap();
        assert_eq!(collected, 0);

        assert!(store.contains_async(&addr).await.unwrap());
    }

    #[tokio::test]
    async fn store_streaming_roundtrip() {
        let store = make_temp_store();
        let data = b"streaming content here";
        let cursor = std::io::Cursor::new(data.to_vec());
        let reader = tokio::io::BufReader::new(cursor);

        let addr = store.store_streaming_async(reader).await.unwrap();
        let retrieved = store.retrieve_async(&addr).await.unwrap();
        assert_eq!(retrieved, data);
    }

    #[tokio::test]
    async fn retrieve_streaming_writes_all_data() {
        let store = make_temp_store();
        let data = b"streamed out";
        let addr = store.store_async(data).await.unwrap();

        let mut output = Vec::new();
        let writer = tokio::io::BufWriter::new(&mut output);
        store.retrieve_streaming_async(&addr, writer).await.unwrap();

        assert_eq!(output, data);
    }

    #[tokio::test]
    async fn get_metadata_returns_correct_record() {
        let store = make_temp_store();
        let data = b"metadata check";
        let addr = store.store_async(data).await.unwrap();

        let meta = store.read_meta(&addr).await.unwrap();
        assert_eq!(meta.content_addr(), &addr);
        assert_eq!(meta.size_bytes(), data.len() as u64);
        assert_eq!(meta.reference_count(), 1);
        assert_eq!(meta.status(), BlobStatus::DurablyStored);
    }

    use std::fmt::Write;
}
