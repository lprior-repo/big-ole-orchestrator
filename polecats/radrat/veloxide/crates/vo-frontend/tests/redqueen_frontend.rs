//! RED-QUEEN coevolutionary adversarial tests for vo-frontend.
//!
//! Attacks coevolve with defensive code. Each test targets an edge case where
//! a prior fix may regress: state corruption via config merges, Workflow name
//! abuse, badge class invariants, and parse boundary attacks.

use std::str::FromStr;
use vo_frontend::ui::domain_types::HttpMethod;
use vo_frontend::ui::graph::{node_kind_to_category, Node, NodeId, Workflow};

#[test]
fn config_update_overwrites_existing_key() {
    let mut node = Node::new(NodeId::new(), "x".into(), vo_types::NodeKind::Pure);
    node.apply_config_update(&serde_json::json!({"port": 8080}));
    node.apply_config_update(&serde_json::json!({"port": -1}));
    assert_eq!(
        node.config["port"], -1,
        "last write wins — attacker cannot pin stale value"
    );
}

#[test]
fn config_update_with_empty_object_is_noop() {
    let mut node = Node::new(NodeId::new(), "x".into(), vo_types::NodeKind::Pure);
    let before = node.config.clone();
    node.apply_config_update(&serde_json::json!({}));
    assert_eq!(node.config, before, "empty object merge must be identity");
}

#[test]
fn workflow_name_with_null_bytes_roundtrips() {
    let wf = Workflow::new("before\0after".into());
    let json = serde_json::to_string(&wf).unwrap();
    let recovered: Workflow = serde_json::from_str(&json).unwrap();
    assert!(
        recovered.name.contains('\0'),
        "null byte survives roundtrip — documented"
    );
}

#[test]
fn workflow_name_with_emoji_does_not_panic() {
    let mut wf = Workflow::new("🦀🔥💣".into());
    wf.add_node(Node::new(
        NodeId::new(),
        "node".into(),
        vo_types::NodeKind::Pure,
    ));
    let json = serde_json::to_string(&wf).unwrap();
    let _: Workflow = serde_json::from_str(&json).unwrap();
}

#[test]
fn badge_class_invariant_after_kind_flips() {
    let mut node = Node::new(NodeId::new(), "mutant".into(), vo_types::NodeKind::Pure);
    let kinds = [
        vo_types::NodeKind::Pure,
        vo_types::NodeKind::ManagedEffect,
        vo_types::NodeKind::Wait,
        vo_types::NodeKind::Signal,
        vo_types::NodeKind::Unsafe,
    ];
    for kind in kinds {
        node.set_kind(kind);
        let cat = node_kind_to_category(kind);
        assert_eq!(
            node.category, cat,
            "category must match kind after mutation"
        );
        assert!(!node.icon.is_empty(), "icon must never be empty");
        assert!(
            !cat.badge_class().is_empty(),
            "badge class must never be empty"
        );
    }
}

#[test]
fn nodeid_boundary_lengths() {
    assert!(
        NodeId::parse("01ARYZ6S41TSV4RRFFQ69G5FAV").is_some(),
        "26 chars accepted"
    );
    assert!(
        NodeId::parse("01ARYZ6S41TSV4RRFFQ69G5FA").is_none(),
        "25 chars rejected"
    );
    assert!(
        NodeId::parse("01ARYZ6S41TSV4RRFFQ69G5FAVG").is_none(),
        "27 chars rejected"
    );
}

#[test]
fn http_method_from_str_rejects_unknown() {
    assert!(
        HttpMethod::from_str("EVIL").is_err(),
        "unknown method must be rejected"
    );
    assert!(
        HttpMethod::from_str("").is_err(),
        "empty string must be rejected"
    );
    assert!(
        HttpMethod::from_str("get").is_ok(),
        "lowercase get must be accepted"
    );
}
