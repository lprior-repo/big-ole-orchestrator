use std::collections::{HashMap, VecDeque};

use super::page::CacheRegion;
use super::MmapCacheError;

#[derive(Debug)]
pub(super) struct LruEntry {
    pub(super) key: String,
    pub(super) region: CacheRegion,
    pub(super) last_access: u64,
}

pub(super) fn evict_until_space_available(
    lru_queue: &mut VecDeque<String>,
    entries: &mut HashMap<String, LruEntry>,
    current_memory_bytes: &mut usize,
    max_memory_bytes: usize,
    needed: usize,
) -> Result<(), MmapCacheError> {
    while *current_memory_bytes + needed > max_memory_bytes && !lru_queue.is_empty() {
        if let Some(lru_key) = lru_queue.pop_front() {
            if let Some(entry) = entries.remove(&lru_key) {
                *current_memory_bytes -= entry.region.size as usize;
                let _ = std::fs::remove_file(entry.region.file_path);
            }
        }
    }
    Ok(())
}
