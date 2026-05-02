//! BDD tests for ADR-010: Compile-Time DAG Type Safety
//!
//! This module contains Behavior-Driven Development tests that verify the
//! compile-time type safety guarantees described in ADR-010.
//!
//! ## Type Safety Contract
//!
//! The `connect<T>` method enforces type safety at compile time via its signature:
//! ```ignore
//! pub fn connect<T>(
//!     &mut self,
//!     from: &NodeHandle<impl Any, T>,
//!     to: &NodeHandle<T, impl Any>,
//! ) -> Result<(), DagError>
//! ```
//!
//! This means:
//! - `from` outputs type `T`
//! - `to` inputs type `T`
//! - If the types don't match, the code will NOT COMPILE
//!
//! ## BDD Scenarios (Given-When-Then)
//!
//! ### Happy Path: Type-Compatible Connections
//!
//! ```gherkin
//! Feature: Compile-Time Type Safety
//!
//!   Scenario: Valid DAG with type-safe connections compiles successfully
//!     Given a workflow with typed nodes
//!     When nodes with matching input/output types are connected
//!     Then the DAG builds successfully
//! ```
//!
//! ### Error Path: Type-Incompatible Connections
//!
//! ```gherkin
//!   Scenario: Type-incompatible connection is rejected at compile time
//!     Given a workflow with typed nodes
//!     When attempting to connect nodes with mismatched types
//!     Then the code fails to compile with a type error
//! ```

use vo_sdk::dag::{Dag, DagError};
use vo_sdk::node_handle::NodeHandle;
use vo_sdk::Workflow;
use vo_types::NodeKind;

#[cfg(test)]
mod bdd_tests {
    use super::*;

    mod given_when_then {
        use super::*;

        #[test]
        fn scenario_valid_dag_with_type_safe_connections_compiles() {
            // Given: A workflow with typed nodes
            // When: Nodes with matching input/output types are connected
            // Then: The DAG builds successfully

            let mut dag = Dag::new();

            // validate: Order -> ValidatedOrder
            let validate: NodeHandle<String, bool> = dag
                .add_node_with_kind(
                    "validate",
                    NodeKind::Pure,
                    |input: String| !input.is_empty(),
                )
                .expect("valid node creation");

            // charge: bool -> Receipt
            let charge: NodeHandle<bool, String> = dag
                .add_node_with_kind(
                    "charge",
                    NodeKind::ManagedEffect,
                    |valid: bool| {
                        if valid {
                            "receipt_123".to_string()
                        } else {
                            "failed".to_string()
                        }
                    },
                )
                .expect("valid node creation");

            // When: Type-compatible connection is made
            dag.connect(&validate, &charge).expect("connect should succeed");

            // Then: The DAG builds successfully
            let spec = dag.build("checkout").expect("build should succeed");
            assert_eq!(spec.nodes.len(), 2);
            assert_eq!(spec.edges.len(), 1);
        }

        #[test]
        fn scenario_type_safe_workflow_compiles_and_builds() {
            // Given: A typed workflow chain (String -> bool -> String -> ())
            // When: All type-compatible connections are made
            // Then: The workflow builds successfully

            let mut wf = Workflow::new("checkout-flow");

            // Node types:
            // validate: String -> bool
            // charge: bool -> String
            // finalize: String -> ()

            let validate = wf
                .pure("validate", |input: String| !input.is_empty())
                .expect("valid");

            let charge = wf
                .effect("charge", |valid: bool| {
                    if valid {
                        "receipt_456".to_string()
                    } else {
                        "failed".to_string()
                    }
                })
                .expect("valid");

            let finalize = wf
                .wait("finalize", |receipt: String| {
                    println!("Finalizing: {}", receipt);
                })
                .expect("valid");

            // When: Type-compatible chain is connected
            wf.connect(&validate, &charge).expect("validate -> charge");
            wf.connect(&charge, &finalize).expect("charge -> finalize");

            // Then: Workflow builds successfully
            let spec = wf.build().expect("workflow should build");
            assert_eq!(spec.nodes.len(), 3);
            assert_eq!(spec.edges.len(), 2);
        }

        #[test]
        fn scenario_linear_chain_with_different_types_builds() {
            // Given: A linear chain of nodes with different but compatible types
            // When: Each node's output matches the next node's input
            // Then: The workflow builds successfully

            let mut dag = Dag::new();

            // Node A: () -> String
            let node_a: NodeHandle<(), String> = dag
                .add_node_with_kind("start", NodeKind::Pure, |_: ()| "data".to_string())
                .expect("valid");

            // Node B: String -> i32
            let node_b: NodeHandle<String, i32> = dag
                .add_node_with_kind("process", NodeKind::Pure, |s: String| s.len() as i32)
                .expect("valid");

            // Node C: i32 -> bool
            let node_c: NodeHandle<i32, bool> = dag
                .add_node_with_kind("check", NodeKind::Pure, |n: i32| n > 0)
                .expect("valid");

            // When: Type-compatible connections are made
            dag.connect(&node_a, &node_b).expect("start -> process");
            dag.connect(&node_b, &node_c).expect("process -> check");

            // Then: DAG builds
            let spec = dag.build("typed-chain").expect("should build");
            assert_eq!(spec.nodes.len(), 3);
            assert_eq!(spec.edges.len(), 2);
        }

        #[test]
        fn scenario_diamond_dag_with_type_compatible_branches_builds() {
            // Given: A diamond DAG where branches have compatible types
            // When: Type-compatible connections are made
            // Then: The DAG builds successfully

            let mut dag = Dag::new();

            //        start: () -> i32
            //        /                    \
            // left: i32 -> String    right: i32 -> String
            //        \                    /
            //                  end: String -> ()

            let start: NodeHandle<(), i32> = dag
                .add_node_with_kind("start", NodeKind::Pure, |_: ()| 42)
                .expect("valid");

            let left: NodeHandle<i32, String> = dag
                .add_node_with_kind("left", NodeKind::Pure, |i: i32| format!("left:{}", i))
                .expect("valid");

            let right: NodeHandle<i32, String> = dag
                .add_node_with_kind("right", NodeKind::Pure, |i: i32| format!("right:{}", i))
                .expect("valid");

            let end: NodeHandle<String, ()> = dag
                .add_node_with_kind("end", NodeKind::Pure, |s: String| {
                    println!("Got: {}", s);
                })
                .expect("valid");

            // When: Type-compatible connections
            dag.connect(&start, &left).expect("start -> left");
            dag.connect(&start, &right).expect("start -> right");
            dag.connect(&left, &end).expect("left -> end");
            dag.connect(&right, &end).expect("right -> end");

            // Then: Diamond DAG builds
            let spec = dag.build("diamond").expect("should build");
            assert_eq!(spec.nodes.len(), 4);
            assert_eq!(spec.edges.len(), 4);
        }

        #[test]
        fn scenario_multiple_outputs_fan_out_type_safe() {
            // Given: A node with multiple type-compatible outgoing connections
            // When: All connections are type-compatible
            // Then: The DAG builds successfully

            let mut dag = Dag::new();

            // splitter: () -> i32
            // one: i32 -> String
            // two: i32 -> bool

            let splitter: NodeHandle<(), i32> = dag
                .add_node_with_kind("splitter", NodeKind::Pure, |_: ()| 100)
                .expect("valid");

            let one: NodeHandle<i32, String> = dag
                .add_node_with_kind("one", NodeKind::Pure, |i: i32| format!("val:{}", i))
                .expect("valid");

            let two: NodeHandle<i32, bool> = dag
                .add_node_with_kind("two", NodeKind::Pure, |i: i32| i > 50)
                .expect("valid");

            // When: Fan-out connections
            dag.connect(&splitter, &one).expect("splitter -> one");
            dag.connect(&splitter, &two).expect("splitter -> two");

            // Then: DAG builds with multiple edges from one node
            let spec = dag.build("fan-out").expect("should build");
            assert_eq!(spec.nodes.len(), 3);
            assert_eq!(spec.edges.len(), 2);
        }

        #[test]
        fn scenario_complex_workflow_with_all_node_types_builds() {
            // Given: A workflow using all node kinds with type-safe connections
            // When: All connections respect type compatibility
            // Then: The workflow builds successfully

            let mut wf = Workflow::new("all-kinds");

            // Pure: () -> String
            let source = wf
                .pure("source", |_: ()| "hello".to_string())
                .expect("valid");

            // Effect: String -> bool
            let processor = wf
                .effect("processor", |s: String| s.len() > 0)
                .expect("valid");

            // Wait: bool -> i32
            let waiter = wf
                .wait("waiter", |b: bool| if b { 1 } else { 0 })
                .expect("valid");

            // Signal: i32 -> String
            let signaler = wf
                .signal("signaler", |n: i32| format!("signal-{}", n))
                .expect("valid");

            // Unsafe: String -> ()
            let ender = wf
                .unsafe_node("ender", |s: String| println!("Ended: {}", s))
                .expect("valid");

            // When: Type-compatible chain
            wf.connect(&source, &processor).expect("source -> processor");
            wf.connect(&processor, &waiter).expect("processor -> waiter");
            wf.connect(&waiter, &signaler).expect("waiter -> signaler");
            wf.connect(&signaler, &ender).expect("signaler -> ender");

            // Then: Workflow builds
            let spec = wf.build().expect("should build");
            assert_eq!(spec.nodes.len(), 5);
            assert_eq!(spec.edges.len(), 4);
        }
    }

    mod compile_time_type_safety_enforcement {
        use super::*;

        #[test]
        fn type_safety_enforced_via_generic_signature() {
            // This test documents that the connect method's generic signature
            // enforces type safety at compile time.
            //
            // The signature:
            //   connect<T>(&mut self, from: &NodeHandle<impl Any, T>, to: &NodeHandle<T, impl Any>)
            //
            // If you try to call connect with incompatible types, you get a
            // compile error, NOT a runtime error.

            let mut dag = Dag::new();

            // Node outputs String
            let str_node: NodeHandle<(), String> = dag
                .add_node_with_kind("str", NodeKind::Pure, |_: ()| "test".to_string())
                .expect("valid");

            // NOTE: The following code would NOT compile:
            //
            // let int_node: NodeHandle<i32, ()> = dag
            //     .add_node_with_kind("int", NodeKind::Pure, |_: i32| ())
            //     .expect("valid");
            // dag.connect(&str_node, &int_node);
            // Error: expected `i32`, found `String`
            //
            // This is the compile-time type safety that ADR-010 guarantees!

            // What we CAN do is connect compatible types
            let another_str_node: NodeHandle<(), String> = dag
                .add_node_with_kind("str2", NodeKind::Pure, |_: ()| "test2".to_string())
                .expect("valid");

            // This compiles because both nodes have the same connection type
            let str_to_str: NodeHandle<String, String> = dag
                .add_node_with_kind("str_to_str", NodeKind::Pure, |s: String| s)
                .expect("valid");

            dag.connect(&str_node, &str_to_str).expect("compatible connection");
            dag.connect(&another_str_node, &str_to_str).expect("another compatible");

            // 3 nodes: str_node, another_str_node, str_to_str
            // int_node type incompatibility is caught at COMPILE TIME, not runtime
            let spec = dag.build("type-safe").expect("should build");
            assert_eq!(spec.nodes.len(), 3);
        }

        #[test]
        fn phantom_type_erasure_does_not_compromise_safety() {
            // ADR-010 guarantees that phantom types (I, O) are used only for
            // compile-time type checking and do not affect runtime behavior.
            //
            // NodeHandle<String, i32> and NodeHandle<bool, ()> with the same
            // NodeName are considered equal for serialization and hashing.

            let nm = vo_types::NodeName::parse("test-node").expect("valid");

            let h1: NodeHandle<String, i32> = NodeHandle::new(nm.clone());
            let h2: NodeHandle<bool, ()> = NodeHandle::new(nm.clone());

            // Same name
            assert_eq!(h1.name(), h2.name());

            // Same hash (phantom types don't affect hash)
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let (mut s1, mut s2) = (DefaultHasher::new(), DefaultHasher::new());
            h1.hash(&mut s1);
            h2.hash(&mut s2);
            assert_eq!(s1.finish(), s2.finish());
        }
    }

    mod error_detection {
        use super::*;

        #[test]
        fn runtime_errors_still_detected_correctly() {
            // Self-loops are detected at connect time, not build time.
            // This is different from cycles which are detected at build time.

            let mut dag = Dag::new();

            let a: NodeHandle<(), ()> = dag
                .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
                .expect("valid");

            let b: NodeHandle<(), ()> = dag
                .add_node_with_kind("b", NodeKind::Pure, |_: ()| ())
                .expect("valid");

            // Self-loop is NOT allowed at connect time (returns Err)
            let result = dag.connect(&a, &a);
            assert!(
                result.is_err(),
                "Self-loop should be detected at connect time"
            );
            assert!(matches!(result.unwrap_err(), DagError::SelfLoop { .. }));

            // Regular connections work
            dag.connect(&a, &b).expect("a -> b should succeed");

            // But cycles are detected at build time
            dag.connect(&b, &a).expect("b -> a creates cycle");
            let build_result = dag.build("cycle-test");
            assert!(
                build_result.is_err(),
                "Cycle should be detected at build time"
            );
        }

        #[test]
        fn orphan_detection_still_works() {
            // Orphan nodes (nodes with no connections) should be detected at build time.

            let mut dag = Dag::new();

            let _orphan: NodeHandle<(), ()> = dag
                .add_node_with_kind("orphan", NodeKind::Pure, |_: ()| ())
                .expect("valid");

            // Orphan is detected at build time
            let result = dag.build("orphan-test");
            assert!(
                result.is_err(),
                "Orphan node should be detected at build time"
            );
        }

        #[test]
        fn cycle_detection_still_works() {
            // Cycles are still detected at build time, not compile time

            let mut dag = Dag::new();

            let a: NodeHandle<(), ()> = dag
                .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
                .expect("valid");

            let b: NodeHandle<(), ()> = dag
                .add_node_with_kind("b", NodeKind::Pure, |_: ()| ())
                .expect("valid");

            dag.connect(&a, &b).expect("a -> b");
            dag.connect(&b, &a).expect("b -> a creates cycle");

            let result = dag.build("cycle-test");
            assert!(
                result.is_err(),
                "Cycle should be detected at build time"
            );
        }
    }
}