//! Adversarial tests for vo-sdk (bead ve-z32z).
//!
//! DIMENSION: NodeHandle property verification.

use std::collections::HashMap;

use crate::node_handle::NodeHandle;
use vo_types::NodeName;

#[test]
fn node_handle_equality_is_name_based() {
    let h1: NodeHandle<String, i32> = NodeHandle::new(NodeName::parse("same").unwrap());
    let h2: NodeHandle<String, i32> = NodeHandle::new(NodeName::parse("same").unwrap());

    assert_eq!(h1, h2, "same name should be equal");
}

#[test]
fn node_handle_hash_consistent_with_equality() {
    let h1: NodeHandle<String, i32> = NodeHandle::new(NodeName::parse("key").unwrap());
    let h2: NodeHandle<String, i32> = NodeHandle::new(NodeName::parse("key").unwrap());

    let mut map = HashMap::new();
    map.insert(h1, 42);

    assert_eq!(
        map.get(&h2),
        Some(&42),
        "same-name handle should hash to same bucket"
    );
}

#[test]
fn node_handle_inequality_different_names() {
    let h1: NodeHandle<(), ()> = NodeHandle::new(NodeName::parse("alpha").unwrap());
    let h2: NodeHandle<(), ()> = NodeHandle::new(NodeName::parse("beta").unwrap());

    assert_ne!(h1, h2);
}