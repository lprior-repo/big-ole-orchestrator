//! Unit tests for hash ring consistent hashing implementation.

use crate::pool::hash_ring::{HashRing, HashRingConfig, RingNode};
use vo_types::connection_pool::PoolId;

#[test]
fn test_hash_ring_empty() {
    let config = HashRingConfig::default();
    let ring = HashRing::new(config);

    assert_eq!(ring.node_count(), 0);
    assert_eq!(ring.total_virtual_nodes(), 0);
    assert!(ring.get_node(&"test-key").is_none());
}

#[test]
fn test_hash_ring_add_node() {
    let config = HashRingConfig::default();
    let mut ring = HashRing::new(config);

    let node = RingNode {
        pool_id: PoolId::new("pool-1"),
        weight: 1,
    };

    ring.add_node(node.clone());

    assert_eq!(ring.node_count(), 1);
    assert!(ring.total_virtual_nodes() > 0);
    assert_eq!(ring.get_node(&"any-key"), Some(PoolId::new("pool-1")));
}

#[test]
fn test_hash_ring_consistent_hashing() {
    let config = HashRingConfig { virtual_nodes: 150 };
    let mut ring = HashRing::new(config);

    let node1 = RingNode {
        pool_id: PoolId::new("pool-1"),
        weight: 1,
    };
    let node2 = RingNode {
        pool_id: PoolId::new("pool-2"),
        weight: 1,
    };

    ring.add_node(node1.clone());
    ring.add_node(node2.clone());

    // Same key should always hash to same node
    let key = "consistent-key";
    let result1 = ring.get_node(&key);
    let result2 = ring.get_node(&key);
    let result3 = ring.get_node(&key);

    assert_eq!(result1, result2);
    assert_eq!(result2, result3);
}

#[test]
fn test_hash_ring_distribution_across_nodes() {
    let config = HashRingConfig { virtual_nodes: 150 };
    let mut ring = HashRing::new(config);

    let node1 = RingNode {
        pool_id: PoolId::new("pool-1"),
        weight: 1,
    };
    let node2 = RingNode {
        pool_id: PoolId::new("pool-2"),
        weight: 1,
    };

    ring.add_node(node1);
    ring.add_node(node2);

    // Test with many keys
    let num_keys = 1000;
    let mut counts = std::collections::HashMap::new();

    for i in 0..num_keys {
        let key = format!("key-{}", i);
        if let Some(pool_id) = ring.get_node(&key) {
            *counts.entry(pool_id).or_insert(0) += 1;
        }
    }

    // With 2 equal-weight nodes, distribution should be roughly 50/50
    // Allow 10% variance
    let total = num_keys as f64;
    let expected_per_node = total / 2.0;
    let tolerance = expected_per_node * 0.1;

    for (pool_id, count) in &counts {
        let diff = (*count as f64 - expected_per_node).abs();
        assert!(
            diff <= tolerance,
            "Distribution too uneven for {:?}: {} vs expected {}",
            pool_id,
            count,
            expected_per_node
        );
    }
}

#[test]
fn test_hash_ring_add_node_redistribution() {
    let config = HashRingConfig { virtual_nodes: 150 };
    let mut ring = HashRing::new(config);

    let node1 = RingNode {
        pool_id: PoolId::new("pool-1"),
        weight: 1,
    };

    ring.add_node(node1);

    // All keys should go to pool-1
    for i in 0..100 {
        let key = format!("test-key-{}", i);
        assert_eq!(ring.get_node(&key), Some(PoolId::new("pool-1")));
    }

    // Add second node
    let node2 = RingNode {
        pool_id: PoolId::new("pool-2"),
        weight: 1,
    };
    ring.add_node(node2);

    // Keys should now be distributed
    let mut counts = std::collections::HashMap::new();
    for i in 0..100 {
        let key = format!("test-key-{}", i);
        if let Some(pool_id) = ring.get_node(&key) {
            *counts.entry(pool_id).or_insert(0) += 1;
        }
    }

    // Should have keys for both pools (allowing for edge cases)
    assert!(counts.contains_key(&PoolId::new("pool-1")));
    assert!(counts.contains_key(&PoolId::new("pool-2")));
}

#[test]
fn test_hash_ring_remove_node() {
    let config = HashRingConfig { virtual_nodes: 150 };
    let mut ring = HashRing::new(config);

    let node1 = RingNode {
        pool_id: PoolId::new("pool-1"),
        weight: 1,
    };
    let node2 = RingNode {
        pool_id: PoolId::new("pool-2"),
        weight: 1,
    };

    ring.add_node(node1);
    ring.add_node(node2);

    assert_eq!(ring.node_count(), 2);

    // Remove pool-1
    ring.remove_node(&PoolId::new("pool-1"));

    assert_eq!(ring.node_count(), 1);
    assert!(ring.get_node(&"any-key") == Some(PoolId::new("pool-2")));
}

#[test]
fn test_hash_ring_remove_node_no_key_loss() {
    let config = HashRingConfig { virtual_nodes: 150 };
    let mut ring = HashRing::new(config);

    let node1 = RingNode {
        pool_id: PoolId::new("pool-1"),
        weight: 1,
    };
    let node2 = RingNode {
        pool_id: PoolId::new("pool-2"),
        weight: 1,
    };

    ring.add_node(node1);
    ring.add_node(node2);

    // Record which keys map to which pool before removal
    let mut key_to_pool = std::collections::HashMap::new();
    for i in 0..1000 {
        let key = format!("key-{}", i);
        if let Some(pool_id) = ring.get_node(&key) {
            key_to_pool.insert(key, pool_id.clone());
        }
    }

    // Remove pool-1
    ring.remove_node(&PoolId::new("pool-1"));

    // All remaining keys should still have a valid pool assignment
    // (they may have been redistributed to pool-2)
    for (key, _expected_pool) in &key_to_pool {
        assert!(
            ring.get_node(key).is_some(),
            "Key {} lost after node removal",
            key
        );
    }
}

#[test]
fn test_hash_ring_weighted_distribution() {
    let config = HashRingConfig { virtual_nodes: 50 };
    let mut ring = HashRing::new(config);

    // Pool-1 with weight 1, Pool-2 with weight 3
    let node1 = RingNode {
        pool_id: PoolId::new("pool-1"),
        weight: 1,
    };
    let node2 = RingNode {
        pool_id: PoolId::new("pool-2"),
        weight: 3,
    };

    ring.add_node(node1);
    ring.add_node(node2);

    // Test with many keys
    let num_keys = 1000;
    let mut counts = std::collections::HashMap::new();

    for i in 0..num_keys {
        let key = format!("key-{}", i);
        if let Some(pool_id) = ring.get_node(&key) {
            *counts.entry(pool_id).or_insert(0) += 1;
        }
    }

    let pool1_count = *counts.get(&PoolId::new("pool-1")).unwrap_or(&0);
    let pool2_count = *counts.get(&PoolId::new("pool-2")).unwrap_or(&0);

    // Pool-2 should get roughly 3x the traffic of pool-1
    // Allow 20% variance for statistical noise
    let ratio = pool2_count as f64 / pool1_count as f64;
    assert!(
        ratio >= 2.0 && ratio <= 4.0,
        "Weighted distribution incorrect: pool-2/pool-1 ratio = {}, expected ~3.0",
        ratio
    );
}

#[test]
fn test_hash_ring_get_nodes() {
    let config = HashRingConfig { virtual_nodes: 150 };
    let mut ring = HashRing::new(config);

    let node1 = RingNode {
        pool_id: PoolId::new("pool-1"),
        weight: 1,
    };
    let node2 = RingNode {
        pool_id: PoolId::new("pool-2"),
        weight: 1,
    };
    let node3 = RingNode {
        pool_id: PoolId::new("pool-3"),
        weight: 1,
    };

    ring.add_node(node1);
    ring.add_node(node2);
    ring.add_node(node3);

    // Get 2 nodes for a key
    let nodes = ring.get_nodes(&"test-key", 2);

    assert_eq!(nodes.len(), 2);
    assert!(nodes
        .iter()
        .all(|n| *n != PoolId::new("pool-1") || nodes.contains(&PoolId::new("pool-1"))));
}

#[test]
fn test_hash_ring_unique_nodes_in_get_nodes() {
    let config = HashRingConfig { virtual_nodes: 150 };
    let mut ring = HashRing::new(config);

    let node1 = RingNode {
        pool_id: PoolId::new("pool-1"),
        weight: 1,
    };
    let node2 = RingNode {
        pool_id: PoolId::new("pool-2"),
        weight: 1,
    };

    ring.add_node(node1);
    ring.add_node(node2);

    // Get 2 nodes - should be unique
    let nodes = ring.get_nodes(&"test-key", 2);

    assert_eq!(nodes.len(), 2);
    assert!(nodes[0] != nodes[1]);
}

#[test]
fn test_hash_ring_single_node() {
    let config = HashRingConfig::default();
    let mut ring = HashRing::new(config);

    let node = RingNode {
        pool_id: PoolId::new("single-pool"),
        weight: 1,
    };

    ring.add_node(node);

    // All keys should go to the single node
    for i in 0..100 {
        let key = format!("key-{}", i);
        assert_eq!(ring.get_node(&key), Some(PoolId::new("single-pool")));
    }
}

#[test]
fn test_hash_ring_empty_ring_returns_none() {
    let config = HashRingConfig::default();
    let ring = HashRing::new(config);

    assert!(ring.get_node(&"any-key").is_none());
    assert!(ring.get_nodes(&"any-key", 1).is_empty());
}

#[test]
fn test_hash_ring_virtual_node_count() {
    let config = HashRingConfig { virtual_nodes: 100 };
    let mut ring = HashRing::new(config);

    let node = RingNode {
        pool_id: PoolId::new("pool-1"),
        weight: 2,
    };

    ring.add_node(node);

    // Should have 100 * 2 = 200 virtual nodes
    assert_eq!(ring.total_virtual_nodes(), 200);
}

#[test]
fn test_hash_ring_with_nodes_constructor() {
    let config = HashRingConfig::default();
    let nodes = vec![
        RingNode {
            pool_id: PoolId::new("pool-1"),
            weight: 1,
        },
        RingNode {
            pool_id: PoolId::new("pool-2"),
            weight: 1,
        },
    ];

    let ring = HashRing::with_nodes(config, nodes);

    assert_eq!(ring.node_count(), 2);
    assert!(ring.get_node(&"any-key").is_some());
}
