use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::num::NonZeroUsize;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LruCacheError {
    #[error("LRU cache capacity cannot be zero")]
    CapacityZero,
    #[error("key not found in cache")]
    KeyNotFound,
}

pub struct LruCache<K, V> {
    capacity: NonZeroUsize,
    access_counter: u64,
    lru_queue: VecDeque<K>,
    entries: HashMap<K, (V, u64)>,
}

impl<K, V> LruCache<K, V>
where
    K: Eq + Hash + Clone,
{
    pub fn new(capacity: NonZeroUsize) -> Self {
        Self {
            capacity,
            access_counter: 0,
            lru_queue: VecDeque::new(),
            entries: HashMap::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Result<Self, LruCacheError> {
        let capacity = NonZeroUsize::new(capacity).ok_or(LruCacheError::CapacityZero)?;
        Ok(Self::new(capacity))
    }

    pub fn insert(&mut self, key: K, value: V) {
        self.access_counter += 1;
        let access = self.access_counter;

        if let Some((existing_value, _)) = self.entries.get_mut(&key) {
            *existing_value = value;
            self.lru_queue.retain(|k| k != &key);
            self.lru_queue.push_back(key);
            return;
        }

        while self.entries.len() >= self.capacity.get() && !self.lru_queue.is_empty() {
            if let Some(lru_key) = self.lru_queue.pop_front() {
                self.entries.remove(&lru_key);
            }
        }

        self.entries.insert(key.clone(), (value, access));
        self.lru_queue.push_back(key);
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.entries.get(key).map(|(v, _)| v)
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        if let Some((v, _)) = self.entries.get_mut(key) {
            Some(v)
        } else {
            None
        }
    }

    pub fn contains(&self, key: &K) -> bool {
        self.entries.contains_key(key)
    }

    pub fn remove(&mut self, key: &K) -> Result<(), LruCacheError> {
        if self.entries.remove(key).is_some() {
            self.lru_queue.retain(|k| k != key);
            Ok(())
        } else {
            Err(LruCacheError::KeyNotFound)
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity.get()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.lru_queue.clear();
        self.access_counter = 0;
    }

    pub fn peek_lru(&self) -> Option<&K> {
        self.lru_queue.front()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries.iter().map(|(k, (v, _))| (k, v))
    }
}

impl<K, V> Default for LruCache<K, V>
where
    K: Eq + Hash + Clone,
{
    fn default() -> Self {
        Self::new(NonZeroUsize::new(100).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_get() {
        let mut cache = LruCache::with_capacity(3).unwrap();
        cache.insert("a", 1);
        cache.insert("b", 2);
        assert_eq!(cache.get(&"a"), Some(&1));
        assert_eq!(cache.get(&"b"), Some(&2));
    }

    #[test]
    fn lru_eviction_order() {
        let mut cache = LruCache::with_capacity(3).unwrap();
        cache.insert("a", 1);
        cache.insert("b", 2);
        cache.insert("c", 3);
        assert!(cache.contains(&"a"));
        cache.get(&"b");
        cache.insert("d", 4);
        assert!(!cache.contains(&"a"));
        assert!(cache.contains(&"b"));
        assert!(cache.contains(&"c"));
        assert!(cache.contains(&"d"));
    }

    #[test]
    fn update_existing_key_does_not_evict() {
        let mut cache = LruCache::with_capacity(2).unwrap();
        cache.insert("a", 1);
        cache.insert("b", 2);
        cache.insert("a", 10);
        cache.insert("c", 3);
        assert!(cache.contains(&"a"));
        assert!(!cache.contains(&"b"));
    }

    #[test]
    fn remove_key() {
        let mut cache = LruCache::with_capacity(3).unwrap();
        cache.insert("a", 1);
        cache.insert("b", 2);
        assert!(cache.remove(&"a").is_ok());
        assert!(!cache.contains(&"a"));
        assert!(cache.contains(&"b"));
        assert!(cache.remove(&"nonexistent").is_err());
    }

    #[test]
    fn clear_cache() {
        let mut cache = LruCache::with_capacity(3).unwrap();
        cache.insert("a", 1);
        cache.insert("b", 2);
        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn peek_lru() {
        let mut cache = LruCache::with_capacity(3).unwrap();
        assert!(cache.peek_lru().is_none());
        cache.insert("a", 1);
        cache.insert("b", 2);
        assert_eq!(cache.peek_lru(), Some(&"a"));
        cache.insert("c", 3);
        assert_eq!(cache.peek_lru(), Some(&"a"));
        cache.get(&"a");
        assert_eq!(cache.peek_lru(), Some(&"a"));
    }

    #[test]
    fn capacity_zero_error() {
        assert!(matches!(
            LruCache::<&str, i32>::with_capacity(0),
            Err(LruCacheError::CapacityZero)
        ));
    }

    #[test]
    fn default_capacity() {
        let cache: LruCache<&str, i32> = LruCache::default();
        assert_eq!(cache.capacity(), 100);
    }

    #[test]
    fn len_and_is_empty() {
        let mut cache = LruCache::with_capacity(5).unwrap();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
        cache.insert("a", 1);
        assert!(!cache.is_empty());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn iter() {
        let mut cache = LruCache::with_capacity(3).unwrap();
        cache.insert("a", 1);
        cache.insert("b", 2);
        let items: Vec<_> = cache.iter().collect();
        assert_eq!(items.len(), 2);
    }
}
