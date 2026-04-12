#![allow(unused_imports)]

use memmap2::Mmap;
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug)]
pub enum MmapCacheError {
    IoError(std::io::Error),
    MmapError(std::io::Error),
    RegionNotFound(String),
    InvalidRegion,
    CacheFull,
    SerializationError,
}

impl fmt::Display for MmapCacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IoError(e) => write!(f, "IO error: {e}"),
            Self::MmapError(e) => write!(f, "Mmap error: {e}"),
            Self::RegionNotFound(key) => write!(f, "region not found: {key}"),
            Self::InvalidRegion => write!(f, "invalid region"),
            Self::CacheFull => write!(f, "cache full"),
            Self::SerializationError => write!(f, "serialization error"),
        }
    }
}

impl std::error::Error for MmapCacheError {}

impl From<std::io::Error> for MmapCacheError {
    fn from(err: std::io::Error) -> Self {
        Self::IoError(err)
    }
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
}

impl MmapCache {
    /// Creates a new memory-mapped cache at the given base path.
    ///
    /// # Errors
    ///
    /// Returns `MmapCacheError::IoError` if the directory cannot be created.
    pub fn new(base_path: PathBuf, max_memory_bytes: usize) -> Result<Self, MmapCacheError> {
        std::fs::create_dir_all(&base_path)?;
        Ok(Self {
            base_path,
            max_memory_bytes,
            current_memory_bytes: 0,
            access_counter: 0,
            lru_queue: VecDeque::new(),
            entries: HashMap::new(),
            lock: parking_lot::Mutex::new(()),
        })
    }

    /// Inserts a key-value pair into the cache, evicting LRU entries if needed.
    ///
    /// # Errors
    ///
    /// Returns `MmapCacheError::IoError` on filesystem failures.
    pub fn insert(&mut self, key: &str, data: &[u8]) -> Result<(), MmapCacheError> {
        let needs_evict = {
            let _guard = self.lock.lock();
            self.current_memory_bytes + data.len() > self.max_memory_bytes
        };
        if needs_evict {
            self.evict_until_space_available(data.len())?;
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
        let (mut cache, _dir) = create_test_cache();
        cache.insert("key1", b"hello world").unwrap();
        let value = cache.get("key1").unwrap();
        assert_eq!(value, b"hello world");
    }

    #[test]
    fn get_missing_key_returns_error() {
        let (mut cache, _dir) = create_test_cache();
        let result = cache.get("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn contains_key() {
        let (mut cache, _dir) = create_test_cache();
        assert!(!cache.contains_key("key1"));
        cache.insert("key1", b"value").unwrap();
        assert!(cache.contains_key("key1"));
    }

    #[test]
    fn remove_entry() {
        let (mut cache, _dir) = create_test_cache();
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
        let (mut cache, _dir) = create_test_cache();
        cache.insert("key1", b"value").unwrap();
        let result = cache.prefetch("key1");
        assert!(result.is_ok());
    }

    #[test]
    fn read_ahead_multiple_keys() {
        let (mut cache, _dir) = create_test_cache();
        cache.insert("key1", b"value1").unwrap();
        cache.insert("key2", b"value2").unwrap();
        let result = cache.read_ahead(&["key1", "key2"]);
        assert!(result.is_ok());
    }

    #[test]
    fn clear_cache() {
        let (mut cache, _dir) = create_test_cache();
        cache.insert("key1", b"value1").unwrap();
        cache.insert("key2", b"value2").unwrap();
        assert_eq!(cache.len(), 2);
        cache.clear().unwrap();
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }

    #[test]
    fn memory_usage_tracking() {
        let (mut cache, _dir) = create_test_cache();
        assert_eq!(cache.current_memory_usage(), 0);
        cache.insert("key1", b"hello").unwrap();
        assert_eq!(cache.current_memory_usage(), 5);
        cache.insert("key2", b"world").unwrap();
        assert_eq!(cache.current_memory_usage(), 10);
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
}
