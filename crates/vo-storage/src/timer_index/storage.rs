use std::sync::Arc;

use crate::codec::StorageError;

use super::ScanResult;

pub const TIMER_INDEX_PARTITION: &str = "timer_index";

pub trait Storage {
    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), StorageError>;
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError>;
    fn delete(&mut self, key: &[u8]) -> Result<(), StorageError>;
    fn scan(&self, start: &[u8], end: &[u8]) -> Result<ScanResult, StorageError>;
}

pub struct FjallTimerIndexStore {
    partition: Arc<fjall::Keyspace>,
}

impl FjallTimerIndexStore {
    pub fn open(db: &fjall::Database) -> Result<Self, StorageError> {
        let partition = db
            .keyspace(
                TIMER_INDEX_PARTITION,
                fjall::KeyspaceCreateOptions::default,
            )
            .map_err(|e| StorageError::Storage)?;
        Ok(Self {
            partition: Arc::new(partition),
        })
    }
}

impl Storage for FjallTimerIndexStore {
    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), StorageError> {
        self.partition
            .insert(key, value)
            .map_err(|_| StorageError::Storage)?;
        Ok(())
    }

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        match self.partition.get(key) {
            Ok(Some(value)) => Ok(Some(value.to_vec())),
            Ok(None) => Ok(None),
            Err(_) => Err(StorageError::Storage),
        }
    }

    fn delete(&mut self, key: &[u8]) -> Result<(), StorageError> {
        self.partition
            .remove(key)
            .map_err(|_| StorageError::Storage)?;
        Ok(())
    }

    fn scan(&self, start: &[u8], end: &[u8]) -> Result<ScanResult, StorageError> {
        let mut results = ScanResult::new();
        let iter = self.partition.iter();
        for item in iter {
            let (key_bytes, value_bytes) =
                item.into_inner()
                    .map_err(|_| StorageError::Storage)?;
            let key_ref = key_bytes.as_ref();
            if key_ref >= start && key_ref < end {
                results.push((key_bytes.to_vec(), value_bytes.to_vec()));
            }
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fjall_timer_index_store_put_and_get() {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        let mut store = FjallTimerIndexStore::open(&db).unwrap();

        let key = b"test_key";
        let value = b"test_value";

        store.put(key, value).unwrap();

        let retrieved = store.get(key).unwrap().unwrap();
        assert_eq!(retrieved, value);
    }

    #[test]
    fn fjall_timer_index_store_get_missing() {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        let store = FjallTimerIndexStore::open(&db).unwrap();

        let result = store.get(b"nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn fjall_timer_index_store_delete() {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        let mut store = FjallTimerIndexStore::open(&db).unwrap();

        let key = b"delete_key";
        let value = b"delete_value";

        store.put(key, value).unwrap();
        assert!(store.get(key).unwrap().is_some());

        store.delete(key).unwrap();
        assert!(store.get(key).unwrap().is_none());
    }

    #[test]
    fn fjall_timer_index_store_scan() {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        let mut store = FjallTimerIndexStore::open(&db).unwrap();

        store.put(b"a_1", b"value1").unwrap();
        store.put(b"a_2", b"value2").unwrap();
        store.put(b"a_3", b"value3").unwrap();
        store.put(b"b_1", b"value4").unwrap();

        let results = store.scan(b"a_1", b"a_3").unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(&results[0].0, b"a_1");
        assert_eq!(&results[1].0, b"a_2");
    }

    #[test]
    fn fjall_timer_index_store_persists_across_restart() {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        let mut store = FjallTimerIndexStore::open(&db).unwrap();

        store.put(b"persist_key", b"persist_value").unwrap();
        drop(store);
        drop(db);

        let db2 = fjall::Database::builder(dir.path()).open().unwrap();
        let store2 = FjallTimerIndexStore::open(&db2).unwrap();

        let retrieved = store2.get(b"persist_key").unwrap().unwrap();
        assert_eq!(retrieved, b"persist_value");
    }
}
