#![allow(clippy::unwrap_used, clippy::redundant_clone)]
mod key_value_tests;
mod ops_tests;
mod poll_tests;
mod record_tests;

use crate::codec::StorageError;
use crate::timer_index::Storage;
use vo_types::{InstanceId, TimerId};

pub(crate) struct MockStorage {
    data: std::collections::BTreeMap<Vec<u8>, Vec<u8>>,
    fail_on_op: Option<String>,
}

impl MockStorage {
    fn new() -> Self {
        Self {
            data: std::collections::BTreeMap::new(),
            fail_on_op: None,
        }
    }
}

impl Storage for MockStorage {
    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), StorageError> {
        if self.fail_on_op.as_deref() == Some("put") {
            return Err(StorageError::Storage);
        }
        self.data.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        if self.fail_on_op.as_deref() == Some("get") {
            return Err(StorageError::Storage);
        }
        Ok(self.data.get(key).cloned())
    }

    fn delete(&mut self, key: &[u8]) -> Result<(), StorageError> {
        if self.fail_on_op.as_deref() == Some("delete") {
            return Err(StorageError::Storage);
        }
        self.data.remove(key);
        Ok(())
    }

    fn scan(&self, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        if self.fail_on_op.as_deref() == Some("scan") {
            return Err(StorageError::Storage);
        }
        Ok(self
            .data
            .range(start.to_vec()..end.to_vec())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect())
    }
}

pub(crate) fn create_instance_id() -> InstanceId {
    InstanceId::from_bytes([1; 16])
}

pub(crate) fn create_timer_id() -> TimerId {
    TimerId::from_bytes([2; 16])
}