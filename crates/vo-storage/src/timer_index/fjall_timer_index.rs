use std::sync::Arc;

use vo_types::{InstanceId, TimerId};

use super::storage::{ScanResult, Storage, TIMER_INDEX_PARTITION};
use crate::codec::StorageError;

pub struct FjallTimerIndex {
    db: Arc<fjall::Database>,
    partition: Arc<fjall::Keyspace>,
}

impl FjallTimerIndex {
    #[must_use]
    pub fn open(db: &fjall::Database) -> Result<Self, StorageError> {
        let partition = db
            .keyspace(TIMER_INDEX_PARTITION, || fjall::KeyspaceCreateOptions::default())
            .map_err(|_| StorageError::FjallError)?;
        Ok(Self {
            db: Arc::new(db.clone()),
            partition: Arc::new(partition),
        })
    }
}

impl Storage for FjallTimerIndex {
    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), StorageError> {
        self.partition
            .insert(key, value)
            .map_err(|_| StorageError::FjallError)
    }

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        self.partition
            .get(key)
            .map(|opt| opt.map(|s| s.to_vec()))
            .map_err(|_| StorageError::FjallError)
    }

    fn delete(&mut self, key: &[u8]) -> Result<(), StorageError> {
        self.partition
            .remove(key)
            .map_err(|_| StorageError::FjallError)
    }

    fn scan(&self, start: &[u8], end: &[u8]) -> Result<ScanResult, StorageError> {
        let mut results = Vec::new();

        if start.is_empty() && end.is_empty() {
            for item in self.partition.iter() {
                let (k, v) = item.into_inner().map_err(|_| StorageError::FjallError)?;
                results.push((k.to_vec(), v.to_vec()));
            }
        } else if start.is_empty() {
            for item in self.partition.range(..=end) {
                let (k, v) = item.into_inner().map_err(|_| StorageError::FjallError)?;
                results.push((k.to_vec(), v.to_vec()));
            }
        } else if end.is_empty() {
            for item in self.partition.range(start..) {
                let (k, v) = item.into_inner().map_err(|_| StorageError::FjallError)?;
                results.push((k.to_vec(), v.to_vec()));
            }
        } else {
            for item in self.partition.range(start..=end) {
                let (k, v) = item.into_inner().map_err(|_| StorageError::FjallError)?;
                results.push((k.to_vec(), v.to_vec()));
            }
        }

        Ok(results)
    }
}