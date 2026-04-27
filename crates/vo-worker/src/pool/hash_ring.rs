//! Consistent Hashing Ring for distributed connection pool routing.
//!
//! Provides O(log N) node lookup with minimal redistribution when nodes are added/removed.
//! Uses a virtual node approach to improve distribution fairness.

use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crc32fast::Hasher as CrcHasher;
use vo_common::connection_pool::PoolId;

#[derive(Debug, Clone)]
pub struct HashRingConfig {
    pub virtual_nodes: u32,
}

impl Default for HashRingConfig {
    fn default() -> Self {
        Self { virtual_nodes: 150 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RingNode {
    pub pool_id: PoolId,
    pub weight: u32,
}

impl Hash for RingNode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.pool_id.hash(state);
    }
}

#[derive(Debug, Clone)]
pub struct HashRing {
    config: HashRingConfig,
    ring: BTreeMap<u64, RingNode>,
    virtual_node_count: HashMap<PoolId, u32>,
    total_weight: u64,
}

impl HashRing {
    pub fn new(config: HashRingConfig) -> Self {
        Self {
            config,
            ring: BTreeMap::new(),
            virtual_node_count: HashMap::new(),
            total_weight: 0,
        }
    }

    pub fn with_nodes(config: HashRingConfig, nodes: Vec<RingNode>) -> Self {
        let mut ring = Self::new(config);
        for node in nodes {
            ring.add_node(node);
        }
        ring
    }

    pub fn add_node(&mut self, node: RingNode) {
        let pool_id = node.pool_id.clone();
        let weight = node.weight.max(1);
        let virtual_count = self.config.virtual_nodes * weight;

        for i in 0..virtual_count {
            let key = self.compute_key(&pool_id, i);
            self.ring.insert(
                key,
                RingNode {
                    pool_id: pool_id.clone(),
                    weight,
                },
            );
        }

        *self.virtual_node_count.entry(pool_id.clone()).or_insert(0) += virtual_count;
        self.total_weight += virtual_count as u64;
    }

    pub fn remove_node(&mut self, pool_id: &PoolId) {
        let Some(&virtual_count) = self.virtual_node_count.get(pool_id) else {
            return;
        };

        for i in 0..virtual_count {
            let key = self.compute_key(pool_id, i);
            self.ring.remove(&key);
        }

        self.virtual_node_count.remove(pool_id);
        self.total_weight -= virtual_count as u64;
    }

    pub fn get_node<K: Hash>(&self, key: &K) -> Option<PoolId> {
        if self.ring.is_empty() {
            return None;
        }

        let hash = self.hash_key(key);
        let entry = self
            .ring
            .range(hash..)
            .next()
            .or_else(|| self.ring.iter().next());

        entry.map(|(_, node)| node.pool_id.clone())
    }

    pub fn get_nodes<K: Hash>(&self, key: &K, count: usize) -> Vec<PoolId> {
        if self.ring.is_empty() || count == 0 {
            return Vec::new();
        }

        let hash = self.hash_key(key);
        let mut result = Vec::with_capacity(count);
        let mut seen_pools: std::collections::HashSet<&PoolId> = std::collections::HashSet::new();

        // Iterate through ring starting from hash position, wrapping around
        let ring_entries: Vec<_> = self.ring.iter().collect();
        let start_index = ring_entries
            .iter()
            .position(|(k, _)| **k >= hash)
            .unwrap_or(ring_entries.len());

        for i in 0..ring_entries.len() {
            if result.len() >= count {
                break;
            }
            let idx = (start_index + i) % ring_entries.len();
            let (_, node) = &ring_entries[idx];
            if !seen_pools.contains(&node.pool_id) {
                seen_pools.insert(&node.pool_id);
                result.push(node.pool_id.clone());
            }
        }

        result
    }

    pub fn node_count(&self) -> usize {
        self.virtual_node_count.len()
    }

    pub fn total_virtual_nodes(&self) -> u64 {
        self.total_weight
    }

    fn compute_key(&self, pool_id: &PoolId, virtual_index: u32) -> u64 {
        let mut hasher = CrcHasher::new();
        pool_id.as_str().hash(&mut hasher);
        virtual_index.hash(&mut hasher);
        hasher.finish()
    }

    fn hash_key<K: Hash>(&self, key: &K) -> u64 {
        let mut hasher = CrcHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_node(id: &str) -> RingNode {
        RingNode {
            pool_id: PoolId::new(id),
            weight: 1,
        }
    }

    #[test]
    fn test_empty_ring_returns_none() {
        let ring = HashRing::new(HashRingConfig::default());
        assert!(ring.get_node(&"test".to_string()).is_none());
    }

    #[test]
    fn test_single_node_always_returns_that_node() {
        let mut ring = HashRing::new(HashRingConfig::default());
        ring.add_node(create_test_node("node1"));

        for key in &["key1", "key2", "key3", "different", "test"] {
            assert_eq!(ring.get_node(key), Some(PoolId::new("node1")));
        }
    }

    #[test]
    fn test_get_nodes_returns_unique_pools() {
        let mut ring = HashRing::new(HashRingConfig::default());
        ring.add_node(create_test_node("node1"));
        ring.add_node(create_test_node("node2"));
        ring.add_node(create_test_node("node3"));

        let nodes = ring.get_nodes(&"test".to_string(), 3);
        assert_eq!(nodes.len(), 3);
        assert!(nodes.contains(&PoolId::new("node1")));
        assert!(nodes.contains(&PoolId::new("node2")));
        assert!(nodes.contains(&PoolId::new("node3")));
    }

    #[test]
    fn test_get_nodes_count_limit() {
        let mut ring = HashRing::new(HashRingConfig::default());
        ring.add_node(create_test_node("node1"));
        ring.add_node(create_test_node("node2"));

        let nodes = ring.get_nodes(&"test".to_string(), 1);
        assert_eq!(nodes.len(), 1);
    }

    #[test]
    fn test_remove_node() {
        let mut ring = HashRing::new(HashRingConfig::default());
        ring.add_node(create_test_node("node1"));
        ring.add_node(create_test_node("node2"));

        ring.remove_node(&PoolId::new("node1"));

        assert_eq!(ring.node_count(), 1);
        let remaining = ring.get_node(&"test".to_string());
        assert_eq!(remaining, Some(PoolId::new("node2")));
    }

    #[test]
    fn test_consistent_distribution() {
        let mut ring = HashRing::new(HashRingConfig::default());
        ring.add_node(create_test_node("node1"));
        ring.add_node(create_test_node("node2"));

        let mut distribution = HashMap::new();
        for i in 0..1000 {
            let key = format!("key_{}", i);
            if let Some(node) = ring.get_node(&key) {
                *distribution.entry(node).or_insert(0) += 1;
            }
        }

        assert_eq!(distribution.len(), 2);
        for count in distribution.values() {
            assert!(*count > 300, "Distribution should be reasonably balanced");
        }
    }

    #[test]
    fn test_weighted_nodes() {
        let mut ring = HashRing::new(HashRingConfig { virtual_nodes: 100 });
        ring.add_node(RingNode {
            pool_id: PoolId::new("low"),
            weight: 1,
        });
        ring.add_node(RingNode {
            pool_id: PoolId::new("high"),
            weight: 3,
        });

        let mut distribution = HashMap::new();
        for i in 0..1000 {
            let key = format!("key_{}", i);
            if let Some(node) = ring.get_node(&key) {
                *distribution.entry(node).or_insert(0) += 1;
            }
        }

        let high_count = *distribution.get(&PoolId::new("high")).unwrap_or(&0);
        let low_count = *distribution.get(&PoolId::new("low")).unwrap_or(&0);
        assert!(
            high_count > low_count * 2,
            "High weight node should get more traffic"
        );
    }

    #[test]
    fn test_node_count() {
        let mut ring = HashRing::new(HashRingConfig::default());
        assert_eq!(ring.node_count(), 0);

        ring.add_node(create_test_node("node1"));
        assert_eq!(ring.node_count(), 1);

        ring.add_node(create_test_node("node2"));
        assert_eq!(ring.node_count(), 2);

        ring.remove_node(&PoolId::new("node1"));
        assert_eq!(ring.node_count(), 1);
    }

    #[test]
    fn test_total_virtual_nodes() {
        let mut ring = HashRing::new(HashRingConfig { virtual_nodes: 100 });
        assert_eq!(ring.total_virtual_nodes(), 0);

        ring.add_node(RingNode {
            pool_id: PoolId::new("node1"),
            weight: 1,
        });
        assert_eq!(ring.total_virtual_nodes(), 100);

        ring.add_node(RingNode {
            pool_id: PoolId::new("node2"),
            weight: 2,
        });
        assert_eq!(ring.total_virtual_nodes(), 300);
    }

    #[test]
    fn test_with_nodes_constructor() {
        let nodes = vec![
            RingNode {
                pool_id: PoolId::new("node1"),
                weight: 1,
            },
            RingNode {
                pool_id: PoolId::new("node2"),
                weight: 1,
            },
        ];

        let ring = HashRing::with_nodes(HashRingConfig::default(), nodes);
        assert_eq!(ring.node_count(), 2);
    }
}
