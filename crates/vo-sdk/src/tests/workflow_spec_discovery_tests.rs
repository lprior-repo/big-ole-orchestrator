//! Comprehensive tests for WorkflowSpec validation and discovery (ADR-003/017/022/031).
//!
//! This module provides end-to-end test coverage for:
//! - WorkflowSpec model: valid specs accepted, invalid specs rejected
//! - Discovery validation: version compatibility, schema evolution, upgrade path
//! - NodeKind support: Pure, ManagedEffect, Wait, Signal, Unsafe constraints

use crate::dag::{Dag, DagError, Workflow};
use vo_types::discovery::{
    enforce_pin, validate_discovery_path, DiscoveryPath, DiscoveryPathError, PinEnforcementError,
    VersionConstraint, VersionPin,
};
use vo_types::{BinaryHash, NodeKind, MAX_SUPPORTED_SCHEMA_VERSION};

#[cfg(test)]
mod workflow_spec_tests {
    use super::*;

    #[test]
    fn valid_workflow_spec_with_all_node_kinds_is_accepted() {
        let mut wf = Workflow::new("full_kinds_workflow");
        let _pure = wf.pure("pure_node", |_: ()| ()).expect("valid");
        let _effect = wf.effect("effect_node", |_: ()| ()).expect("valid");
        let _wait = wf.wait("wait_node", |_: ()| ()).expect("valid");
        let _signal = wf.signal("signal_node", |_: ()| ()).expect("valid");
        let _unsafe = wf.unsafe_node("unsafe_node", |_: ()| ()).expect("valid");

        let spec = wf.build().expect("valid workflow should build");
        assert_eq!(spec.workflow_name.as_str(), "full_kinds_workflow");
        assert_eq!(spec.nodes.len(), 5);

        let kinds: Vec<NodeKind> = spec.nodes.iter().map(|n| n.kind).collect();
        assert!(kinds.contains(&NodeKind::Pure));
        assert!(kinds.contains(&NodeKind::ManagedEffect));
        assert!(kinds.contains(&NodeKind::Wait));
        assert!(kinds.contains(&NodeKind::Signal));
        assert!(kinds.contains(&NodeKind::Unsafe));
    }

    #[test]
    fn empty_workflow_is_rejected() {
        let wf = Workflow::new("empty_workflow");
        let result = wf.build();
        assert!(matches!(result, Err(DagError::EmptyWorkflow)));
    }

    #[test]
    fn workflow_with_single_node_is_accepted() {
        let mut wf = Workflow::new("single_node_workflow");
        let _node = wf.pure("only", |_: ()| ()).expect("valid");
        let spec = wf.build().expect("single node should build");
        assert_eq!(spec.nodes.len(), 1);
    }

    #[test]
    fn workflow_spec_serializes_with_correct_schema() {
        let mut wf = Workflow::new("test_workflow");
        let _node = wf.pure("step", |_: String| i32::default()).expect("valid");
        let spec = wf.build().expect("build should succeed");

        let json = spec.to_json_bytes();
        let json_str = String::from_utf8(json).expect("json should be valid utf8");

        assert!(json_str.contains("\"workflow_name\""));
        assert!(json_str.contains("\"nodes\""));
        assert!(json_str.contains("\"edges\""));
        assert!(json_str.contains("\"name\":\"step\""));
        assert!(json_str.contains("\"kind\":\"pure\""));
    }

    #[test]
    fn dag_rejects_self_loop_cycle() {
        let mut dag = Dag::new();
        let node: crate::node_handle::NodeHandle<(), ()> = dag
            .add_node_with_kind("self-loop", NodeKind::Pure, |_i: ()| ())
            .expect("valid");
        dag.connect(&node, &node).expect("connect should succeed");
        let result = dag.build("cyclic");
        assert!(matches!(result, Err(DagError::CycleDetected)));
    }

    #[test]
    fn dag_rejects_two_node_cycle() {
        let mut dag = Dag::new();
        let a: crate::node_handle::NodeHandle<(), ()> = dag
            .add_node_with_kind("a", NodeKind::Pure, |_i: ()| ())
            .expect("valid");
        let b: crate::node_handle::NodeHandle<(), ()> = dag
            .add_node_with_kind("b", NodeKind::Pure, |_i: ()| ())
            .expect("valid");
        dag.connect(&a, &b).expect("connect a->b");
        dag.connect(&b, &a).expect("connect b->a");
        let result = dag.build("two_node_cycle");
        assert!(matches!(result, Err(DagError::CycleDetected)));
    }

    #[test]
    fn dag_accepts_linear_chain() {
        let mut dag = Dag::new();
        let a: crate::node_handle::NodeHandle<(), ()> = dag
            .add_node_with_kind("a", NodeKind::Pure, |_i: ()| ())
            .expect("valid");
        let b: crate::node_handle::NodeHandle<(), ()> = dag
            .add_node_with_kind("b", NodeKind::Pure, |_i: ()| ())
            .expect("valid");
        let c: crate::node_handle::NodeHandle<(), ()> = dag
            .add_node_with_kind("c", NodeKind::Pure, |_i: ()| ())
            .expect("valid");
        dag.connect(&a, &b).expect("connect a->b");
        dag.connect(&b, &c).expect("connect b->c");
        let result = dag.build("linear");
        assert!(result.is_ok(), "linear chain should not be a cycle");
    }

    #[test]
    fn dag_accepts_diamond_graph() {
        let mut dag = Dag::new();
        let start: crate::node_handle::NodeHandle<(), ()> = dag
            .add_node_with_kind("start", NodeKind::Pure, |_i: ()| ())
            .expect("valid");
        let left: crate::node_handle::NodeHandle<(), ()> = dag
            .add_node_with_kind("left", NodeKind::Pure, |_i: ()| ())
            .expect("valid");
        let right: crate::node_handle::NodeHandle<(), ()> = dag
            .add_node_with_kind("right", NodeKind::Pure, |_i: ()| ())
            .expect("valid");
        let end: crate::node_handle::NodeHandle<(), ()> = dag
            .add_node_with_kind("end", NodeKind::Pure, |_i: ()| ())
            .expect("valid");
        dag.connect(&start, &left).expect("connect start->left");
        dag.connect(&start, &right).expect("connect start->right");
        dag.connect(&left, &end).expect("connect left->end");
        dag.connect(&right, &end).expect("connect right->end");
        let result = dag.build("diamond");
        assert!(result.is_ok(), "diamond graph should not be a cycle");
    }
}

#[cfg(test)]
mod discovery_validation_tests {
    use super::*;

    #[test]
    fn discovery_path_parses_valid_path() {
        let path = DiscoveryPath::parse("/var/wtf/versions/abcdef0123456789/my-binary").unwrap();
        assert_eq!(
            path.binary_hash(),
            &BinaryHash::parse("abcdef0123456789").unwrap()
        );
        assert_eq!(path.binary_name(), "my-binary");
    }

    #[test]
    fn discovery_path_parses_with_file_prefix() {
        let path =
            DiscoveryPath::parse("file:///var/wtf/versions/abcdef0123456789/my-binary").unwrap();
        assert_eq!(
            path.binary_hash(),
            &BinaryHash::parse("abcdef0123456789").unwrap()
        );
    }

    #[test]
    fn discovery_path_rejects_invalid_prefix() {
        let result = DiscoveryPath::parse("/other/path/abcdef0123456789/binary");
        assert!(matches!(
            result,
            Err(DiscoveryPathError::InvalidFormat { .. })
        ));
    }

    #[test]
    fn discovery_path_rejects_invalid_hash() {
        let result = DiscoveryPath::parse("/var/wtf/versions/notahext/binary");
        assert!(matches!(result, Err(DiscoveryPathError::InvalidHash(_))));
    }

    #[test]
    fn validate_discovery_path_rejects_empty_name() {
        let path = DiscoveryPath::new(
            "/var/wtf/versions".to_string(),
            BinaryHash::parse("abcdef0123456789").unwrap(),
            String::new(),
        );
        let result = validate_discovery_path(&path);
        assert!(matches!(
            result,
            Err(DiscoveryPathError::InvalidFormat { .. })
        ));
    }

    #[test]
    fn validate_discovery_path_rejects_name_with_separator() {
        let path = DiscoveryPath::new(
            "/var/wtf/versions".to_string(),
            BinaryHash::parse("abcdef0123456789").unwrap(),
            "foo/bar".to_string(),
        );
        let result = validate_discovery_path(&path);
        assert!(matches!(
            result,
            Err(DiscoveryPathError::InvalidFormat { .. })
        ));
    }

    #[test]
    fn discovery_path_display_shows_full_path() {
        let path = DiscoveryPath::parse("/var/wtf/versions/abcdef0123456789/my-binary").unwrap();
        assert_eq!(
            path.to_string(),
            "/var/wtf/versions/abcdef0123456789/my-binary"
        );
    }
}

#[cfg(test)]
mod version_constraint_tests {
    use super::*;

    #[test]
    fn version_constraint_exact_matches_identical_hash() {
        let hash = BinaryHash::parse("abcdef0123456789").unwrap();
        let constraint = VersionConstraint::Exact;
        assert!(constraint.matches(&hash, &hash));
    }

    #[test]
    fn version_constraint_exact_rejects_different_hash() {
        let hash1 = BinaryHash::parse("abcdef0123456789").unwrap();
        let hash2 = BinaryHash::parse("1234567890abcdef").unwrap();
        let constraint = VersionConstraint::Exact;
        assert!(!constraint.matches(&hash1, &hash2));
    }

    #[test]
    fn version_constraint_compatible_matches_same_prefix() {
        let hash1 = BinaryHash::parse("abcdef0123456789").unwrap();
        let hash2 = BinaryHash::parse("abcdef01deadbeef").unwrap();
        let constraint = VersionConstraint::Compatible;
        assert!(constraint.matches(&hash1, &hash2));
    }

    #[test]
    fn version_constraint_compatible_rejects_different_prefix() {
        let hash1 = BinaryHash::parse("abcdef0123456789").unwrap();
        let hash2 = BinaryHash::parse("1234567890abcdef").unwrap();
        let constraint = VersionConstraint::Compatible;
        assert!(!constraint.matches(&hash1, &hash2));
    }

    #[test]
    fn version_constraint_latest_always_matches() {
        let hash1 = BinaryHash::parse("abcdef0123456789").unwrap();
        let hash2 = BinaryHash::parse("1234567890abcdef").unwrap();
        let constraint = VersionConstraint::Latest;
        assert!(constraint.matches(&hash1, &hash2));
    }
}

#[cfg(test)]
mod version_pinning_tests {
    use super::*;

    #[test]
    fn enforce_pin_succeeds_when_hash_matches() {
        let hash = BinaryHash::parse("abcdef0123456789").unwrap();
        let pin = VersionPin::new(hash.clone(), 1000);
        enforce_pin(&pin, &hash).expect("pin should enforce when hash matches");
    }

    #[test]
    fn enforce_pin_fails_when_hash_mismatches() {
        let hash1 = BinaryHash::parse("abcdef0123456789").unwrap();
        let hash2 = BinaryHash::parse("1234567890abcdef").unwrap();
        let pin = VersionPin::new(hash1, 1000);
        let result = enforce_pin(&pin, &hash2);
        assert!(matches!(
            result,
            Err(PinEnforcementError::HashMismatch { .. })
        ));
    }

    #[test]
    fn version_pin_preserves_pin_hash_and_timestamp() {
        let hash = BinaryHash::parse("abcdef0123456789").unwrap();
        let pin = VersionPin::new(hash.clone(), 1700000000000);
        assert_eq!(pin.pin_hash(), &hash);
        assert_eq!(pin.pinned_at_ms(), 1700000000000);
    }
}

#[cfg(test)]
mod schema_evolution_tests {
    use super::*;

    #[test]
    fn extract_schema_version_accepts_version_zero() {
        let payload = serde_json::json!({ "version": 0 });
        let result = vo_types::extract_schema_version(&payload, None);
        assert_eq!(result, Ok(0));
    }

    #[test]
    fn extract_schema_version_accepts_version_one() {
        let payload = serde_json::json!({ "version": 1 });
        let result = vo_types::extract_schema_version(&payload, None);
        assert_eq!(result, Ok(1));
    }

    #[test]
    fn extract_schema_version_rejects_future_version() {
        let payload = serde_json::json!({ "version": 2 });
        let result = vo_types::extract_schema_version(&payload, None);
        assert!(result.is_err());
    }

    #[test]
    fn extract_schema_version_rejects_string_version() {
        let payload = serde_json::json!({ "version": "1" });
        let result = vo_types::extract_schema_version(&payload, None);
        assert!(result.is_err());
    }

    #[test]
    fn extract_schema_version_rejects_negative_version() {
        let payload = serde_json::json!({ "version": -1 });
        let result = vo_types::extract_schema_version(&payload, None);
        assert!(result.is_err());
    }

    #[test]
    fn extract_schema_version_rejects_missing_version_without_fallback() {
        let payload = serde_json::json!({});
        let result = vo_types::extract_schema_version(&payload, None);
        assert!(result.is_err());
    }

    #[test]
    fn extract_schema_version_uses_fallback_when_version_missing() {
        let payload = serde_json::json!({});
        let result = vo_types::extract_schema_version(&payload, Some(0));
        assert_eq!(result, Ok(0));
    }

    #[test]
    fn extract_schema_version_prioritizes_payload_over_fallback() {
        let payload = serde_json::json!({ "version": 1 });
        let result = vo_types::extract_schema_version(&payload, Some(0));
        assert_eq!(result, Ok(1));
    }

    #[test]
    fn workflow_spec_defaults_to_max_supported_version() {
        let spec = vo_types::WorkflowSpec::default();
        assert_eq!(spec.version(), MAX_SUPPORTED_SCHEMA_VERSION);
    }

    #[test]
    fn workflow_spec_allows_explicit_version() {
        let spec = vo_types::WorkflowSpec { version: 0 };
        assert_eq!(spec.version(), 0);
    }
}

#[cfg(test)]
mod node_kind_tests {
    use super::*;

    #[test]
    fn node_kind_all_variants_returns_five_kinds() {
        let variants = NodeKind::all_variants();
        assert_eq!(variants.len(), 5);
    }

    #[test]
    fn node_kind_pure_serializes_to_snake_case() {
        let json = serde_json::to_string(&NodeKind::Pure).unwrap();
        assert_eq!(json, "\"pure\"");
    }

    #[test]
    fn node_kind_managed_effect_serializes_to_snake_case() {
        let json = serde_json::to_string(&NodeKind::ManagedEffect).unwrap();
        assert_eq!(json, "\"managed_effect\"");
    }

    #[test]
    fn node_kind_wait_serializes_to_snake_case() {
        let json = serde_json::to_string(&NodeKind::Wait).unwrap();
        assert_eq!(json, "\"wait\"");
    }

    #[test]
    fn node_kind_signal_serializes_to_snake_case() {
        let json = serde_json::to_string(&NodeKind::Signal).unwrap();
        assert_eq!(json, "\"signal\"");
    }

    #[test]
    fn node_kind_unsafe_serializes_to_snake_case() {
        let json = serde_json::to_string(&NodeKind::Unsafe).unwrap();
        assert_eq!(json, "\"unsafe\"");
    }

    #[test]
    fn node_kind_all_variants_round_trip_via_serde() {
        for variant in NodeKind::all_variants() {
            let json = serde_json::to_string(variant).unwrap();
            let recovered: NodeKind = serde_json::from_str(&json).unwrap();
            assert_eq!(recovered, *variant);
        }
    }

    #[test]
    fn node_kind_rejects_unknown_variant() {
        let result: Result<NodeKind, _> = serde_json::from_str("\"nonexistent\"");
        assert!(result.is_err());
    }

    #[test]
    fn dag_build_with_kind_produces_correct_node_kinds() {
        let mut dag = Dag::new();
        let _pure_node: crate::node_handle::NodeHandle<String, i32> = dag
            .add_node_with_kind("pure-task", NodeKind::Pure, |_i: String| -> i32 { 0 })
            .expect("valid");
        let _effect_node: crate::node_handle::NodeHandle<i32, bool> = dag
            .add_node_with_kind("effect-task", NodeKind::ManagedEffect, |_i: i32| -> bool {
                true
            })
            .expect("valid");
        let _wait_node: crate::node_handle::NodeHandle<bool, ()> = dag
            .add_node_with_kind("wait-task", NodeKind::Wait, |_i: bool| -> () { () })
            .expect("valid");
        let _signal_node: crate::node_handle::NodeHandle<(), String> = dag
            .add_node_with_kind("signal-task", NodeKind::Signal, |_i: ()| -> String {
                String::new()
            })
            .expect("valid");
        let _unsafe_node: crate::node_handle::NodeHandle<String, ()> = dag
            .add_node_with_kind("unsafe-task", NodeKind::Unsafe, |_i: String| -> () { () })
            .expect("valid");

        let spec = dag.build("kinds_test").expect("build should succeed");

        let pure = spec
            .nodes
            .iter()
            .find(|n| n.name.as_str() == "pure-task")
            .unwrap();
        let effect = spec
            .nodes
            .iter()
            .find(|n| n.name.as_str() == "effect-task")
            .unwrap();
        let wait = spec
            .nodes
            .iter()
            .find(|n| n.name.as_str() == "wait-task")
            .unwrap();
        let signal = spec
            .nodes
            .iter()
            .find(|n| n.name.as_str() == "signal-task")
            .unwrap();
        let unsafe_node = spec
            .nodes
            .iter()
            .find(|n| n.name.as_str() == "unsafe-task")
            .unwrap();

        assert_eq!(pure.kind, NodeKind::Pure);
        assert_eq!(effect.kind, NodeKind::ManagedEffect);
        assert_eq!(wait.kind, NodeKind::Wait);
        assert_eq!(signal.kind, NodeKind::Signal);
        assert_eq!(unsafe_node.kind, NodeKind::Unsafe);
    }

    #[test]
    fn workflow_builder_pure_creates_pure_node() {
        let mut wf = Workflow::new("pure_test");
        let _handle: crate::node_handle::NodeHandle<i32, String> =
            wf.pure("to_string", |i: i32| i.to_string()).expect("valid");
        let spec = wf.build().expect("build should succeed");
        let node = spec
            .nodes
            .iter()
            .find(|n| n.name.as_str() == "to_string")
            .unwrap();
        assert_eq!(node.kind, NodeKind::Pure);
    }

    #[test]
    fn workflow_builder_effect_creates_managed_effect_node() {
        let mut wf = Workflow::new("effect_test");
        let _handle: crate::node_handle::NodeHandle<String, ()> = wf
            .effect("persist", |s: String| {
                let _ = s;
            })
            .expect("valid");
        let spec = wf.build().expect("build should succeed");
        let node = spec
            .nodes
            .iter()
            .find(|n| n.name.as_str() == "persist")
            .unwrap();
        assert_eq!(node.kind, NodeKind::ManagedEffect);
    }

    #[test]
    fn workflow_builder_wait_creates_wait_node() {
        let mut wf = Workflow::new("wait_test");
        let _handle: crate::node_handle::NodeHandle<(), bool> =
            wf.wait("await_signal", |_: ()| true).expect("valid");
        let spec = wf.build().expect("build should succeed");
        let node = spec
            .nodes
            .iter()
            .find(|n| n.name.as_str() == "await_signal")
            .unwrap();
        assert_eq!(node.kind, NodeKind::Wait);
    }

    #[test]
    fn workflow_builder_signal_creates_signal_node() {
        let mut wf = Workflow::new("signal_test");
        let _handle: crate::node_handle::NodeHandle<String, ()> = wf
            .signal("notify", |s: String| {
                let _ = s;
            })
            .expect("valid");
        let spec = wf.build().expect("build should succeed");
        let node = spec
            .nodes
            .iter()
            .find(|n| n.name.as_str() == "notify")
            .unwrap();
        assert_eq!(node.kind, NodeKind::Signal);
    }

    #[test]
    fn workflow_builder_unsafe_creates_unsafe_node() {
        let mut wf = Workflow::new("unsafe_test");
        let _handle: crate::node_handle::NodeHandle<(), ()> =
            wf.unsafe_node("fire", |_: ()| ()).expect("valid");
        let spec = wf.build().expect("build should succeed");
        let node = spec
            .nodes
            .iter()
            .find(|n| n.name.as_str() == "fire")
            .unwrap();
        assert_eq!(node.kind, NodeKind::Unsafe);
    }
}
