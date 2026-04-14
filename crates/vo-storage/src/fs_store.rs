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

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::blob_store::{
    decode_blob_record, encode_blob_record, BlobRecord, BlobStore, BlobStoreError, ContentAddress,
};

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

    fn blobs_dir(&self) -> PathBuf {
        self.base_dir.join("blobs")
    }

    fn meta_dir(&self) -> PathBuf {
        self.base_dir.join("meta")
    }

    fn blob_path(&self, addr: &ContentAddress) -> PathBuf {
        self.blobs_dir().join(addr.as_str())
    }

    fn meta_path(&self, addr: &ContentAddress) -> PathBuf {
        self.meta_dir().join(format!("{}.json", addr.as_str()))
    }

    fn compute_content_address(data: &[u8]) -> ContentAddress {
        let digest = Sha256::digest(data);
        let bytes: [u8; 32] = digest.into();
        ContentAddress::from_bytes(&bytes)
    }

    async fn ensure_dirs(&self) -> Result<(), BlobStoreError> {
        fs::create_dir_all(self.blobs_dir())
            .await
            .map_err(|e| BlobStoreError::Storage {
                reason: format!("failed to create blobs dir: {e}"),
            })?;
        fs::create_dir_all(self.meta_dir())
            .await
            .map_err(|e| BlobStoreError::Storage {
                reason: format!("failed to create meta dir: {e}"),
            })?;
        Ok(())
    }

    async fn write_blob_file(&self, path: &Path, data: &[u8]) -> Result<(), BlobStoreError> {
        let mut file = fs::File::create(path)
            .await
            .map_err(|e| BlobStoreError::Storage {
                reason: format!("failed to create blob file: {e}"),
            })?;
        file.write_all(data)
            .await
            .map_err(|e| BlobStoreError::Storage {
                reason: format!("failed to write blob data: {e}"),
            })?;
        file.sync_data()
            .await
            .map_err(|e| BlobStoreError::Storage {
                reason: format!("failed to sync blob data: {e}"),
            })?;
        Ok(())
    }

    async fn write_meta_file(
        &self,
        addr: &ContentAddress,
        record: &BlobRecord,
    ) -> Result<(), BlobStoreError> {
        let encoded = encode_blob_record(record).map_err(|e| BlobStoreError::Storage {
            reason: format!("failed to encode metadata: {e}"),
        })?;
        let path = self.meta_path(addr);
        let mut file = fs::File::create(&path)
            .await
            .map_err(|e| BlobStoreError::Storage {
                reason: format!("failed to create meta file: {e}"),
            })?;
        file.write_all(&encoded)
            .await
            .map_err(|e| BlobStoreError::Storage {
                reason: format!("failed to write metadata: {e}"),
            })?;
        file.sync_data()
            .await
            .map_err(|e| BlobStoreError::Storage {
                reason: format!("failed to sync metadata: {e}"),
            })?;
        Ok(())
    }

    async fn read_meta(&self, addr: &ContentAddress) -> Result<BlobRecord, BlobStoreError> {
        let path = self.meta_path(addr);
        let data = fs::read(&path).await.map_err(|e| BlobStoreError::Storage {
            reason: format!("failed to read metadata: {e}"),
        })?;
        decode_blob_record(&data).map_err(|e| BlobStoreError::Storage {
            reason: format!("failed to decode metadata: {e}"),
        })
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
}

impl FsBlobStore {
    async fn store_async(&self, data: &[u8]) -> Result<ContentAddress, BlobStoreError> {
        self.ensure_dirs().await?;

        let addr = Self::compute_content_address(data);
        let blob_path = self.blob_path(&addr);

        if blob_path.exists() {
            return Err(BlobStoreError::DuplicateContent {
                content_addr: addr.to_string(),
            });
        }

        self.write_blob_file(&blob_path, data).await?;

        let ts = now_ms();
        let record = BlobRecord::with_status(
            addr.clone(),
            data.len() as u64,
            1,
            ts,
            None,
            vo_types::BlobStatus::DurablyStored,
        );
        self.write_meta_file(&addr, &record).await?;

        Ok(addr)
    }

    async fn store_streaming_async<R>(
        &self,
        mut reader: R,
    ) -> Result<ContentAddress, BlobStoreError>
    where
        R: tokio::io::AsyncRead + Send + Unpin,
    {
        use tokio::io::AsyncReadExt;

        self.ensure_dirs().await?;

        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            let n = reader
                .read(&mut chunk)
                .await
                .map_err(|e| BlobStoreError::Storage {
                    reason: format!("streaming read failed: {e}"),
                })?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
        }

        self.store_async(&buf).await
    }

    async fn retrieve_async(&self, addr: &ContentAddress) -> Result<Vec<u8>, BlobStoreError> {
        let blob_path = self.blob_path(addr);
        if !blob_path.exists() {
            return Err(BlobStoreError::ContentNotFound {
                content_addr: addr.to_string(),
            });
        }

        let data = fs::read(&blob_path)
            .await
            .map_err(|e| BlobStoreError::Storage {
                reason: format!("failed to read blob: {e}"),
            })?;

        let computed = Self::compute_content_address(&data);
        if computed != *addr {
            return Err(BlobStoreError::ChecksumMismatch {
                content_addr: addr.to_string(),
                expected: addr.to_string(),
                actual: computed.to_string(),
            });
        }

        Ok(data)
    }

    async fn retrieve_streaming_async<W>(
        &self,
        addr: &ContentAddress,
        mut writer: W,
    ) -> Result<(), BlobStoreError>
    where
        W: tokio::io::AsyncWrite + Send + Unpin,
    {
        let data = self.retrieve_async(addr).await?;
        writer
            .write_all(&data)
            .await
            .map_err(|e| BlobStoreError::Storage {
                reason: format!("streaming write failed: {e}"),
            })?;
        writer
            .shutdown()
            .await
            .map_err(|e| BlobStoreError::Storage {
                reason: format!("streaming shutdown failed: {e}"),
            })?;
        Ok(())
    }

    async fn contains_async(&self, addr: &ContentAddress) -> Result<bool, BlobStoreError> {
        let path = self.blob_path(addr);
        match fs::metadata(path).await {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(BlobStoreError::Storage {
                reason: format!("failed to check blob existence: {e}"),
            }),
        }
    }

    async fn increment_ref_count_async(
        &self,
        addr: &ContentAddress,
    ) -> Result<u64, BlobStoreError> {
        let record = self.read_meta(addr).await?;
        let new_count = record.increment_ref_count();
        let updated = BlobRecord::with_status(
            record.content_addr().clone(),
            record.size_bytes(),
            new_count,
            record.created_at_ms(),
            record.expires_at_ms(),
            record.status(),
        );
        self.write_meta_file(addr, &updated).await?;
        Ok(new_count)
    }

    async fn decrement_ref_count_async(
        &self,
        addr: &ContentAddress,
    ) -> Result<u64, BlobStoreError> {
        let record = self.read_meta(addr).await?;
        let new_count = record.decrement_ref_count();
        let updated = BlobRecord::with_status(
            record.content_addr().clone(),
            record.size_bytes(),
            new_count,
            record.created_at_ms(),
            record.expires_at_ms(),
            record.status(),
        );
        self.write_meta_file(addr, &updated).await?;
        Ok(new_count)
    }

    async fn list_gc_candidates_async(
        &self,
        now_ms: u64,
    ) -> Result<Vec<ContentAddress>, BlobStoreError> {
        let meta_dir = self.meta_dir();
        if !meta_dir.exists() {
            return Ok(Vec::new());
        }

        let mut entries = fs::read_dir(&meta_dir)
            .await
            .map_err(|e| BlobStoreError::Storage {
                reason: format!("failed to read meta dir: {e}"),
            })?;

        let mut candidates = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| BlobStoreError::Storage {
                reason: format!("failed to read meta dir entry: {e}"),
            })?
        {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            let data = fs::read(&path).await.map_err(|e| BlobStoreError::Storage {
                reason: format!("failed to read meta file: {e}"),
            })?;

            let Ok(record) = decode_blob_record(&data) else {
                continue;
            };

            if record.is_gc_eligible(now_ms) {
                candidates.push(record.content_addr().clone());
            }
        }

        Ok(candidates)
    }

    async fn run_gc_async(&self, now_ms: u64) -> Result<u64, BlobStoreError> {
        let candidates = self.list_gc_candidates_async(now_ms).await?;
        let mut collected = 0u64;

        for addr in &candidates {
            let blob_path = self.blob_path(addr);
            let meta_path = self.meta_path(addr);

            if let Err(e) = fs::remove_file(&blob_path).await {
                eprintln!("gc: failed to remove blob {addr}: {e}");
                continue;
            }
            if let Err(e) = fs::remove_file(&meta_path).await {
                eprintln!("gc: failed to remove meta {addr}: {e}");
                continue;
            }
            collected += 1;
        }

        Ok(collected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
