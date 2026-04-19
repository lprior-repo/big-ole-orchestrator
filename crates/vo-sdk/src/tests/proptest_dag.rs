//! Proptests for Dag builder methods.

#![allow(clippy::unwrap_used)]

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
    fn dag_build_accepts_linear_chain(
        names in prop::collection::vec(valid_node_name(), 3..=10)
    ) {
        prop_assume!(!names.is_empty() && names.iter().all(|n| !n.is_empty()));
        let unique: Vec<String> = {
            let mut seen = std::collections::HashSet::new();
            names.into_iter().filter(|n| seen.insert(n.clone())).collect()
        };
        prop_assume!(unique.len() >= 3);
        let mut dag = Dag::new();
        let mut handles: Vec<NodeHandle<String, String>> = Vec::new();
        for name in &unique {
            handles.push(dag.add_node_with_kind(name, vo_types::NodeKind::Pure, |s: String| s).unwrap());
        }
        for i in 0..handles.len() - 1 {
            dag.connect(&handles[i], &handles[i + 1]).unwrap();
        }
        let result = dag.build("linear_chain");
        prop_assert!(result.is_ok(), "linear chain should build successfully: {:?}", result);
        let spec = result.unwrap();
        prop_assert_eq!(spec.nodes.len(), unique.len());
        prop_assert_eq!(spec.edges.len(), unique.len() - 1);
    }

    #[test]
    fn dag_build_rejects_self_referential_node(name in valid_node_name()) {
        prop_assume!(!name.is_empty());
        let mut dag = Dag::new();
        let handle: NodeHandle<String, String> = dag
            .add_node_with_kind(&name, vo_types::NodeKind::Pure, |s: String| s)
            .unwrap();
        dag.connect(&handle, &handle).unwrap();
        let result = dag.build("self_loop");
        prop_assert!(matches!(result, Err(DagError::CycleDetected { .. })),
            "self-referential node should be rejected: {:?}", result);
    }

    #[test]
    fn dag_build_rejects_mutual_dependency(
        name_a in valid_node_name(),
        name_b in valid_node_name()
    ) {
        prop_assume!(!name_a.is_empty() && !name_b.is_empty() && name_a != name_b);
        let mut dag = Dag::new();
        let a: NodeHandle<String, String> = dag
            .add_node_with_kind(&name_a, vo_types::NodeKind::Pure, |s: String| s)
            .unwrap();
        let b: NodeHandle<String, String> = dag
            .add_node_with_kind(&name_b, vo_types::NodeKind::Pure, |s: String| s)
            .unwrap();
        dag.connect(&a, &b).unwrap();
        dag.connect(&b, &a).unwrap();
        let result = dag.build("mutual_dep");
        prop_assert!(matches!(result, Err(DagError::CycleDetected { .. })),
            "mutual dependency A->B->A should be rejected: {:?}", result);
    }

    #[test]
    fn dag_build_rejects_triangle_cycle(
        name_a in valid_node_name(),
        name_b in valid_node_name(),
        name_c in valid_node_name()
    ) {
        prop_assume!(!name_a.is_empty() && !name_b.is_empty() && !name_c.is_empty());
        prop_assume!(name_a != name_b && name_b != name_c && name_a != name_c);
        let mut dag = Dag::new();
        let a: NodeHandle<String, String> = dag
            .add_node_with_kind(&name_a, vo_types::NodeKind::Pure, |s: String| s)
            .unwrap();
        let b: NodeHandle<String, String> = dag
            .add_node_with_kind(&name_b, vo_types::NodeKind::Pure, |s: String| s)
            .unwrap();
        let c: NodeHandle<String, String> = dag
            .add_node_with_kind(&name_c, vo_types::NodeKind::Pure, |s: String| s)
            .unwrap();
        dag.connect(&a, &b).unwrap();
        dag.connect(&b, &c).unwrap();
        dag.connect(&c, &a).unwrap();
        let result = dag.build("triangle");
        prop_assert!(matches!(result, Err(DagError::CycleDetected { .. })),
            "triangle cycle A->B->C->A should be rejected: {:?}", result);
    }

    #[test]
    fn dag_build_rejects_multiple_disconnected_cycles(
        names in prop::collection::vec(valid_node_name(), 6..=12)
    ) {
        let unique: Vec<String> = {
            let mut seen = std::collections::HashSet::new();
            names.into_iter().filter(|n| seen.insert(n.clone())).collect()
        };
        prop_assume!(unique.len() >= 6);
        let first_six: Vec<String> = unique[..6].to_vec();
        prop_assume!(first_six.iter().all(|n| !n.is_empty()));
        prop_assume!(first_six[0] != first_six[1] && first_six[1] != first_six[2]
            && first_six[0] != first_six[2]
            && first_six[3] != first_six[4] && first_six[4] != first_six[5]
            && first_six[3] != first_six[5]);
        let mut dag = Dag::new();
        let a: NodeHandle<String, String> = dag
            .add_node_with_kind(&first_six[0], vo_types::NodeKind::Pure, |s: String| s)
            .unwrap();
        let b: NodeHandle<String, String> = dag
            .add_node_with_kind(&first_six[1], vo_types::NodeKind::Pure, |s: String| s)
            .unwrap();
        let c: NodeHandle<String, String> = dag
            .add_node_with_kind(&first_six[2], vo_types::NodeKind::Pure, |s: String| s)
            .unwrap();
        let x: NodeHandle<String, String> = dag
            .add_node_with_kind(&first_six[3], vo_types::NodeKind::Pure, |s: String| s)
            .unwrap();
        let y: NodeHandle<String, String> = dag
            .add_node_with_kind(&first_six[4], vo_types::NodeKind::Pure, |s: String| s)
            .unwrap();
        let z: NodeHandle<String, String> = dag
            .add_node_with_kind(&first_six[5], vo_types::NodeKind::Pure, |s: String| s)
            .unwrap();
        dag.connect(&a, &b).unwrap();
        dag.connect(&b, &c).unwrap();
        dag.connect(&c, &a).unwrap();
        dag.connect(&x, &y).unwrap();
        dag.connect(&y, &z).unwrap();
        dag.connect(&z, &x).unwrap();
        let result = dag.build("disconnected_cycles");
        prop_assert!(matches!(result, Err(DagError::CycleDetected { .. })),
            "multiple disconnected cycles should be rejected: {:?}", result);
    }

    #[test]
    fn dag_build_accepts_large_acyclic_graph(
        names in prop::collection::vec(valid_node_name(), 50..=100)
    ) {
        let unique: Vec<String> = {
            let mut seen = std::collections::HashSet::new();
            names.into_iter().filter(|n| seen.insert(n.clone())).collect()
        };
        prop_assume!(unique.len() >= 50);
        prop_assume!(unique.iter().all(|n| !n.is_empty()));
        let mut dag = Dag::new();
        let mut handles: Vec<NodeHandle<String, String>> = Vec::new();
        for name in &unique {
            handles.push(dag.add_node_with_kind(name, vo_types::NodeKind::Pure, |s: String| s).unwrap());
        }
        for i in 0..handles.len() - 1 {
            dag.connect(&handles[i], &handles[i + 1]).unwrap();
        }
        let result = dag.build("large_acyclic");
        prop_assert!(result.is_ok(), "large acyclic graph should build: {:?}", result);
        let spec = result.unwrap();
        prop_assert_eq!(spec.nodes.len(), unique.len());
        prop_assert_eq!(spec.edges.len(), unique.len() - 1);
    }

    #[test]
    fn dag_build_accepts_100_plus_node_linear_chain(
        names in prop::collection::vec(valid_node_name(), 100..=150)
    ) {
        let unique: Vec<String> = {
            let mut seen = std::collections::HashSet::new();
            names.into_iter().filter(|n| seen.insert(n.clone())).collect()
        };
        prop_assume!(unique.len() >= 100);
        prop_assume!(unique.iter().all(|n| !n.is_empty()));
        let mut dag = Dag::new();
        let mut handles: Vec<NodeHandle<String, String>> = Vec::new();
        for name in &unique {
            handles.push(dag.add_node_with_kind(name, vo_types::NodeKind::Pure, |s: String| s).unwrap());
        }
        for i in 0..handles.len() - 1 {
            dag.connect(&handles[i], &handles[i + 1]).unwrap();
        }
        let result = dag.build("hundred_plus_chain");
        prop_assert!(result.is_ok(), "100+ node linear chain should build: {:?}", result);
        let spec = result.unwrap();
        prop_assert_eq!(spec.nodes.len(), unique.len());
        prop_assert_eq!(spec.edges.len(), unique.len() - 1);
    }
}
