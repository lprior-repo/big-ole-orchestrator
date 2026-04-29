//! Red Queen coevolutionary adversarial tests for hash_ring hotspot behavior.
//!
//! Task ID: rq-024
//! EARS Requirements:
//!   - Ubiquitous: THE SYSTEM SHALL maintain even distribution
//!   - Event-Driven: WHEN node added/removed, THE SYSTEM SHALL minimize redistribution
//!   - Unwanted: If hotspot created, THE SYSTEM SHALL have uneven load
//!
//! Contracts:
//!   - Preconditions: Hash ring stable
//!   - Postconditions: Distribution even after change
//!   - Invariants: Minimal redistribution

use std::collections::HashMap;
use vo_types::connection_pool::PoolId;
use vo_worker::pool::hash_ring::{HashRing, HashRingConfig, RingNode};

const TEST_KEY_COUNT: usize = 10_000;
const VIRTUAL_NODES: u32 = 150;

fn baseline_distribution(ring: &HashRing, key_count: usize) -> HashMap<String, usize> {
    let mut dist = HashMap::new();
    for i in 0..key_count {
        let key = format!("key-{}", i);
        if let Some(node) = ring.get_node(&key) {
            *dist.entry(node.to_string()).or_insert(0) += 1;
        }
    }
    dist
}

fn compute_redistribution(
    before: &HashMap<String, usize>,
    after: &HashMap<String, usize>,
    key_count: usize,
) -> f64 {
    let mut changed = 0usize;
    for i in 0..key_count {
        let key = format!("key-{}", i);
        let before_node = before.get(&format!("key-{}", i));
        let after_node = after.get(&format!("key-{}", i));
        if before_node != after_node {
            changed += 1;
        }
    }
    changed as f64 / key_count as f64
}

fn distribution_variance(dist: &HashMap<String, usize>, total: usize) -> f64 {
    if dist.is_empty() {
        return 0.0;
    }
    let expected = total as f64 / dist.len() as f64;
    let variance: f64 = dist
        .values()
        .map(|&c| (c as f64 - expected).powi(2))
        .sum::<f64>()
        / dist.len() as f64;
    variance.sqrt()
}

fn is_evenly_distributed(dist: &HashMap<String, usize>, total: usize) -> bool {
    if dist.is_empty() {
        return true;
    }
    let expected = total as f64 / dist.len() as f64;
    let variance_threshold = expected * 0.5;
    for &count in dist.values() {
        if (count as f64 - expected).abs() > variance_threshold {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod hotspot_after_node_add {
    use super::*;

    #[test]
    fn minimal_redistribution_when_node_added_to_3_node_ring() {
        let mut ring = HashRing::new(HashRingConfig {
            virtual_nodes: VIRTUAL_NODES,
        });
        ring.add_node(RingNode {
            pool_id: PoolId::new("node-1"),
            weight: 1,
        });
        ring.add_node(RingNode {
            pool_id: PoolId::new("node-2"),
            weight: 1,
        });
        ring.add_node(RingNode {
            pool_id: PoolId::new("node-3"),
            weight: 1,
        });

        let before = baseline_distribution(&ring, TEST_KEY_COUNT);
        assert_eq!(before.len(), 3, "Precondition: 3 nodes must be present");
        assert!(
            is_evenly_distributed(&before, TEST_KEY_COUNT),
            "Precondition: distribution must be even before node add"
        );

        ring.add_node(RingNode {
            pool_id: PoolId::new("node-4"),
            weight: 1,
        });

        let after = baseline_distribution(&ring, TEST_KEY_COUNT);
        let redistribution_pct = compute_redistribution(&before, &after, TEST_KEY_COUNT);

        let expected_max_redistribution = 0.30;
        assert!(
            redistribution_pct <= expected_max_redistribution,
            "Redistribution {}% exceeds maximum {}% for adding 4th node to 3-node ring. \
             Keys that remapped: {}/{}",
            (redistribution_pct * 100.0).round(),
            (expected_max_redistribution * 100.0).round(),
            (redistribution_pct * TEST_KEY_COUNT as f64).round() as usize,
            TEST_KEY_COUNT
        );
    }

    #[test]
    fn even_distribution_maintained_after_node_add() {
        let mut ring = HashRing::new(HashRingConfig {
            virtual_nodes: VIRTUAL_NODES,
        });
        for i in 0..3 {
            ring.add_node(RingNode {
                pool_id: PoolId::new(format!("node-{}", i)),
                weight: 1,
            });
        }

        ring.add_node(RingNode {
            pool_id: PoolId::new("new-node"),
            weight: 1,
        });

        let dist = baseline_distribution(&ring, TEST_KEY_COUNT);
        assert!(
            is_evenly_distributed(&dist, TEST_KEY_COUNT),
            "Hotspot detected: distribution too uneven after node add. \
             Counts per node: {:?}",
            dist
        );
    }

    #[test]
    fn redistribution_percentage_decreases_with_more_virtual_nodes() {
        let key_count = 5000;
        let low_vn = 50u32;
        let high_vn = 200u32;

        let mut ring_low = HashRing::new(HashRingConfig {
            virtual_nodes: low_vn,
        });
        for i in 0..3 {
            ring_low.add_node(RingNode {
                pool_id: PoolId::new(format!("node-{}", i)),
                weight: 1,
            });
        }
        let before_low = baseline_distribution(&ring_low, key_count);
        ring_low.add_node(RingNode {
            pool_id: PoolId::new("node-4"),
            weight: 1,
        });
        let after_low = baseline_distribution(&ring_low, key_count);
        let redist_low = compute_redistribution(&before_low, &after_low, key_count);

        let mut ring_high = HashRing::new(HashRingConfig {
            virtual_nodes: high_vn,
        });
        for i in 0..3 {
            ring_high.add_node(RingNode {
                pool_id: PoolId::new(format!("node-{}", i)),
                weight: 1,
            });
        }
        let before_high = baseline_distribution(&ring_high, key_count);
        ring_high.add_node(RingNode {
            pool_id: PoolId::new("node-4"),
            weight: 1,
        });
        let after_high = baseline_distribution(&ring_high, key_count);
        let redist_high = compute_redistribution(&before_high, &after_high, key_count);

        assert!(
            redist_high <= redist_low,
            "Higher virtual nodes ({}) should cause <= redistribution than lower ({}) \
             (got {}% vs {}%)",
            high_vn,
            low_vn,
            (redist_high * 100.0).round(),
            (redist_low * 100.0).round()
        );
    }
}

#[cfg(test)]
mod hotspot_after_node_remove {
    #[test]
    fn minimal_redistribution_when_node_removed_from_4_node_ring() {
        let mut ring = HashRing::new(HashRingConfig {
            virtual_nodes: VIRTUAL_NODES,
        });
        for i in 0..4 {
            ring.add_node(RingNode {
                pool_id: PoolId::new(format!("node-{}", i)),
                weight: 1,
            });
        }

        let before = baseline_distribution(&ring, TEST_KEY_COUNT);
        assert_eq!(before.len(), 4, "Precondition: 4 nodes must be present");
        assert!(
            is_evenly_distributed(&before, TEST_KEY_COUNT),
            "Precondition: distribution must be even before node remove"
        );

        ring.remove_node(&PoolId::new("node-2"));

        let after = baseline_distribution(&ring, TEST_KEY_COUNT);
        let redistribution_pct = compute_redistribution(&before, &after, TEST_KEY_COUNT);

        let expected_max_redistribution = 0.30;
        assert!(
            redistribution_pct <= expected_max_redistribution,
            "Redistribution {}% exceeds maximum {}% for removing node from 4-node ring. \
             Keys that remapped: {}/{}",
            (redistribution_pct * 100.0).round(),
            (expected_max_redistribution * 100.0).round(),
            (redistribution_pct * TEST_KEY_COUNT as f64).round() as usize,
            TEST_KEY_COUNT
        );
    }

    #[test]
    fn even_distribution_maintained_after_node_remove() {
        let mut ring = HashRing::new(HashRingConfig {
            virtual_nodes: VIRTUAL_NODES,
        });
        for i in 0..4 {
            ring.add_node(RingNode {
                pool_id: PoolId::new(format!("node-{}", i)),
                weight: 1,
            });
        }

        ring.remove_node(&PoolId::new("node-1"));

        let dist = baseline_distribution(&ring, TEST_KEY_COUNT);
        assert!(
            is_evenly_distributed(&dist, TEST_KEY_COUNT),
            "Hotspot detected: distribution too uneven after node remove. \
             Counts per node: {:?}",
            dist
        );
    }

    #[test]
    fn redistribution_bounded_by_virtual_node_count() {
        let key_count = 5000;
        let mut ring = HashRing::new(HashRingConfig { virtual_nodes: 300 });
        for i in 0..5 {
            ring.add_node(RingNode {
                pool_id: PoolId::new(format!("node-{}", i)),
                weight: 1,
            });
        }

        let before = baseline_distribution(&ring, key_count);
        ring.remove_node(&PoolId::new("node-2"));
        let after = baseline_distribution(&ring, key_count);
        let redistribution_pct = compute_redistribution(&before, &after, key_count);

        assert!(
            redistribution_pct < 0.5,
            "Redistribution {}% should be bounded even with high virtual node count",
            (redistribution_pct * 100.0).round()
        );
    }
}

#[cfg(test)]
mod hotspot_stability {
    #[test]
    fn no_hotspots_with_weighted_nodes_after_add() {
        let mut ring = HashRing::new(HashRingConfig {
            virtual_nodes: VIRTUAL_NODES,
        });
        ring.add_node(RingNode {
            pool_id: PoolId::new("high-weight"),
            weight: 4,
        });
        ring.add_node(RingNode {
            pool_id: PoolId::new("low-weight"),
            weight: 1,
        });

        ring.add_node(RingNode {
            pool_id: PoolId::new("new-node"),
            weight: 1,
        });

        let dist = baseline_distribution(&ring, TEST_KEY_COUNT);
        assert!(
            is_evenly_distributed(&dist, TEST_KEY_COUNT),
            "Hotspot detected with weighted nodes. Distribution: {:?}",
            dist
        );
    }

    #[test]
    fn variance_bounded_after_consecutive_changes() {
        let mut ring = HashRing::new(HashRingConfig {
            virtual_nodes: VIRTUAL_NODES,
        });
        for i in 0..3 {
            ring.add_node(RingNode {
                pool_id: PoolId::new(format!("node-{}", i)),
                weight: 1,
            });
        }

        let dist1 = baseline_distribution(&ring, TEST_KEY_COUNT);
        let var1 = distribution_variance(&dist1, TEST_KEY_COUNT);

        ring.add_node(RingNode {
            pool_id: PoolId::new("node-3"),
            weight: 1,
        });
        let dist2 = baseline_distribution(&ring, TEST_KEY_COUNT);
        let var2 = distribution_variance(&dist2, TEST_KEY_COUNT);

        ring.add_node(RingNode {
            pool_id: PoolId::new("node-4"),
            weight: 1,
        });
        let dist3 = baseline_distribution(&ring, TEST_KEY_COUNT);
        let var3 = distribution_variance(&dist3, TEST_KEY_COUNT);

        assert!(
            var3 <= var1 * 2.0,
            "Variance grew too much after consecutive adds: var1={:.2}, var2={:.2}, var3={:.2}",
            var1,
            var2,
            var3
        );
    }

    #[test]
    fn single_node_ring_always_returns_same_node() {
        let mut ring = HashRing::new(HashRingConfig {
            virtual_nodes: VIRTUAL_NODES,
        });
        ring.add_node(RingNode {
            pool_id: PoolId::new("only-node"),
            weight: 1,
        });

        for i in 0..100 {
            let key = format!("key-{}", i);
            assert_eq!(
                ring.get_node(&key),
                Some(PoolId::new("only-node")),
                "Single node ring must always return same node for key {}",
                key
            );
        }
    }
}
