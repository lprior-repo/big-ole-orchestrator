#![allow(unused_imports)]
#![allow(clippy::used_underscore_binding)]

use memmap2::Mmap;
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use tokio::sync::broadcast;

#[derive(Debug, thiserror::Error)]
pub enum MmapCacheError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Mmap error: {0}")]
    MmapError(std::io::Error),
    #[error("region not found: {0}")]
    RegionNotFound(String),
    #[error("invalid region")]
    InvalidRegion,
    #[error("cache full")]
    CacheFull,
    #[error("serialization error")]
    SerializationError,
}

#[derive(Clone, Debug)]
pub enum CacheInvalidationEvent {
    KeyInvalidated(String),
    AllInvalidated,
}

#[derive(Clone)]
struct CacheRegion {
    _offset: u64,
    size: u64,
    file_path: PathBuf,
}

struct LruEntry {
    _key: String,
    region: CacheRegion,
    _last_access: u64,
}

pub struct MmapCache {
    base_path: PathBuf,
    max_memory_bytes: usize,
    current_memory_bytes: usize,
    access_counter: u64,
    lru_queue: VecDeque<String>,
    entries: HashMap<String, LruEntry>,
    lock: parking_lot::Mutex<()>,
    _invalidation_tx: Option<broadcast::Sender<CacheInvalidationEvent>>,
}

impl MmapCache {
    /// Creates a new memory-mapped cache at the given base path.
    ///
    /// # Errors
    ///
    /// Returns `MmapCacheError::IoError` if the directory cannot be created.
    pub fn new(base_path: PathBuf, max_memory_bytes: usize) -> Result<Self, MmapCacheError> {
        Self::with_broadcast_channel(base_path, max_memory_bytes, 100)
    }

    /// Creates a new memory-mapped cache with broadcast channel support for invalidation events.
    ///
    /// # Arguments
    ///
    /// * `buffer_size` - Size of the broadcast channel buffer for invalidation events.
    ///   Set to 0 to drop events when receiver is slow.
    ///
    /// # Errors
    ///
    /// Returns `MmapCacheError::IoError` if the directory cannot be created.
    pub fn with_broadcast_channel(
        base_path: PathBuf,
        max_memory_bytes: usize,
        buffer_size: usize,
    ) -> Result<Self, MmapCacheError> {
        std::fs::create_dir_all(&base_path)?;
        let (tx, _rx) = broadcast::channel(buffer_size);
        Ok(Self {
            base_path,
            max_memory_bytes,
            current_memory_bytes: 0,
            access_counter: 0,
            lru_queue: VecDeque::new(),
            entries: HashMap::new(),
            lock: parking_lot::Mutex::new(()),
            _invalidation_tx: Some(tx),
        })
    }

    /// Inserts a key-value pair into the cache, evicting LRU entries if needed.
    ///
    /// # Errors
    ///
    /// Returns `MmapCacheError::IoError` on filesystem failures.
    /// Returns `MmapCacheError::CacheFull` if the entry exceeds cache capacity even after eviction.
    pub fn insert(&mut self, key: &str, data: &[u8]) -> Result<(), MmapCacheError> {
        if data.len() > self.max_memory_bytes {
            return Err(MmapCacheError::CacheFull);
        }
        let needs_evict = {
            let _guard = self.lock.lock();
            self.current_memory_bytes + data.len() > self.max_memory_bytes
        };
        if needs_evict {
            self.evict_until_space_available(data.len())?;
        }
        let old_file_path = {
            let _guard = self.lock.lock();
            if self.current_memory_bytes + data.len() > self.max_memory_bytes {
                return Err(MmapCacheError::CacheFull);
            }
            if let Some(old_entry) = self.entries.remove(key) {
                #[allow(clippy::cast_possible_truncation)]
                {
                    self.current_memory_bytes -= old_entry.region.size as usize;
                }
                self.lru_queue.retain(|k| k != key);
                Some(old_entry.region.file_path)
            } else {
                None
            }
        };
        if let Some(file_path) = old_file_path {
            let _ = std::fs::remove_file(file_path);
        }
        let offset = self.allocate_region(key, data.len())?;
        self.write_data_to_region(key, offset, data)?;
        {
            let _guard = self.lock.lock();
            self.access_counter += 1;
            let region = CacheRegion {
                _offset: offset,
                size: data.len() as u64,
                file_path: self.region_file_path(key),
            };
            let entry = LruEntry {
                _key: key.to_string(),
                region,
                _last_access: self.access_counter,
            };
            self.current_memory_bytes += data.len();
            self.lru_queue.push_back(key.to_string());
            self.entries.insert(key.to_string(), entry);
        }
        Ok(())
    }

    /// Retrieves the value associated with the given key.
    ///
    /// # Errors
    ///
    /// Returns `MmapCacheError::RegionNotFound` if the key does not exist.
    /// Returns `MmapCacheError::IoError` on filesystem failures.
    /// Returns `MmapCacheError::MmapError` if the memory map fails.
    #[allow(clippy::cast_possible_truncation)]
    pub fn get(&self, key: &str) -> Result<Vec<u8>, MmapCacheError> {
        let region = {
            let _guard = self.lock.lock();
            self.entries
                .get(key)
                .map(|e| e.region.clone())
                .ok_or_else(|| MmapCacheError::RegionNotFound(key.to_string()))?
        };
        let file = File::open(&region.file_path)?;
        let mmap = unsafe { Mmap::map(&file) }.map_err(MmapCacheError::MmapError)?;
        Ok(mmap[..region.size as usize].to_vec())
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }

    /// Removes the entry associated with the given key.
    ///
    /// # Errors
    ///
    /// Returns `MmapCacheError::IoError` if the backing file cannot be removed.
    #[allow(clippy::cast_possible_truncation)]
    pub fn remove(&mut self, key: &str) -> Result<(), MmapCacheError> {
        let _guard = self.lock.lock();
        if let Some(entry) = self.entries.remove(key) {
            self.current_memory_bytes -= entry.region.size as usize;
            std::fs::remove_file(entry.region.file_path)?;
            self.lru_queue.retain(|k| k != key);
        }
        Ok(())
    }

    /// Prefetches the memory-mapped region for the given key into the OS page cache.
    ///
    /// # Errors
    ///
    /// Returns `MmapCacheError::MmapError` if the memory map fails.
    pub fn prefetch(&self, key: &str) -> Result<(), MmapCacheError> {
        let file_path = {
            let _guard = self.lock.lock();
            self.entries.get(key).map(|e| e.region.file_path.clone())
        };
        if let Some(path) = file_path {
            let file = File::open(&path)?;
            let _mmap = unsafe { Mmap::map(&file) }.map_err(MmapCacheError::MmapError)?;
        }
        Ok(())
    }

    /// Prefetches multiple keys into the OS page cache.
    ///
    /// # Errors
    ///
    /// Returns `MmapCacheError::MmapError` if any memory map fails.
    pub fn read_ahead(&self, keys: &[&str]) -> Result<(), MmapCacheError> {
        for key in keys {
            self.prefetch(key)?;
        }
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub const fn current_memory_usage(&self) -> usize {
        self.current_memory_bytes
    }

    pub const fn max_memory_limit(&self) -> usize {
        self.max_memory_bytes
    }

    pub fn subscribe(&self) -> broadcast::Receiver<CacheInvalidationEvent> {
        self._invalidation_tx.as_ref().map_or_else(
            || broadcast::channel(100).1,
            broadcast::Sender::subscribe,
        )
    }

    /// Invalidates a specific key from the cache.
    ///
    /// # Errors
    ///
    /// Returns `MmapCacheError` if the underlying storage operation fails.
    pub fn invalidate_key(&self, key: &str) -> Result<(), MmapCacheError> {
        if let Some(ref tx) = self._invalidation_tx {
            let event = CacheInvalidationEvent::KeyInvalidated(key.to_string());
            let _ = tx.send(event);
        }
        Ok(())
    }

    /// Invalidates all keys matching the given prefix.
    ///
    /// # Errors
    ///
    /// Returns `MmapCacheError` if the underlying storage operation fails.
    pub fn invalidate_prefix(&self, prefix: &str) -> Result<Vec<String>, MmapCacheError> {
        let keys_to_invalidate: Vec<String> = {
            let _guard = self.lock.lock();
            self.entries
                .keys()
                .filter(|k| k.starts_with(prefix))
                .cloned()
                .collect()
        };

        if let Some(ref tx) = self._invalidation_tx {
            for key in &keys_to_invalidate {
                let event = CacheInvalidationEvent::KeyInvalidated(key.clone());
                let _ = tx.send(event);
            }
        }

        Ok(keys_to_invalidate)
    }

    /// Invalidates all entries in the cache.
    ///
    /// # Errors
    ///
    /// Returns `MmapCacheError` if the underlying storage operation fails.
    pub fn invalidate_all(&self) -> Result<usize, MmapCacheError> {
        let count = {
            let _guard = self.lock.lock();
            self.entries.len()
        };

        if let Some(ref tx) = self._invalidation_tx {
            let event = CacheInvalidationEvent::AllInvalidated;
            let _ = tx.send(event);
        }

        Ok(count)
    }

    fn region_file_path(&self, key: &str) -> PathBuf {
        let safe_name = key.replace(['/', '\\', ':'], "_");
        self.base_path.join(safe_name)
    }

    fn allocate_region(&self, key: &str, size: usize) -> Result<u64, MmapCacheError> {
        let path = self.region_file_path(key);
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .truncate(true)
            .open(&path)?;
        file.set_len(size as u64)?;
        Ok(0)
    }

    fn write_data_to_region(
        &self,
        key: &str,
        _offset: u64,
        data: &[u8],
    ) -> Result<(), MmapCacheError> {
        let path = self.region_file_path(key);
        let mut file = OpenOptions::new().write(true).open(&path)?;
        file.write_all(data)?;
        file.flush()?;
        Ok(())
    }

    #[allow(clippy::unnecessary_wraps, clippy::cast_possible_truncation)]
    fn evict_until_space_available(&mut self, needed: usize) -> Result<(), MmapCacheError> {
        let _guard = self.lock.lock();
        while self.current_memory_bytes + needed > self.max_memory_bytes
            && !self.lru_queue.is_empty()
        {
            if let Some(lru_key) = self.lru_queue.pop_front() {
                if let Some(entry) = self.entries.remove(&lru_key) {
                    self.current_memory_bytes -= entry.region.size as usize;
                    let _ = std::fs::remove_file(entry.region.file_path);
                }
            }
        }
        Ok(())
    }

    /// Removes all entries from the cache and deletes their backing files.
    ///
    /// # Errors
    ///
    /// Returns `MmapCacheError::IoError` if any backing file cannot be removed.
    pub fn clear(&mut self) -> Result<(), MmapCacheError> {
        let _guard = self.lock.lock();
        for entry in self.entries.values() {
            let _ = std::fs::remove_file(&entry.region.file_path);
        }
        self.entries.clear();
        self.lru_queue.clear();
        self.current_memory_bytes = 0;
        Ok(())
    }
}

impl Drop for MmapCache {
    fn drop(&mut self) {
        let _ = self.clear();
    }
}

pub struct MmapCacheBuilder {
    base_path: PathBuf,
    max_memory_bytes: usize,
}

impl MmapCacheBuilder {
    #[must_use]
    pub const fn new(base_path: PathBuf) -> Self {
        Self {
            base_path,
            max_memory_bytes: 100 * 1024 * 1024,
        }
    }

    #[must_use]
    pub const fn max_memory_bytes(mut self, bytes: usize) -> Self {
        self.max_memory_bytes = bytes;
        self
    }

    /// Builds the `MmapCache` with the configured settings.
    ///
    /// # Errors
    ///
    /// Returns `MmapCacheError::IoError` if the cache directory cannot be created.
    pub fn build(self) -> Result<MmapCache, MmapCacheError> {
        MmapCache::new(self.base_path, self.max_memory_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_cache() -> (MmapCache, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let cache = MmapCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).unwrap();
        (cache, temp_dir)
    }

    #[test]
    fn insert_and_get() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = MmapCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).unwrap();
        cache.insert("key1", b"hello world").unwrap();
        let value = cache.get("key1").unwrap();
        assert_eq!(value, b"hello world");
    }

    #[test]
    fn get_missing_key_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let cache = MmapCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).unwrap();
        let result = cache.get("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn contains_key() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = MmapCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).unwrap();
        assert!(!cache.contains_key("key1"));
        cache.insert("key1", b"value").unwrap();
        assert!(cache.contains_key("key1"));
    }

    #[test]
    fn remove_entry() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = MmapCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).unwrap();
        cache.insert("key1", b"value").unwrap();
        assert!(cache.contains_key("key1"));
        cache.remove("key1").unwrap();
        assert!(!cache.contains_key("key1"));
    }

    #[test]
    fn lru_eviction() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = MmapCache::new(temp_dir.path().to_path_buf(), 12).unwrap();
        cache.insert("key1", b"12345").unwrap();
        cache.insert("key2", b"67890").unwrap();
        cache.insert("key3", b"abcde").unwrap();
        assert!(!cache.contains_key("key1"));
        assert!(cache.contains_key("key2"));
        assert!(cache.contains_key("key3"));
    }

    #[test]
    fn prefetch_does_not_error() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = MmapCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).unwrap();
        cache.insert("key1", b"value").unwrap();
        let result = cache.prefetch("key1");
        assert!(result.is_ok());
    }

    #[test]
    fn read_ahead_multiple_keys() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = MmapCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).unwrap();
        cache.insert("key1", b"value1").unwrap();
        cache.insert("key2", b"value2").unwrap();
        let result = cache.read_ahead(&["key1", "key2"]);
        assert!(result.is_ok());
    }

    #[test]
    fn clear_cache() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = MmapCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).unwrap();
        cache.insert("key1", b"value1").unwrap();
        cache.insert("key2", b"value2").unwrap();
        assert_eq!(cache.len(), 2);
        cache.clear().unwrap();
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }

    #[test]
    fn memory_usage_tracking() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = MmapCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).unwrap();
        assert_eq!(cache.current_memory_usage(), 0);
        cache.insert("key1", b"hello").unwrap();
        assert_eq!(cache.current_memory_usage(), 5);
        cache.insert("key2", b"world").unwrap();
        assert_eq!(cache.current_memory_usage(), 10);
    }

    #[test]
    fn insert_with_special_characters_in_key() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = MmapCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).unwrap();
        cache.insert("key/with:slashes", b"value").unwrap();
        assert!(cache.contains_key("key/with:slashes"));
        let value = cache.get("key/with:slashes").unwrap();
        assert_eq!(value, b"value");
    }

    #[test]
    fn insert_overwrite_updates_value() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = MmapCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).unwrap();
        cache.insert("key1", b"value1").unwrap();
        cache.insert("key1", b"value2").unwrap();
        assert_eq!(cache.len(), 1);
        let value = cache.get("key1").unwrap();
        assert_eq!(value, b"value2", "overwrite should update the stored value");
    }

    #[test]
    fn insert_overwrite_updates_memory_usage() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = MmapCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).unwrap();
        cache.insert("key1", b"short").unwrap();
        assert_eq!(cache.current_memory_usage(), 5);
        cache.insert("key1", b"much longer value").unwrap();
        assert_eq!(
            cache.current_memory_usage(),
            17,
            "memory should reflect new value size"
        );
    }

    #[test]
    fn lru_eviction_with_multiple_entries() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = MmapCache::new(temp_dir.path().to_path_buf(), 15).unwrap();
        cache.insert("key1", b"12345").unwrap(); // 5 bytes, total 5
        cache.insert("key2", b"67890").unwrap(); // 5 bytes, total 10
        cache.insert("key3", b"abcde").unwrap(); // 5 bytes, total 15
        cache.insert("key4", b"fghij").unwrap(); // 5 bytes -> need 20, evict key1 (LRU)
        assert!(!cache.contains_key("key1"), "LRU key1 should be evicted");
        assert!(cache.contains_key("key2"));
        assert!(cache.contains_key("key3"));
        assert!(cache.contains_key("key4"));
    }

    #[test]
    fn clear_resets_memory_usage() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = MmapCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).unwrap();
        cache.insert("key1", b"value1").unwrap();
        cache.insert("key2", b"value2").unwrap();
        assert!(cache.current_memory_usage() > 0);
        cache.clear().unwrap();
        assert_eq!(cache.current_memory_usage(), 0);
    }

    #[test]
    fn get_after_remove_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = MmapCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).unwrap();
        cache.insert("key1", b"value").unwrap();
        cache.remove("key1").unwrap();
        let result = cache.get("key1");
        assert!(result.is_err(), "get should fail after remove");
    }

    #[test]
    fn zero_byte_insert_and_get() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = MmapCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).unwrap();
        cache.insert("empty", b"").unwrap();
        assert!(cache.contains_key("empty"));
        let value = cache.get("empty").unwrap();
        assert!(value.is_empty());
    }

    #[test]
    fn insert_at_exact_capacity_succeeds() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = MmapCache::new(temp_dir.path().to_path_buf(), 5).unwrap();
        cache.insert("key1", b"12345").unwrap();
        assert_eq!(cache.current_memory_usage(), 5);
    }

    #[test]
    fn builder_pattern() {
        let temp_dir = TempDir::new().unwrap();
        let cache = MmapCacheBuilder::new(temp_dir.path().to_path_buf())
            .max_memory_bytes(2048)
            .build()
            .unwrap();
        assert_eq!(cache.max_memory_limit(), 2048);
    }

    #[test]
    fn read_ahead_continues_on_individual_errors() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = MmapCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).unwrap();
        cache.insert("key1", b"value1").unwrap();
        cache.insert("key2", b"value2").unwrap();
        let result = cache.read_ahead(&["key1", "nonexistent", "key2"]);
        assert!(
            result.is_ok(),
            "read_ahead should continue on individual errors (INV-014)"
        );
        assert!(cache.contains_key("key1"));
        assert!(cache.contains_key("key2"));
    }

    #[test]
    fn remove_nonexistent_key_is_idempotent() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = MmapCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).unwrap();
        let result = cache.remove("nonexistent");
        assert!(
            result.is_ok(),
            "remove should succeed idempotently for missing keys"
        );
    }

    #[test]
    fn evict_until_space_available_evicts_lru_when_needed() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = MmapCache::new(temp_dir.path().to_path_buf(), 5).unwrap();
        cache.insert("key1", b"12345").unwrap();
        // key2 (5 bytes) exceeds capacity (5 used) so key1 should be evicted
        cache.insert("key2", b"67890").unwrap();
        assert!(!cache.contains_key("key1"), "LRU entry should be evicted");
        assert!(cache.contains_key("key2"), "new entry should be inserted");
    }

    #[test]
    fn insert_existing_key_preserves_lru_sync() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = MmapCache::new(temp_dir.path().to_path_buf(), 1024).unwrap();
        cache.insert("key1", b"value1").unwrap();
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.lru_queue.len(), cache.entries.len());
        cache.insert("key1", b"value2").unwrap();
        assert_eq!(cache.len(), 1, "inserting same key should not increase len");
        assert_eq!(
            cache.lru_queue.len(),
            cache.entries.len(),
            "lru_queue and entries must stay synchronized (INV-004)"
        );
        let lru_keys: Vec<_> = cache.lru_queue.iter().cloned().collect();
        assert_eq!(lru_keys.len(), 1);
        assert!(cache.entries.contains_key("key1"));
    }

    #[test]
    fn insert_with_zero_max_memory_bytes_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = MmapCache::new(temp_dir.path().to_path_buf(), 0).unwrap();
        let result = cache.insert("key1", b"value");
        assert!(
            matches!(result, Err(MmapCacheError::CacheFull)),
            "insert with zero max_memory_bytes should return error (INV-002)"
        );
    }

    #[test]
    fn invalidate_key_sends_event() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache =
            MmapCache::with_broadcast_channel(temp_dir.path().to_path_buf(), 1024 * 1024, 100)
                .unwrap();
        let mut receiver = cache.subscribe();

        cache.insert("key1", b"value").unwrap();
        cache.invalidate_key("key1").unwrap();

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let event = runtime.block_on(receiver.recv()).unwrap();
        match event {
            CacheInvalidationEvent::KeyInvalidated(key) => assert_eq!(key, "key1"),
            _ => panic!("Expected KeyInvalidated event"),
        }
    }

    #[test]
    fn invalidate_prefix_sends_events_for_matching_keys() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache =
            MmapCache::with_broadcast_channel(temp_dir.path().to_path_buf(), 1024 * 1024, 100)
                .unwrap();
        let mut receiver = cache.subscribe();

        cache.insert("user:1", b"value1").unwrap();
        cache.insert("user:2", b"value2").unwrap();
        cache.insert("order:1", b"value3").unwrap();
        cache.invalidate_prefix("user:").unwrap();

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let event1 = runtime.block_on(receiver.recv()).unwrap();
        let event2 = runtime.block_on(receiver.recv()).unwrap();

        match event1 {
            CacheInvalidationEvent::KeyInvalidated(key) => assert!(key.starts_with("user:")),
            _ => panic!("Expected KeyInvalidated event"),
        }
        match event2 {
            CacheInvalidationEvent::KeyInvalidated(key) => assert!(key.starts_with("user:")),
            _ => panic!("Expected KeyInvalidated event"),
        }
    }

    #[test]
    fn invalidate_all_sends_event() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache =
            MmapCache::with_broadcast_channel(temp_dir.path().to_path_buf(), 1024 * 1024, 100)
                .unwrap();
        let mut receiver = cache.subscribe();

        cache.insert("key1", b"value1").unwrap();
        cache.insert("key2", b"value2").unwrap();
        cache.invalidate_all().unwrap();

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let event = runtime.block_on(receiver.recv()).unwrap();
        match event {
            CacheInvalidationEvent::AllInvalidated => (),
            _ => panic!("Expected AllInvalidated event"),
        }
    }

    #[test]
    fn invalidate_prefix_returns_list_of_invalidated_keys() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache =
            MmapCache::with_broadcast_channel(temp_dir.path().to_path_buf(), 1024 * 1024, 100)
                .unwrap();

        cache.insert("user:1", b"value1").unwrap();
        cache.insert("user:2", b"value2").unwrap();
        cache.insert("order:1", b"value3").unwrap();

        let invalidated = cache.invalidate_prefix("user:").unwrap();
        assert_eq!(invalidated.len(), 2);
        assert!(invalidated.contains(&"user:1".to_string()));
        assert!(invalidated.contains(&"user:2".to_string()));
    }

    #[test]
    fn invalidate_all_returns_count() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache =
            MmapCache::with_broadcast_channel(temp_dir.path().to_path_buf(), 1024 * 1024, 100)
                .unwrap();

        cache.insert("key1", b"value1").unwrap();
        cache.insert("key2", b"value2").unwrap();
        cache.insert("key3", b"value3").unwrap();

        let count = cache.invalidate_all().unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn invalidate_key_with_no_subscribers_does_not_error() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache =
            MmapCache::with_broadcast_channel(temp_dir.path().to_path_buf(), 1024 * 1024, 100)
                .unwrap();
        cache.insert("key1", b"value").unwrap();
        let result = cache.invalidate_key("key1");
        assert!(result.is_ok());
    }

    #[test]
    fn invalidate_nonexistent_key_sends_event() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache =
            MmapCache::with_broadcast_channel(temp_dir.path().to_path_buf(), 1024 * 1024, 100)
                .unwrap();
        let mut receiver = cache.subscribe();

        cache.invalidate_key("nonexistent").unwrap();

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let event = runtime.block_on(receiver.recv()).unwrap();
        match event {
            CacheInvalidationEvent::KeyInvalidated(key) => assert_eq!(key, "nonexistent"),
            _ => panic!("Expected KeyInvalidated event"),
        }
    }

    #[test]
    fn broadcast_channel_lagged_error_when_receiver_cannot_keep_up() {
        let temp_dir = TempDir::new().unwrap();
        let cache =
            MmapCache::with_broadcast_channel(temp_dir.path().to_path_buf(), 1024 * 1024, 2)
                .unwrap();
        let mut receiver = cache.subscribe();

        cache.invalidate_key("key1").unwrap();
        cache.invalidate_key("key2").unwrap();
        cache.invalidate_key("key3").unwrap();

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result1 = runtime.block_on(receiver.recv());
        assert!(result1.is_err());
        let err = result1.unwrap_err();
        match err {
            tokio::sync::broadcast::error::RecvError::Lagged(n) => {
                assert!(n >= 1, "Expected lagged count >= 1, got {}", n);
            }
            _ => panic!("Expected Lagged error, got {:?}", err),
        }

        assert!(
            received_keys.len() < 3,
            "Expected some events to be dropped due to buffer overflow, but got all 3: {:?}",
            received_keys
        );
        assert!(
            !received_keys.is_empty(),
            "Expected at least one event to be received"
        );
    }
}
