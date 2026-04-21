//! REDQUEEN adversarial test: Hash Ring Hotspot Analysis
//! 
//! Tests whether the hash ring causes excessive redistribution (hotspots) when nodes are added or removed.
//! True consistent hashing should only redistribute ~1/N keys where N is the number of nodes.
//! If significantly more keys redistribute, we have a hotspot vulnerability.

use std::collections::HashMap;
use vo_worker::pool::hash_ring::{HashRing, HashRingConfig, RingNode};
use vo_types::connection_pool::PoolId;

/// Helper to create a ring node
fn node(id: &str) -> RingNode {
    RingNode {
        pool_id: PoolId::new(id),
        weight: 1,
    }
}

/// Helper to get distribution map
fn get_distribution(ring: &HashRing, keys: &[String]) -> HashMap<String, u32> {
    let mut dist = HashMap::new();
    for key in keys {
        if let Some(n) = ring.get_node(key) {
            *dist.entry(n.to_string()).or_insert(0) += 1;
        }
    }
    dist
}

#[test]
fn test_redistribution_on_node_add_4_to_5() {
    let mut ring = HashRing::new(HashRingConfig { virtual_nodes: 150 });
    for i in 0..4 {
        ring.add_node(node(&format!("node-{}", i)));
    }
    
    let keys: Vec<String> = (0..10000).map(|i| format!("key-{}", i)).collect();
    let initial: Vec<PoolId> = keys.iter().map(|k| ring.get_node(k).unwrap()).collect();
    
    // Add 5th node
    ring.add_node(node("node-new"));
    
    // Count changes
    let changes: usize = keys.iter().zip(initial.iter()).filter(|(k, init)| {
        ring.get_node(k).unwrap() != **init
    }).count();
    
    let pct = changes as f64 / 100.0;
    println!("\n[REDIST 4->5] {} keys changed ({:.1}%)", changes, pct);
    println!("[REDIST 4->5] Expected (consistent): ~20%");
    
    // Assert: should not have more than 40% redistribution (generous threshold)
    // True consistent hashing should be ~20%, but we allow up to 40% for CRC32 noise
    assert!(pct < 40.0, "HOTSPOT: Excessive redistribution ({:.1}%) on node add", pct);
}

#[test]
fn test_redistribution_on_node_add_10_to_11() {
    let mut ring = HashRing::new(HashRingConfig { virtual_nodes: 150 });
    for i in 0..10 {
        ring.add_node(node(&format!("node-{}", i)));
    }
    
    let keys: Vec<String> = (0..10000).map(|i| format!("key-{}", i)).collect();
    let initial: Vec<PoolId> = keys.iter().map(|k| ring.get_node(k).unwrap()).collect();
    
    ring.add_node(node("node-new"));
    
    let changes: usize = keys.iter().zip(initial.iter()).filter(|(k, init)| {
        ring.get_node(k).unwrap() != **init
    }).count();
    
    let pct = changes as f64 / 100.0;
    println!("\n[REDIST 10->11] {} keys changed ({:.1}%)", changes, pct);
    println!("[REDIST 10->11] Expected (consistent): ~9%");
    
    // With 10 nodes, should only be ~9% redistribution
    assert!(pct < 25.0, "HOTSPOT: Excessive redistribution ({:.1}%) on node add", pct);
}

#[test]
fn test_redistribution_on_node_remove() {
    let mut ring = HashRing::new(HashRingConfig { virtual_nodes: 150 });
    for i in 0..5 {
        ring.add_node(node(&format!("node-{}", i)));
    }
    
    let keys: Vec<String> = (0..10000).map(|i| format!("key-{}", i)).collect();
    let initial: Vec<PoolId> = keys.iter().map(|k| ring.get_node(k).unwrap()).collect();
    
    ring.remove_node(&PoolId::new("node-2"));
    
    let changes: usize = keys.iter().zip(initial.iter()).filter(|(k, init)| {
        ring.get_node(k).unwrap() != **init
    }).count();
    
    let pct = changes as f64 / 100.0;
    println!("\n[REDIST remove] {} keys changed ({:.1}%)", changes, pct);
    println!("[REDIST remove] Expected (consistent): ~20%");
    
    assert!(pct < 40.0, "HOTSPOT: Excessive redistribution ({:.1}%) on node remove", pct);
}

#[test]
fn test_distribution_evenness_after_changes() {
    let mut ring = HashRing::new(HashRingConfig { virtual_nodes: 150 });
    for i in 0..4 {
        ring.add_node(node(&format!("node-{}", i)));
    }
    
    let keys: Vec<String> = (0..10000).map(|i| format!("key-{}", i)).collect();
    let before_dist = get_distribution(&ring, &keys);
    
    ring.add_node(node("node-new"));
    
    let after_dist = get_distribution(&ring, &keys);
    
    let before_max = *before_dist.values().max().unwrap();
    let before_min = *before_dist.values().min().unwrap();
    let after_max = *after_dist.values().max().unwrap();
    let after_min = *after_dist.values().min().unwrap();
    
    let before_skew = before_max as f64 / before_min as f64;
    let after_skew = after_max as f64 / after_min as f64;
    
    println!("\n[DIST] Before: skew={:.2}, After: skew={:.2}", before_skew, after_skew);
    
    // Skew should remain reasonable (< 2.0) after adding a node
    assert!(after_skew < 2.5, "HOTSPOT: Distribution skew too high ({:.2}) after node add", after_skew);
}

#[test]
fn test_multiple_successive_adds() {
    let mut ring = HashRing::new(HashRingConfig { virtual_nodes: 150 });
    for i in 0..5 {
        ring.add_node(node(&format!("node-{}", i)));
    }
    
    let keys: Vec<String> = (0..10000).map(|i| format!("key-{}", i)).collect();
    
    // Add 3 more nodes one at a time
    for new_node in &["new-1", "new-2", "new-3"] {
        let before: Vec<PoolId> = keys.iter().map(|k| ring.get_node(k).unwrap()).collect();
        ring.add_node(node(new_node));
        let after: Vec<PoolId> = keys.iter().map(|k| ring.get_node(k).unwrap()).collect();
        
        let changes: usize = before.iter().zip(after.iter()).filter(|(b, a)| b != a).count();
        let pct = changes as f64 / 100.0;
        println!("\n[SUCCESSIVE] After adding {}: {} keys changed ({:.1}%)", new_node, changes, pct);
    }
}
