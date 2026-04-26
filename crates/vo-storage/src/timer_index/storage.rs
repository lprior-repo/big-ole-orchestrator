use super::ScanResult;
use crate::codec::StorageError;

pub trait Storage {
    /// Stores a key-value pair.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if the underlying storage fails.
    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), StorageError>;
    /// Retrieves a value by key.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if the underlying storage fails.
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError>;
    /// Deletes a key.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if the underlying storage operation fails.
    fn delete(&mut self, key: &[u8]) -> Result<(), StorageError>;
    /// Scans a range of keys.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if the underlying storage fails.
    fn scan(&self, start: &[u8], end: &[u8]) -> Result<ScanResult, StorageError>;
}
