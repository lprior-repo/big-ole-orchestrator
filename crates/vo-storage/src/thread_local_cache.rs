use std::path::PathBuf;

use crate::mmap_cache::{MmapCache, MmapCacheError};

thread_local! {
    static THREAD_CACHE: std::cell::RefCell<Option<MmapCache>> = const { std::cell::RefCell::new(None) };
}

#[derive(Debug)]
pub enum ThreadLocalCacheError {
    CacheNotInitialized,
    MmapCacheError(MmapCacheError),
}

impl std::fmt::Display for ThreadLocalCacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CacheNotInitialized => write!(f, "thread-local cache not initialized"),
            Self::MmapCacheError(e) => write!(f, "mmap cache error: {e}"),
        }
    }
}

impl std::error::Error for ThreadLocalCacheError {}

impl From<MmapCacheError> for ThreadLocalCacheError {
    fn from(err: MmapCacheError) -> Self {
        Self::MmapCacheError(err)
    }
}

pub struct ThreadLocalCache;

impl ThreadLocalCache {
    pub fn new(base_path: PathBuf, max_memory_bytes: usize) -> Result<(), ThreadLocalCacheError> {
        THREAD_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            if cache.is_some() {
                return Ok(());
            }
            let mmap_cache = MmapCache::new(base_path, max_memory_bytes)?;
            *cache = Some(mmap_cache);
            Ok(())
        })
    }

    pub fn insert(key: &str, data: &[u8]) -> Result<(), ThreadLocalCacheError> {
        THREAD_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            let mmap_cache = cache
                .as_mut()
                .ok_or(ThreadLocalCacheError::CacheNotInitialized)?;
            mmap_cache.insert(key, data)?;
            Ok(())
        })
    }

    pub fn get(key: &str) -> Result<Vec<u8>, ThreadLocalCacheError> {
        THREAD_CACHE.with(|cache| {
            let cache = cache.borrow();
            let mmap_cache = cache
                .as_ref()
                .ok_or(ThreadLocalCacheError::CacheNotInitialized)?;
            mmap_cache.get(key).map_err(ThreadLocalCacheError::from)
        })
    }

    pub fn contains_key(key: &str) -> bool {
        THREAD_CACHE.with(|cache| {
            let cache = cache.borrow();
            match cache.as_ref() {
                Some(mmap_cache) => mmap_cache.contains_key(key),
                None => false,
            }
        })
    }

    pub fn remove(key: &str) -> Result<(), ThreadLocalCacheError> {
        THREAD_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            let mmap_cache = cache
                .as_mut()
                .ok_or(ThreadLocalCacheError::CacheNotInitialized)?;
            mmap_cache.remove(key)?;
            Ok(())
        })
    }

    pub fn prefetch(key: &str) -> Result<(), ThreadLocalCacheError> {
        THREAD_CACHE.with(|cache| {
            let cache = cache.borrow();
            let mmap_cache = cache
                .as_ref()
                .ok_or(ThreadLocalCacheError::CacheNotInitialized)?;
            mmap_cache.prefetch(key)?;
            Ok(())
        })
    }

    pub fn read_ahead(keys: &[&str]) -> Result<(), ThreadLocalCacheError> {
        THREAD_CACHE.with(|cache| {
            let cache = cache.borrow();
            let mmap_cache = cache
                .as_ref()
                .ok_or(ThreadLocalCacheError::CacheNotInitialized)?;
            mmap_cache.read_ahead(keys)?;
            Ok(())
        })
    }

    pub fn clear() -> Result<(), ThreadLocalCacheError> {
        THREAD_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            if let Some(mmap_cache) = cache.as_mut() {
                mmap_cache.clear()?;
            }
            Ok(())
        })
    }

    pub fn len() -> usize {
        THREAD_CACHE.with(|cache| {
            let cache = cache.borrow();
            match cache.as_ref() {
                Some(mmap_cache) => mmap_cache.len(),
                None => 0,
            }
        })
    }

    pub fn is_empty() -> bool {
        THREAD_CACHE.with(|cache| {
            let cache = cache.borrow();
            match cache.as_ref() {
                Some(mmap_cache) => mmap_cache.is_empty(),
                None => true,
            }
        })
    }

    pub fn current_memory_usage() -> usize {
        THREAD_CACHE.with(|cache| {
            let cache = cache.borrow();
            match cache.as_ref() {
                Some(mmap_cache) => mmap_cache.current_memory_usage(),
                None => 0,
            }
        })
    }

    pub fn max_memory_limit() -> usize {
        THREAD_CACHE.with(|cache| {
            let cache = cache.borrow();
            match cache.as_ref() {
                Some(mmap_cache) => mmap_cache.max_memory_limit(),
                None => 0,
            }
        })
    }

    pub fn is_initialized() -> bool {
        THREAD_CACHE.with(|cache| cache.borrow().is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn thread_local_cache_isInitiallyNotInitialized() {
        assert!(!ThreadLocalCache::is_initialized());
    }

    #[test]
    fn thread_local_cache_init_and_insert() {
        let temp_dir = TempDir::new().unwrap();
        ThreadLocalCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).unwrap();
        assert!(ThreadLocalCache::is_initialized());
        ThreadLocalCache::insert("key1", b"hello world").unwrap();
        let value = ThreadLocalCache::get("key1").unwrap();
        assert_eq!(value, b"hello world");
    }

    #[test]
    fn thread_local_cache_contains_key() {
        let temp_dir = TempDir::new().unwrap();
        ThreadLocalCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).unwrap();
        assert!(!ThreadLocalCache::contains_key("key1"));
        ThreadLocalCache::insert("key1", b"value").unwrap();
        assert!(ThreadLocalCache::contains_key("key1"));
    }

    #[test]
    fn thread_local_cache_remove() {
        let temp_dir = TempDir::new().unwrap();
        ThreadLocalCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).unwrap();
        ThreadLocalCache::insert("key1", b"value").unwrap();
        assert!(ThreadLocalCache::contains_key("key1"));
        ThreadLocalCache::remove("key1").unwrap();
        assert!(!ThreadLocalCache::contains_key("key1"));
    }

    #[test]
    fn thread_local_cache_len() {
        let temp_dir = TempDir::new().unwrap();
        ThreadLocalCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).unwrap();
        assert_eq!(ThreadLocalCache::len(), 0);
        ThreadLocalCache::insert("key1", b"value1").unwrap();
        assert_eq!(ThreadLocalCache::len(), 1);
        ThreadLocalCache::insert("key2", b"value2").unwrap();
        assert_eq!(ThreadLocalCache::len(), 2);
    }

    #[test]
    fn thread_local_cache_is_empty() {
        let temp_dir = TempDir::new().unwrap();
        ThreadLocalCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).unwrap();
        assert!(ThreadLocalCache::is_empty());
        ThreadLocalCache::insert("key1", b"value").unwrap();
        assert!(!ThreadLocalCache::is_empty());
    }

    #[test]
    fn thread_local_cache_clear() {
        let temp_dir = TempDir::new().unwrap();
        ThreadLocalCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).unwrap();
        ThreadLocalCache::insert("key1", b"value1").unwrap();
        ThreadLocalCache::insert("key2", b"value2").unwrap();
        assert_eq!(ThreadLocalCache::len(), 2);
        ThreadLocalCache::clear().unwrap();
        assert!(ThreadLocalCache::is_empty());
    }

    #[test]
    fn thread_local_cache_memory_tracking() {
        let temp_dir = TempDir::new().unwrap();
        ThreadLocalCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).unwrap();
        assert_eq!(ThreadLocalCache::current_memory_usage(), 0);
        ThreadLocalCache::insert("key1", b"hello").unwrap();
        assert_eq!(ThreadLocalCache::current_memory_usage(), 5);
        ThreadLocalCache::insert("key2", b"world").unwrap();
        assert_eq!(ThreadLocalCache::current_memory_usage(), 10);
    }

    #[test]
    fn thread_local_cache_max_memory_limit() {
        let temp_dir = TempDir::new().unwrap();
        let max_mem = 1024 * 1024;
        ThreadLocalCache::new(temp_dir.path().to_path_buf(), max_mem).unwrap();
        assert_eq!(ThreadLocalCache::max_memory_limit(), max_mem);
    }

    #[test]
    fn thread_local_cache_get_missing_key_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        ThreadLocalCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).unwrap();
        let result = ThreadLocalCache::get("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn thread_local_cache_prefetch() {
        let temp_dir = TempDir::new().unwrap();
        ThreadLocalCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).unwrap();
        ThreadLocalCache::insert("key1", b"value").unwrap();
        let result = ThreadLocalCache::prefetch("key1");
        assert!(result.is_ok());
    }

    #[test]
    fn thread_local_cache_read_ahead() {
        let temp_dir = TempDir::new().unwrap();
        ThreadLocalCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).unwrap();
        ThreadLocalCache::insert("key1", b"value1").unwrap();
        ThreadLocalCache::insert("key2", b"value2").unwrap();
        let result = ThreadLocalCache::read_ahead(&["key1", "key2"]);
        assert!(result.is_ok());
    }
}
