use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::blob_store::{
    decode_blob_record, encode_blob_record, BlobRecord, BlobStoreError, ContentAddress,
};

use super::FsBlobStore;

impl FsBlobStore {
    pub(super) fn blobs_dir(&self) -> PathBuf {
        self.base_dir.join("blobs")
    }

    pub(super) fn meta_dir(&self) -> PathBuf {
        self.base_dir.join("meta")
    }

    pub(super) fn blob_path(&self, addr: &ContentAddress) -> PathBuf {
        self.blobs_dir().join(addr.as_str())
    }

    pub(super) fn meta_path(&self, addr: &ContentAddress) -> PathBuf {
        self.meta_dir().join(format!("{}.json", addr.as_str()))
    }

    pub(super) fn compute_content_address(data: &[u8]) -> ContentAddress {
        let digest = Sha256::digest(data);
        let bytes: [u8; 32] = digest.into();
        ContentAddress::from_bytes(&bytes)
    }

    pub(super) async fn ensure_dirs(&self) -> Result<(), BlobStoreError> {
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

    pub(super) async fn write_blob_file(
        &self,
        path: &Path,
        data: &[u8],
    ) -> Result<(), BlobStoreError> {
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

    pub(super) async fn write_meta_file(
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

    pub(super) async fn read_meta(
        &self,
        addr: &ContentAddress,
    ) -> Result<BlobRecord, BlobStoreError> {
        let path = self.meta_path(addr);
        let data = fs::read(&path).await.map_err(|e| BlobStoreError::Storage {
            reason: format!("failed to read metadata: {e}"),
        })?;
        decode_blob_record(&data).map_err(|e| BlobStoreError::Storage {
            reason: format!("failed to decode metadata: {e}"),
        })
    }
}
