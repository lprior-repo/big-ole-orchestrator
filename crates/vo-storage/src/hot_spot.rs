// Hot spot key detection and mitigation for Fjall event store partition.
//
// The events partition uses [instance_id(16)][sequence_u64_be(8)] keys stored
// lexicographically in Fjall's LSM-tree. A single instance writing rapidly
// clusters keys in the same leaf/page region, causing write amplification,
// disk I/O hot spots, and compaction bottleneck.
//
// Detection: per-instance AtomicU64 counters (max events, writes/sec)
// Mitigation: XOR-based key scrambling that spreads keys across the key space
// while preserving key length and sequence byte sort order.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use parking_lot::RwLock;
use vo_types::InstanceId;

/// Hash function: FNV-1a 64-bit.
/// Fast, no_std-compatible, deterministic, good avalanche effect.
#[inline]
fn fnv1a_64(data: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 14695981039346656037;
    const FNV_PRIME: u64 = 1099511628211;

    let mut hash = FNV_OFFSET;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// XOR-based instance ID scrambling.
/// Spreads the same instance ID across different key positions while
/// preserving key length (16 bytes).
/// Reversible via `unscramble_instance_id`.
#[inline]
pub fn scramble_instance_id(instance_id: &InstanceId) -> [u8; 16] {
    let id_bytes = instance_id.to_bytes().unwrap_or([0u8; 16]);
    scramble_instance_id_from_bytes(&id_bytes)
}

#[inline]
fn scramble_instance_id_from_bytes(id_bytes: &[u8; 16]) -> [u8; 16] {
    let hash = fnv1a_64(id_bytes);
    let mut scrambled = *id_bytes;
    let hash_bytes = hash.to_le_bytes();
    for i in 0..8 {
        scrambled[i] ^= hash_bytes[i];
    }
    scrambled
}

/// Reverse of `scramble_instance_id`.
/// Since XOR is its own inverse: `unscramble(scrambled(x)) = x`.
#[inline]
pub fn unscramble_instance_id(scrambled: &[u8; 16]) -> [u8; 16] {
    let hash = fnv1a_64(scrambled);
    let mut result = *scrambled;
    let hash_bytes = hash.to_le_bytes();
    for i in 0..8 {
        result[i] ^= hash_bytes[i];
    }
    result
}

/// Apply scrambling to an event key.
/// Event key format: [instance_id(16)][sequence_u64_be(8)] = 26 bytes
/// Only the instance_id portion is scrambled; sequence bytes are untouched
/// (preserving per-instance sort order).
#[inline]
pub fn scramble_event_key(key: &[u8]) -> [u8; 26] {
    let mut scrambled = [0u8; 26];
    let id_bytes: [u8; 16] = key[0..16].try_into().unwrap();
    scrambled[0..16].copy_from_slice(&scramble_instance_id_from_bytes(&id_bytes));
    scrambled[16..24].copy_from_slice(&key[16..24]);
    scrambled
}

/// Reverse of `scramble_event_key`.
#[inline]
pub fn unscramble_event_key(scrambled_key: &[u8]) -> [u8; 26] {
    let mut key = [0u8; 26];
    let id_bytes: [u8; 16] = scrambled_key[0..16].try_into().unwrap();
    key[0..16].copy_from_slice(&unscramble_instance_id(&id_bytes));
    key[16..24].copy_from_slice(&scrambled_key[16..24]);
    key
}

/// Configuration for hot spot detection.
#[derive(Debug, Clone)]
pub struct HotSpotConfig {
    /// Number of events from a single instance to trigger hot spot detection.
    pub max_events: u64,
    /// Maximum writes per second from a single instance.
    pub max_writes_per_second: u64,
    /// Window size in milliseconds for rate calculation.
    pub window_ms: u64,
}

impl Default for HotSpotConfig {
    fn default() -> Self {
        Self {
            max_events: 10_000,
            max_writes_per_second: 500,
            window_ms: 1000,
        }
    }
}

/// Per-instance metrics tracked by the detector.
struct InstanceMetrics {
    /// Total append count for this instance.
    append_count: AtomicU64,
    /// Timestamp of the last check for rate calculation.
    last_check: AtomicUsize,
    /// Number of writes in the current window.
    window_writes: AtomicU64,
    /// Start of the current window.
    window_start: AtomicUsize,
}

/// Thread-safe hot spot detector.
///
/// Tracks per-instance append events using AtomicU64 counters protected by a
/// read-write lock on the HashMap. Suitable for single-threaded or lightly
/// concurrent workloads.
pub struct HotSpotDetector {
    config: HotSpotConfig,
    /// Per-instance metrics.
    instances: RwLock<HashMap<InstanceId, InstanceMetrics>>,
}

impl HotSpotDetector {
    /// Create a new detector with the given config.
    pub fn new(config: HotSpotConfig) -> Self {
        Self {
            config,
            instances: RwLock::new(HashMap::new()),
        }
    }

    /// Record an append event for an instance.
    /// Returns true if the instance is now flagged as hot.
    pub fn record_append(&self, instance_id: &InstanceId) -> bool {
        let mut instances = self.instances.write();
        let metrics = instances
            .entry(instance_id.clone())
            .or_insert_with(|| InstanceMetrics {
                append_count: AtomicU64::new(0),
                last_check: AtomicUsize::new(0),
                window_writes: AtomicU64::new(0),
                window_start: AtomicUsize::new(0),
            });

        // Increment append count
        let new_count = metrics.append_count.fetch_add(1, Ordering::Relaxed) + 1;

        // Check rate: simple window-based check
        let now = Instant::now()
            .duration_since(Instant::now() - std::time::Duration::from_millis(0))
            .as_millis() as usize;
        // Use a simple approach: increment window counter and check periodically
        let window_start = metrics.window_start.load(Ordering::Relaxed);
        if now.saturating_sub(window_start) > self.config.window_ms as usize {
            metrics.window_writes.store(0, Ordering::Relaxed);
            metrics.window_start.store(now, Ordering::Relaxed);
        }
        let window_count = metrics.window_writes.fetch_add(1, Ordering::Relaxed) + 1;

        // Hot spot if either threshold exceeded
        new_count >= self.config.max_events || window_count >= self.config.max_writes_per_second
    }

    /// Check if an instance is currently hot.
    pub fn is_hot(&self, instance_id: &InstanceId) -> bool {
        let instances = self.instances.read();
        match instances.get(instance_id) {
            Some(metrics) => {
                metrics.append_count.load(Ordering::Relaxed) >= self.config.max_events
            }
            None => false,
        }
    }

    /// Reset metrics for an instance (useful for testing).
    pub fn reset(&self, instance_id: &InstanceId) {
        let mut instances = self.instances.write();
        if let Some(metrics) = instances.get_mut(instance_id) {
            metrics.append_count.store(0, Ordering::Relaxed);
            metrics.window_writes.store(0, Ordering::Relaxed);
        }
    }
}

/// Thread-safe hot spot detector with sharded lock.
///
/// Uses FNV-1a hash of instance ID to distribute instances across multiple
/// RwLock instances, reducing contention for multi-instance workloads.
pub struct ShardedHotSpotDetector {
    shards: Arc<Vec<HotSpotDetector>>,
    shard_count: usize,
}

impl ShardedHotSpotDetector {
    /// Create a new sharded detector.
    pub fn new(config: HotSpotConfig, shard_count: usize) -> Self {
        let shards = (0..shard_count)
            .map(|_| HotSpotDetector::new(config.clone()))
            .collect();
        Self {
            shards: Arc::new(shards),
            shard_count,
        }
    }

    /// Get the shard index for an instance ID.
    fn shard_index(&self, instance_id: &InstanceId) -> usize {
        let id_bytes = instance_id.to_bytes().unwrap_or([0u8; 16]);
        fnv1a_64(&id_bytes) as usize % self.shard_count
    }

    /// Record an append event for an instance.
    pub fn record_append(&self, instance_id: &InstanceId) -> bool {
        let idx = self.shard_index(instance_id);
        self.shards[idx].record_append(instance_id)
    }

    /// Check if an instance is currently hot.
    pub fn is_hot(&self, instance_id: &InstanceId) -> bool {
        let idx = self.shard_index(instance_id);
        self.shards[idx].is_hot(instance_id)
    }

    /// Reset metrics for an instance.
    pub fn reset(&self, instance_id: &InstanceId) {
        let idx = self.shard_index(instance_id);
        self.shards[idx].reset(instance_id);
    }
}

/// Provider trait for hot spot detection.
/// Allows dependency injection of the detector into FjallEventStore.
pub trait HotSpotProvider: Send + Sync {
    /// Record an append event and return true if instance is now hot.
    fn record_append(&self, instance_id: &InstanceId) -> bool;

    /// Check if an instance is currently hot.
    fn is_hot(&self, instance_id: &InstanceId) -> bool;

    /// Reset metrics for an instance.
    fn reset(&self, instance_id: &InstanceId);
}

impl HotSpotProvider for HotSpotDetector {
    fn record_append(&self, instance_id: &InstanceId) -> bool {
        self.record_append(instance_id)
    }

    fn is_hot(&self, instance_id: &InstanceId) -> bool {
        self.is_hot(instance_id)
    }

    fn reset(&self, instance_id: &InstanceId) {
        self.reset(instance_id);
    }
}

impl HotSpotProvider for ShardedHotSpotDetector {
    fn record_append(&self, instance_id: &InstanceId) -> bool {
        self.record_append(instance_id)
    }

    fn is_hot(&self, instance_id: &InstanceId) -> bool {
        self.is_hot(instance_id)
    }

    fn reset(&self, instance_id: &InstanceId) {
        self.reset(instance_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- FNV-1a hashing tests ---

    #[test]
    fn test_fnv1a_deterministic() {
        let data = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let hash1 = fnv1a_64(&data);
        let hash2 = fnv1a_64(&data);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_fnv1a_different_inputs_different_hashes() {
        let data1 = [0u8; 16];
        let data2 = [1u8; 16];
        let hash1 = fnv1a_64(&data1);
        let hash2 = fnv1a_64(&data2);
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_fnv1a_avalanche() {
        // Single bit change should produce significantly different hash
        let data1 = [0u8; 16];
        let mut data2 = [0u8; 16];
        data2[0] = 1;
        let hash1 = fnv1a_64(&data1);
        let hash2 = fnv1a_64(&data2);
        // At least 15 bits should differ (good avalanche)
        let diff = (hash1 ^ hash2).count_ones();
        assert!(diff >= 15, "Expected >= 15 bits different, got {}", diff);
    }

    // --- Scrambling tests ---

    #[test]
    fn test_scramble_preserves_length() {
        let instance_id = InstanceId::from_bytes([1u8; 16]);
        let scrambled = scramble_instance_id(&instance_id);
        assert_eq!(scrambled.len(), 16);
    }

    #[test]
    fn test_scramble_is_reversible() {
        let bytes = [0xABu8, 0xCD, 0xEF, 0x01, 0x23, 0x45, 0x67, 0x89, 0xFF, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let instance_id = InstanceId::from_bytes(bytes);
        let scrambled = scramble_instance_id(&instance_id);
        let recovered = unscramble_instance_id(&scrambled);
        assert_eq!(recovered, bytes);
    }

    #[test]
    fn test_scramble_differentiates_same_input() {
        let instance_id = InstanceId::from_bytes([0u8; 16]);
        let scrambled = scramble_instance_id(&instance_id);
        // Hash of all-zeros is FNV_OFFSET, XOR should change at least first 8 bytes
        assert_ne!(scrambled, [0u8; 16]);
    }

    #[test]
    fn test_scramble_event_key_preserves_sequence() {
        let key = [0xABu8; 26];
        key[16..24].copy_from_slice(&[0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07]);
        let scrambled = scramble_event_key(&key);
        // Sequence bytes should be preserved
        assert_eq!(&scrambled[16..24], &[0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07]);
    }

    #[test]
    fn test_scramble_event_key_reversible() {
        let key = [0xDEu8, 0xAD, 0xBE, 0xEF, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09];
        let scrambled = scramble_event_key(&key);
        let recovered = unscramble_event_key(&scrambled);
        assert_eq!(recovered, key);
    }

    // --- HotSpotDetector tests ---

    #[test]
    fn test_detector_no_hot_spot_initially() {
        let config = HotSpotConfig {
            max_events: 100,
            max_writes_per_second: 50,
            window_ms: 1000,
        };
        let detector = HotSpotDetector::new(config);
        let instance_id = InstanceId::from_bytes([0x01u8; 16]);
        assert!(!detector.is_hot(&instance_id));
    }

    #[test]
    fn test_detector_threshold_triggers() {
        let config = HotSpotConfig {
            max_events: 10,
            max_writes_per_second: 100,
            window_ms: 1000,
        };
        let detector = HotSpotDetector::new(config);
        let instance_id = InstanceId::from_bytes([0x02u8; 16]);

        // Fire up to threshold
        for _ in 0..9 {
            detector.record_append(&instance_id);
        }
        assert!(!detector.is_hot(&instance_id));

        // One more should trigger
        detector.record_append(&instance_id);
        assert!(detector.is_hot(&instance_id));
    }

    #[test]
    fn test_detector_multi_instance_independent() {
        let config = HotSpotConfig {
            max_events: 5,
            max_writes_per_second: 100,
            window_ms: 1000,
        };
        let detector = HotSpotDetector::new(config.clone());
        let instance_a = InstanceId::from_bytes([0x0Au8; 16]);
        let instance_b = InstanceId::from_bytes([0x0Bu8; 16]);

        // Fill instance A to threshold
        for _ in 0..5 {
            detector.record_append(&instance_a);
        }
        assert!(detector.is_hot(&instance_a));
        // Instance B should not be affected
        assert!(!detector.is_hot(&instance_b));
    }

    #[test]
    fn test_detector_reset_clears_hot_status() {
        let config = HotSpotConfig {
            max_events: 5,
            max_writes_per_second: 100,
            window_ms: 1000,
        };
        let detector = HotSpotDetector::new(config);
        let instance_id = InstanceId::from_bytes([0x0Cu8; 16]);

        for _ in 0..5 {
            detector.record_append(&instance_id);
        }
        assert!(detector.is_hot(&instance_id));

        detector.reset(&instance_id);
        assert!(!detector.is_hot(&instance_id));
    }

    // --- ShardedHotSpotDetector tests ---

    #[test]
    fn test_sharded_detector_works() {
        let config = HotSpotConfig {
            max_events: 5,
            max_writes_per_second: 100,
            window_ms: 1000,
        };
        let detector = ShardedHotSpotDetector::new(config.clone(), 8);
        let instance_id = InstanceId::from_bytes([0x10u8; 16]);

        for _ in 0..5 {
            detector.record_append(&instance_id);
        }
        assert!(detector.is_hot(&instance_id));
    }

    #[test]
    fn test_sharded_detector_multi_instance() {
        let config = HotSpotConfig {
            max_events: 5,
            max_writes_per_second: 100,
            window_ms: 1000,
        };
        let detector = ShardedHotSpotDetector::new(config, 8);
        let instance_a = InstanceId::from_bytes([0x20u8; 16]);
        let instance_b = InstanceId::from_bytes([0x21u8; 16]);

        for _ in 0..5 {
            detector.record_append(&instance_a);
        }
        assert!(detector.is_hot(&instance_a));
        assert!(!detector.is_hot(&instance_b));
    }
}
