//! Proptests for Dag builder methods.

#![allow(clippy::unwrap_used)]
#![allow(deprecated)]

use proptest::prelude::*;

use crate::dag::{Dag, DagError, Workflow};
use crate::node_handle::NodeHandle;

prop_compose! {
    fn valid_node_name()(s in "[a-z][a-z0-9]{0,10}(-[a-z0-9]{1,5}){0,3}") -> String { s }
}

fn is_invalid_node_name_err<T>(result: &Result<T, DagError>) -> bool {
    matches!(result, Err(DagError::InvalidNodeName { .. }))
}

proptest! {
    #[test]
    fn dag_add_node_accepts_valid_name(name in valid_node_name()) {
        prop_assume!(!name.is_empty());
        let mut dag = Dag::new();
        let result: Result<NodeHandle<String, i32>, _> =
            dag.add_node(&name, |_input: String| -> i32 { 0 });
        prop_assert!(result.is_ok(), "valid name should be accepted: {:?}", result);
        prop_assert_eq!(dag.node_count(), 1);
    }

    #[test]
    fn dag_add_node_with_kind_accepts_valid_name(name in valid_node_name()) {
        prop_assume!(!name.is_empty());
        let mut dag = Dag::new();
        let result: Result<NodeHandle<String, i32>, _> =
            dag.add_node_with_kind(&name, vo_types::NodeKind::Pure, |_input: String| -> i32 { 0 });
        prop_assert!(result.is_ok());
        prop_assert_eq!(dag.node_count(), 1);
    }

    #[test]
    fn dag_add_node_rejects_empty_name(name in "") {
        let mut dag = Dag::new();
        let result: Result<NodeHandle<(), ()>, _> =
            dag.add_node(&name, |_input: ()| {});
        prop_assert!(is_invalid_node_name_err(&result));
    }

    #[test]
    fn dag_add_node_rejects_whitespace_name(name in " +") {
        let mut dag = Dag::new();
        let result: Result<NodeHandle<(), ()>, _> =
            dag.add_node(&name, |_input: ()| {});
        prop_assert!(is_invalid_node_name_err(&result));
    }

    #[test]
    fn dag_connect_two_valid_nodes_succeeds(
        name_a in valid_node_name(),
        name_b in valid_node_name()
    ) {
        prop_assume!(!name_a.is_empty() && !name_b.is_empty() && name_a != name_b);
        let mut dag = Dag::new();
        let a: NodeHandle<String, i32> = dag
            .add_node_with_kind(&name_a, vo_types::NodeKind::Pure, |_s: String| -> i32 { 0 })
            .unwrap();
        let b: NodeHandle<i32, bool> = dag
            .add_node_with_kind(&name_b, vo_types::NodeKind::ManagedEffect, |_i: i32| -> bool { true })
            .unwrap();
        let result = dag.connect(&a, &b);
        prop_assert!(result.is_ok());
        prop_assert_eq!(dag.edge_count(), 1);
    }

    #[test]
    fn dag_build_with_nodes_produces_spec(
        name_a in valid_node_name(),
        name_b in valid_node_name(),
        wf_name in valid_node_name()
    ) {
        prop_assume!(!name_a.is_empty() && !name_b.is_empty() && !wf_name.is_empty() && name_a != name_b);
        let mut dag = Dag::new();
        let a: NodeHandle<String, i32> = dag
            .add_node_with_kind(&name_a, vo_types::NodeKind::Pure, |_s: String| -> i32 { 0 })
            .unwrap();
        let b: NodeHandle<i32, bool> = dag
            .add_node_with_kind(&name_b, vo_types::NodeKind::ManagedEffect, |_i: i32| -> bool { true })
            .unwrap();
        dag.connect(&a, &b).unwrap();
        let spec = dag.build(&wf_name);
        prop_assert!(spec.is_ok());
        let spec = spec.unwrap();
        prop_assert_eq!(spec.nodes.len(), 2);
        prop_assert_eq!(spec.edges.len(), 1);
        prop_assert_eq!(spec.workflow_name.as_str(), wf_name);
    }

    #[test]
    fn dag_build_empty_always_errors(wf_name in valid_node_name()) {
        prop_assume!(!wf_name.is_empty());
        let dag = Dag::new();
        let result = dag.build(&wf_name);
        prop_assert!(matches!(result, Err(DagError::EmptyWorkflow)));
    }

    #[test]
    fn dag_build_rejects_empty_workflow_name(name in valid_node_name()) {
        prop_assume!(!name.is_empty());
        let mut dag = Dag::new();
        let _: NodeHandle<(), ()> = dag.add_node_with_kind(&name, vo_types::NodeKind::Pure, |_: ()| {}).unwrap();
        let result = dag.build("");
        prop_assert!(is_invalid_node_name_err(&result));
    }

    #[test]
    fn dag_multiple_connects_accumulate_edges(
        names in prop::collection::vec(valid_node_name(), 3..=8)
    ) {
        let unique: Vec<String> = {
            let mut seen = std::collections::HashSet::new();
            names.into_iter().filter(|n| seen.insert(n.clone())).collect()
        };
        prop_assume!(unique.len() >= 3 && unique.iter().all(|n| !n.is_empty()));
        let mut dag = Dag::new();
        let mut handles: Vec<NodeHandle<String, String>> = Vec::new();
        for name in &unique {
            handles.push(dag.add_node_with_kind(name, vo_types::NodeKind::Pure, |s: String| s).unwrap());
        }
        for i in 0..handles.len() - 1 {
            dag.connect(&handles[i], &handles[i + 1]).unwrap();
        }
        prop_assert_eq!(dag.edge_count(), handles.len() - 1);
        prop_assert_eq!(dag.node_count(), unique.len());
    }

    #[test]
    fn workflow_all_builder_methods_accept_valid_names(
        name in valid_node_name(),
        wf_name in valid_node_name()
    ) {
        prop_assume!(!name.is_empty() && !wf_name.is_empty());
        let mut wf = Workflow::new(&wf_name);
        let suffixes = ["p", "e", "w", "s", "u"];
        for suffix in suffixes {
            let node_name = format!("{}{}", name, suffix);
            let result: Result<NodeHandle<(), ()>, _> = match suffix {
                "p" => wf.pure(&node_name, |_| ()),
                "e" => wf.effect(&node_name, |_| ()),
                "w" => wf.wait(&node_name, |_| ()),
                "s" => wf.signal(&node_name, |_| ()),
                "u" => wf.unsafe_node(&node_name, |_| ()),
                _ => unreachable!(),
            };
            prop_assert!(result.is_ok(), "node {} should succeed", node_name);
        }
        let spec = wf.build();
        prop_assert!(spec.is_ok());
        prop_assert_eq!(spec.unwrap().nodes.len(), 5);
    }

    #[test]
    fn dag_build_result_is_acyclic(
        names in prop::collection::vec(valid_node_name(), 2..=10)
    ) {
        let unique: Vec<String> = {
            let mut seen = std::collections::HashSet::new();
            names.into_iter().filter(|n| seen.insert(n.clone())).collect()
        };
        prop_assume!(unique.len() >= 2 && unique.iter().all(|n| !n.is_empty()));
        let mut dag = Dag::new();
        let mut handles: Vec<NodeHandle<String, String>> = Vec::new();
        for name in &unique {
            handles.push(dag.add_node_with_kind(name, vo_types::NodeKind::Pure, |s: String| s).unwrap());
        }
        for i in 0..handles.len() - 1 {
            dag.connect(&handles[i], &handles[i + 1]).unwrap();
        }
        let spec = dag.build("test-workflow");
        prop_assert!(spec.is_ok(), "linear chain should build without cycles");
        let spec = spec.unwrap();
        let cycle = spec.detect_cycle();
        prop_assert!(cycle.is_none(), "built spec should not contain cycles: {:?}", cycle);
    }

    #[test]
    fn dag_connectivity_all_non_root_nodes_have_incoming_edge(
        names in prop::collection::vec(valid_node_name(), 2..=8)
    ) {
        let unique: Vec<String> = {
            let mut seen = std::collections::HashSet::new();
            names.into_iter().filter(|n| seen.insert(n.clone())).collect()
        };
        prop_assume!(unique.len() >= 2 && unique.iter().all(|n| !n.is_empty()));
        let mut dag = Dag::new();
        let mut handles: Vec<NodeHandle<String, String>> = Vec::new();
        for name in &unique {
            handles.push(dag.add_node_with_kind(name, vo_types::NodeKind::Pure, |s: String| s).unwrap());
        }
        for i in 0..handles.len() - 1 {
            dag.connect(&handles[i], &handles[i + 1]).unwrap();
        }
        let spec = dag.build("test-workflow");
        prop_assert!(spec.is_ok(), "connected DAG should build");
        let spec = spec.unwrap();
        let root_names: std::collections::HashSet<&str> = spec
            .edges
            .iter()
            .map(|e| e.to.as_str())
            .collect();
        for node in &spec.nodes {
            let has_incoming = root_names.contains(node.name.as_str());
            if node.name.as_str() == unique[0] {
                prop_assert!(!has_incoming, "first node should be root (no incoming edges)");
            } else {
                prop_assert!(has_incoming, "non-root node {} should have incoming edge", node.name.as_str());
            }
        }
    }

    #[test]
    fn dag_diamond_topology_builds_successfully(
        name in valid_node_name()
    ) {
        prop_assume!(!name.is_empty());
        let mut dag = Dag::new();
        let start: NodeHandle<(), i32> = dag
            .add_node_with_kind(&format!("{}-start", name), vo_types::NodeKind::Pure, |_: ()| -> i32 { 0 })
            .unwrap();
        let left: NodeHandle<i32, String> = dag
            .add_node_with_kind(&format!("{}-left", name), vo_types::NodeKind::Pure, |_i: i32| -> String { "l".to_string() })
            .unwrap();
        let right: NodeHandle<i32, String> = dag
            .add_node_with_kind(&format!("{}-right", name), vo_types::NodeKind::Pure, |_i: i32| -> String { "r".to_string() })
            .unwrap();
        let end: NodeHandle<String, ()> = dag
            .add_node_with_kind(&format!("{}-end", name), vo_types::NodeKind::Pure, |_s: String| {})
            .unwrap();
        dag.connect(&start, &left).unwrap();
        dag.connect(&start, &right).unwrap();
        dag.connect(&left, &end).unwrap();
        dag.connect(&right, &end).unwrap();
        let spec = dag.build(&format!("diamond-{}", name));
        prop_assert!(spec.is_ok(), "diamond DAG should build successfully");
        let spec = spec.unwrap();
        prop_assert_eq!(spec.nodes.len(), 4, "diamond should have 4 nodes");
        prop_assert_eq!(spec.edges.len(), 4, "diamond should have 4 edges");
        prop_assert!(spec.validate().is_ok(), "diamond spec should be valid");
    }

    #[test]
    fn dag_star_topology_builds_successfully(
        name in valid_node_name()
    ) {
        prop_assume!(!name.is_empty());
        let mut dag = Dag::new();
        let center: NodeHandle<(), i32> = dag
            .add_node_with_kind(&format!("{}-center", name), vo_types::NodeKind::Pure, |_: ()| -> i32 { 0 })
            .unwrap();
        let leaf1: NodeHandle<i32, ()> = dag
            .add_node_with_kind(&format!("{}-leaf1", name), vo_types::NodeKind::Pure, |_: i32| {})
            .unwrap();
        let leaf2: NodeHandle<i32, ()> = dag
            .add_node_with_kind(&format!("{}-leaf2", name), vo_types::NodeKind::Pure, |_: i32| {})
            .unwrap();
        let leaf3: NodeHandle<i32, ()> = dag
            .add_node_with_kind(&format!("{}-leaf3", name), vo_types::NodeKind::Pure, |_: i32| {})
            .unwrap();
        dag.connect(&center, &leaf1).unwrap();
        dag.connect(&center, &leaf2).unwrap();
        dag.connect(&center, &leaf3).unwrap();
        let spec = dag.build(&format!("star-{}", name));
        prop_assert!(spec.is_ok(), "star topology should build: {:?}", spec);
        let spec = spec.unwrap();
        prop_assert_eq!(spec.nodes.len(), 4, "star should have 4 nodes");
        prop_assert_eq!(spec.edges.len(), 3, "star should have 3 edges");
    }

    #[test]
    fn dag_chain_topology_builds_successfully(
        name in valid_node_name()
    ) {
        prop_assume!(!name.is_empty());
        let mut dag = Dag::new();
        let mut prev: NodeHandle<(), ()> = dag
            .add_node_with_kind(&format!("{}-0", name), vo_types::NodeKind::Pure, |_: ()| {})
            .unwrap();
        for i in 1..5 {
            let next: NodeHandle<(), ()> = dag
                .add_node_with_kind(&format!("{}-{}", name, i), vo_types::NodeKind::Pure, |_: ()| {})
                .unwrap();
            dag.connect(&prev, &next).unwrap();
            prev = next;
        }
        let spec = dag.build(&format!("chain-{}", name));
        prop_assert!(spec.is_ok(), "chain topology should build: {:?}", spec);
        let spec = spec.unwrap();
        prop_assert_eq!(spec.nodes.len(), 6, "chain should have 6 nodes");
        prop_assert_eq!(spec.edges.len(), 5, "chain should have 5 edges");
    }

    #[test]
    fn dag_empty_workflow_always_errors(wf_name in valid_node_name()) {
        prop_assume!(!wf_name.is_empty());
        let dag = Dag::new();
        let result = dag.build(&wf_name);
        prop_assert!(matches!(result, Err(DagError::EmptyWorkflow)));
    }

    #[test]
    fn dag_edge_count_equals_connect_calls(
        names in prop::collection::vec(valid_node_name(), 2..=6)
    ) {
        let unique: Vec<String> = {
            let mut seen = std::collections::HashSet::new();
            names.into_iter().filter(|n| seen.insert(n.clone())).collect()
        };
        prop_assume!(unique.len() >= 2);
        let mut dag = Dag::new();
        let mut handles: Vec<NodeHandle<(), ()>> = Vec::new();
        for name in &unique {
            handles.push(dag.add_node_with_kind(name, vo_types::NodeKind::Pure, |_: ()| {}).unwrap());
        }
        let connect_count = unique.len() - 1;
        for i in 0..connect_count {
            dag.connect(&handles[i], &handles[i + 1]).unwrap();
        }
        prop_assert_eq!(dag.edge_count(), connect_count);
    }

    #[test]
    fn dag_node_count_preserves_all_added_nodes(
        names in prop::collection::vec(valid_node_name(), 1..=10)
    ) {
        let unique: Vec<String> = {
            let mut seen = std::collections::HashSet::new();
            names.into_iter().filter(|n| seen.insert(n.clone())).collect()
        };
        prop_assume!(!unique.is_empty());
        let mut dag = Dag::new();
        for name in &unique {
            dag.add_node_with_kind(name, vo_types::NodeKind::Pure, |_: ()| {}).unwrap();
        }
        prop_assert_eq!(dag.node_count(), unique.len());
    }

    #[test]
    fn dag_build_preserves_all_node_kinds(
        name in valid_node_name()
    ) {
        prop_assume!(!name.is_empty());
        let mut dag = Dag::new();
        let kinds = [
            vo_types::NodeKind::Pure,
            vo_types::NodeKind::ManagedEffect,
            vo_types::NodeKind::Wait,
            vo_types::NodeKind::Signal,
            vo_types::NodeKind::Unsafe,
        ];
        for (i, &kind) in kinds.iter().enumerate() {
            dag.add_node_with_kind(&format!("{}-{}", name, i), kind, |_: ()| {}).unwrap();
        }
        let spec = dag.build(&format!("kinds-{}", name));
        prop_assert!(spec.is_ok(), "should build with all kinds: {:?}", spec);
        let spec = spec.unwrap();
        prop_assert_eq!(spec.nodes.len(), kinds.len());
        for (i, node) in spec.nodes.iter().enumerate() {
            prop_assert_eq!(node.kind, kinds[i], "kind mismatch at index {}", i);
        }
    }

    #[test]
    fn dag_single_node_workflow_builds_successfully(
        name in valid_node_name()
    ) {
        prop_assume!(!name.is_empty());
        let mut dag = Dag::new();
        dag.add_node_with_kind(&name, vo_types::NodeKind::Pure, |_: ()| {}).unwrap();
        let spec = dag.build(&format!("single-{}", name));
        prop_assert!(spec.is_ok(), "single node should build: {:?}", spec);
        let spec = spec.unwrap();
        prop_assert_eq!(spec.nodes.len(), 1);
        prop_assert_eq!(spec.edges.len(), 0);
    }
}
