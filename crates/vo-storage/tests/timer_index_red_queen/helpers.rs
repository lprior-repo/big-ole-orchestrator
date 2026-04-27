use vo_storage::codec::StorageError;
use vo_storage::timer_index::{
    scan_all_timers_for_instance, scan_due_timers, timer_delete, timer_set, TimerKey, TimerRecord,
    TimerValue,
};
use vo_types::{InstanceId, TimerId};

pub fn make_test_instance_id(byte_fill: u8) -> InstanceId {
    InstanceId::from_bytes([byte_fill; 16])
}

pub fn make_test_timer_id(byte_fill: u8) -> TimerId {
    TimerId::from_bytes([byte_fill; 16])
}

pub struct MockStorage {
    data: std::collections::BTreeMap<Vec<u8>, Vec<u8>>,
    fail_on_op: Option<String>,
}

impl MockStorage {
    pub fn new() -> Self {
        Self {
            data: std::collections::BTreeMap::new(),
            fail_on_op: None,
        }
    }

    pub fn with_fail(op: &str) -> Self {
        let mut s = Self::new();
        s.fail_on_op = Some(op.to_string());
        s
    }
}

impl vo_storage::timer_index::Storage for MockStorage {
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