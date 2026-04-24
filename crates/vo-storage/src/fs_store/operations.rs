use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::blob_store::{decode_blob_record, BlobRecord, BlobStoreError, ContentAddress};

use super::{now_ms, FsBlobStore};

impl FsBlobStore {
    pub(super) async fn store_async(&self, data: &[u8]) -> Result<ContentAddress, BlobStoreError> {
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

    pub(super) async fn store_streaming_async<R>(
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

    pub(super) async fn stage_blob_async(
        &self,
        data: &[u8],
    ) -> Result<ContentAddress, BlobStoreError> {
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
            vo_types::BlobStatus::Pending,
        );
        self.write_meta_file(&addr, &record).await?;

        Ok(addr)
    }

    pub(super) async fn mark_durable_async(
        &self,
        addr: &ContentAddress,
    ) -> Result<(), BlobStoreError> {
        let record = self.read_meta(addr).await?;

        if !record.can_transition_to(vo_types::BlobStatus::DurablyStored) {
            return Err(BlobStoreError::InvalidPublicationStatus {
                content_addr: addr.to_string(),
                current_status: format!("{:?}", record.status()),
                attempted_operation: "mark_durable".to_string(),
            });
        }

        let updated = BlobRecord::with_status(
            record.content_addr().clone(),
            record.size_bytes(),
            record.reference_count(),
            record.created_at_ms(),
            record.expires_at_ms(),
            vo_types::BlobStatus::DurablyStored,
        );
        self.write_meta_file(addr, &updated).await?;
        Ok(())
    }

    pub(super) async fn publish_async(&self, addr: &ContentAddress) -> Result<(), BlobStoreError> {
        let record = self.read_meta(addr).await?;

        if !record.can_transition_to(vo_types::BlobStatus::Published) {
            return Err(BlobStoreError::InvalidPublicationStatus {
                content_addr: addr.to_string(),
                current_status: format!("{:?}", record.status()),
                attempted_operation: "publish".to_string(),
            });
        }

        let updated = BlobRecord::with_status(
            record.content_addr().clone(),
            record.size_bytes(),
            record.reference_count(),
            record.created_at_ms(),
            record.expires_at_ms(),
            vo_types::BlobStatus::Published,
        );
        self.write_meta_file(addr, &updated).await?;
        Ok(())
    }

    pub(super) async fn mark_failed_async(
        &self,
        addr: &ContentAddress,
    ) -> Result<(), BlobStoreError> {
        let record = self.read_meta(addr).await?;

        if !record.can_transition_to(vo_types::BlobStatus::Failed) {
            return Err(BlobStoreError::InvalidPublicationStatus {
                content_addr: addr.to_string(),
                current_status: format!("{:?}", record.status()),
                attempted_operation: "mark_failed".to_string(),
            });
        }

        let updated = BlobRecord::with_status(
            record.content_addr().clone(),
            record.size_bytes(),
            record.reference_count(),
            record.created_at_ms(),
            record.expires_at_ms(),
            vo_types::BlobStatus::Failed,
        );
        self.write_meta_file(addr, &updated).await?;
        Ok(())
    }

    pub(super) async fn retrieve_async(
        &self,
        addr: &ContentAddress,
    ) -> Result<Vec<u8>, BlobStoreError> {
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

    pub(super) async fn retrieve_streaming_async<W>(
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

    pub(super) async fn contains_async(
        &self,
        addr: &ContentAddress,
    ) -> Result<bool, BlobStoreError> {
        let path = self.blob_path(addr);
        match fs::metadata(path).await {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(BlobStoreError::Storage {
                reason: format!("failed to check blob existence: {e}"),
            }),
        }
    }

    pub(super) async fn increment_ref_count_async(
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

    pub(super) async fn decrement_ref_count_async(
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

    pub(super) async fn list_gc_candidates_async(
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

    pub(super) async fn run_gc_async(&self, now_ms: u64) -> Result<u64, BlobStoreError> {
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
