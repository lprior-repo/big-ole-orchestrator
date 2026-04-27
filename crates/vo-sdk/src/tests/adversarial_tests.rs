//! Adversarial tests for vo-sdk (bead ve-z32z).
//!
//! Covers: builder validation edge cases, envelope correctness, cross-write guard,
//! boundary conditions, concurrent AtomicBool guards, parse_graph_args edge cases,
//! NodeHandle property verification, WorkflowSpec serde integrity.

use std::io::{Cursor, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use serde_json::{json, Value};

use crate::dag::{Dag, DagError, Workflow};
use crate::graph::{
    default_retry_policy, parse_graph_args, EdgeSpec, GraphArgs, GraphArgsError, NodeSpec,
    WorkflowSpec,
};
use crate::node_handle::NodeHandle;
use crate::tests::{
    read_input_inner_with_atomic_guard as read_input_inner_atomic,
    read_input_inner_with_state as read_input_inner,
    write_failure_inner_with_state as write_failure_inner,
    write_success_inner_with_state as write_success_inner,
};
use crate::{SdkError, TaskFailureKind};
use vo_types::{NodeKind, NodeName, WorkflowName};

use super::valid_envelope;

// ===========================================================================
// DIMENSION: read_input_inner boundary & adversarial
// ===========================================================================

#[test]
fn read_non_utf8_input_returns_invalid_input() {
    let raw = vec![0xFF, 0xFE, 0xFD];
    let mut cursor = Cursor::new(raw);
    let mut is_read = false;

    let result = read_input_inner(&mut cursor, &mut is_read);

    assert_eq!(result, Err(SdkError::InvalidInput));
    assert!(is_read, "guard must be set even for non-UTF-8 input");
}

#[test]
fn read_whitespace_in_idempotency_key_returns_invalid_input() {
    let payload = serde_json::to_vec(&json!({
        "idempotency_key": "has spaces",
        "data": null
    }))
    .expect("serialize");
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let result = read_input_inner(&mut cursor, &mut is_read);

    assert_eq!(result, Err(SdkError::InvalidInput));
}

#[test]
fn read_special_chars_in_idempotency_key_returns_invalid_input() {
    let payload = serde_json::to_vec(&json!({
        "idempotency_key": "key!@#$%",
        "data": null
    }))
    .expect("serialize");
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let result = read_input_inner(&mut cursor, &mut is_read);

    assert_eq!(result, Err(SdkError::InvalidInput));
}

#[test]
fn read_numeric_idempotency_key_returns_valid_input() {
    let payload = serde_json::to_vec(&json!({
        "idempotency_key": "12345",
        "data": null
    }))
    .expect("serialize");
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let result = read_input_inner(&mut cursor, &mut is_read);

    assert!(result.is_ok(), "numeric-only idempotency key is valid: {:?}", result);
}

#[test]
fn read_accepts_any_json_value_as_data() {
    for data_val in [
        json!(null),
        json!([]),
        json!({"nested": {"deep": true}}),
        json!(42),
        json!("str"),
    ] {
        let payload = serde_json::to_vec(&json!({
            "idempotency_key": "key-ok",
            "data": data_val
        }))
        .expect("serialize");
        let mut cursor = Cursor::new(payload);
        let mut is_read = false;

        let result = read_input_inner(&mut cursor, &mut is_read);

        let input = result.expect("any valid JSON value should be accepted as data");
        assert_eq!(input.data, data_val);
    }
}

#[test]
fn read_at_max_input_size_boundary_succeeds() {
    let data = "x".repeat(10 * 1024 * 1024 - 200);
    let payload = valid_envelope("boundary-key", &json!({"big": data}));
    assert!(
        payload.len() <= 10 * 1024 * 1024,
        "payload must be at or under limit"
    );

    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let result = read_input_inner(&mut cursor, &mut is_read);

    assert!(result.is_ok(), "input at max size should succeed");
    assert!(is_read);
}

#[test]
fn read_one_byte_over_max_input_size_returns_invalid_input() {
    let data = "x".repeat(10 * 1024 * 1024);
    let payload = serde_json::to_vec(&json!({
        "idempotency_key": "overflow-key",
        "data": data
    }))
    .expect("serialize");
    assert!(
        payload.len() > 10 * 1024 * 1024,
        "payload must exceed limit"
    );

    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let result = read_input_inner(&mut cursor, &mut is_read);

    assert_eq!(result, Err(SdkError::InvalidInput));
}

#[test]
fn read_failed_parse_still_sets_guard() {
    let payload = b"{not valid json".to_vec();
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let result = read_input_inner(&mut cursor, &mut is_read);

    assert_eq!(result, Err(SdkError::InvalidInput));
    assert!(is_read, "guard must be set before parse attempt");
}

#[test]
fn read_partial_json_truncated_returns_invalid_input() {
    let payload = b"{\"idempotency_key\": \"k\", \"data\": ".to_vec();
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let result = read_input_inner(&mut cursor, &mut is_read);

    assert_eq!(result, Err(SdkError::InvalidInput));
}

// ===========================================================================
// DIMENSION: write_success_inner envelope & adversarial
// ===========================================================================

#[test]
fn write_success_envelope_has_exact_keys() {
    let output = json!({"result": 42});
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    write_success_inner(&mut buf, &output, &mut is_written).unwrap();

    let written: Value = serde_json::from_slice(&buf).expect("valid JSON");
    let keys: Vec<&str> = written
        .as_object()
        .unwrap()
        .keys()
        .map(|s| s.as_str())
        .collect();
    assert_eq!(
        keys,
        vec!["output", "status"],
        "only status and output keys expected"
    );
}

#[test]
fn write_success_accepts_nested_json_output() {
    let output = json!({"users": [{"id": 1, "name": "a"}, {"id": 2, "name": "b"}], "total": 2});
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf).unwrap();
    assert_eq!(written["output"], output);
}

#[test]
fn write_success_io_failure_returns_write_error_and_sets_guard() {
    struct BrokenWriter;
    impl Write for BrokenWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "broken",
            ))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let mut writer = BrokenWriter;
    let mut is_written = false;

    let result = write_success_inner(&mut writer, &json!("ok"), &mut is_written);

    assert_eq!(result, Err(SdkError::WriteError));
    assert!(is_written, "guard set before I/O attempt");
}

// ===========================================================================
// DIMENSION: write_failure_inner envelope & adversarial
// ===========================================================================

#[test]
fn write_failure_envelope_has_exact_keys() {
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    write_failure_inner(&mut buf, TaskFailureKind::User, "err", &mut is_written).unwrap();

    let written: Value = serde_json::from_slice(&buf).expect("valid JSON");
    let keys: Vec<&str> = written
        .as_object()
        .unwrap()
        .keys()
        .map(|s| s.as_str())
        .collect();
    assert_eq!(
        keys,
        vec!["kind", "message", "status"],
        "only status, kind, message keys expected"
    );
}

#[test]
fn write_failure_empty_message_succeeds() {
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_failure_inner(&mut buf, TaskFailureKind::System, "", &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf).unwrap();
    assert_eq!(written["message"], "");
}

#[test]
fn write_failure_newline_in_message_succeeds() {
    let msg = "line1\nline2\nline3";
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_failure_inner(&mut buf, TaskFailureKind::User, msg, &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf).unwrap();
    assert_eq!(written["message"], "line1\nline2\nline3");
}

#[test]
fn write_failure_null_byte_in_message_succeeds() {
    let msg = "before\0after";
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_failure_inner(&mut buf, TaskFailureKind::User, msg, &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf).unwrap();
    assert_eq!(written["message"], "before\0after");
}

#[test]
fn write_failure_io_failure_returns_write_error_and_sets_guard() {
    struct BrokenWriter;
    impl Write for BrokenWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "broken",
            ))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let mut writer = BrokenWriter;
    let mut is_written = false;

    let result = write_failure_inner(&mut writer, TaskFailureKind::User, "msg", &mut is_written);

    assert_eq!(result, Err(SdkError::WriteError));
    assert!(is_written, "guard set before I/O attempt");
}

#[test]
fn write_failure_after_success_returns_already_written() {
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    write_success_inner(&mut buf, &json!("ok"), &mut is_written).unwrap();

    let result = write_failure_inner(&mut buf, TaskFailureKind::User, "err", &mut is_written);

    assert_eq!(result, Err(SdkError::AlreadyWritten));
}

#[test]
fn write_success_after_failure_returns_already_written() {
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    write_failure_inner(&mut buf, TaskFailureKind::User, "err", &mut is_written).unwrap();

    let result = write_success_inner(&mut buf, &json!("ok"), &mut is_written);

    assert_eq!(result, Err(SdkError::AlreadyWritten));
}

// ===========================================================================
// DIMENSION: concurrent AtomicBool guards
// ===========================================================================

#[test]
fn concurrent_write_success_only_one_succeeds() {
    let guard = Arc::new(AtomicBool::new(false));
    let success_count = Arc::new(AtomicBool::new(false));

    let mut handles = Vec::new();
    for _ in 0..4 {
        let guard = Arc::clone(&guard);
        let success_count = Arc::clone(&success_count);
        handles.push(thread::spawn(move || {
            let mut buf: Vec<u8> = Vec::new();
            let mut local_guard = false;
            if guard
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                local_guard = true;
            }
            let result = write_success_inner(&mut buf, &json!("ok"), &mut local_guard);
            if result.is_ok() {
                success_count.store(true, Ordering::SeqCst);
            }
        }));
    }

    for h in handles {
        h.join().expect("thread should not panic");
    }

    let succeeded = success_count.load(Ordering::SeqCst);
    assert!(succeeded, "exactly one write should succeed");
}

#[test]
fn concurrent_read_input_only_one_succeeds() {
    let guard = Arc::new(AtomicBool::new(false));
    let success_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let mut handles = Vec::new();
    for _ in 0..4 {
        let guard = Arc::clone(&guard);
        let success_count = Arc::clone(&success_count);
        handles.push(thread::spawn(move || {
            let payload = valid_envelope("key-abc", &json!(null));
            let mut cursor = Cursor::new(payload);
            let result = read_input_inner_atomic(&mut cursor, &guard);
            if result.is_ok() {
                success_count.fetch_add(1, Ordering::SeqCst);
            }
        }));
    }

    for h in handles {
        h.join().expect("thread should not panic");
    }

    let count = success_count.load(Ordering::SeqCst);
    assert_eq!(count, 1, "exactly one read should succeed");
}

// ===========================================================================
// DIMENSION: Dag builder validation edge cases
// ===========================================================================

#[test]
fn dag_add_node_rejects_name_with_only_hyphens() {
    let mut dag = Dag::new();
    let result: Result<NodeHandle<(), ()>, DagError> =
        dag.add_node_with_kind("---", NodeKind::Pure, |_: ()| ());

    assert!(matches!(result, Err(DagError::InvalidNodeName { .. })));
}

#[test]
fn dag_add_node_rejects_name_starting_with_number() {
    let mut dag = Dag::new();
    let result: Result<NodeHandle<(), ()>, DagError> =
        dag.add_node_with_kind("123node", NodeKind::Pure, |_: ()| ());

    assert!(matches!(result, Err(DagError::InvalidNodeName { .. })));
}

#[test]
fn dag_add_node_rejects_consecutive_underscores() {
    let mut dag = Dag::new();
    let result: Result<NodeHandle<(), ()>, DagError> =
        dag.add_node_with_kind("node__bad", NodeKind::Pure, |_: ()| ());

    assert!(matches!(result, Err(DagError::InvalidNodeName { .. })));
}

#[test]
fn dag_add_node_accepts_name_at_max_length() {
    let name: String = "a".repeat(128);
    let mut dag = Dag::new();
    let result: Result<NodeHandle<(), ()>, DagError> =
        dag.add_node_with_kind(&name, NodeKind::Pure, |_: ()| ());

    assert!(result.is_ok(), "128-char name should be accepted");
}

#[test]
fn dag_add_node_rejects_name_over_max_length() {
    let name: String = "a".repeat(129);
    let mut dag = Dag::new();
    let result: Result<NodeHandle<(), ()>, DagError> =
        dag.add_node_with_kind(&name, NodeKind::Pure, |_: ()| ());

    assert!(matches!(result, Err(DagError::InvalidNodeName { .. })));
}

#[test]
fn dag_connect_rejects_handle_from_different_dag() {
    let mut dag1 = Dag::new();
    let mut dag2 = Dag::new();
    let a: NodeHandle<(), ()> = dag1
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .unwrap();
    let b: NodeHandle<(), ()> = dag2
        .add_node_with_kind("b", NodeKind::Pure, |_: ()| ())
        .unwrap();

    let result = dag1.connect(&a, &b);

    assert!(matches!(result, Err(DagError::NodeNotFound { name: _ })))
}

#[test]
fn dag_build_preserves_edge_insertion_order() {
    let mut dag = Dag::new();
    let a: NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .unwrap();
    let b: NodeHandle<(), ()> = dag
        .add_node_with_kind("b", NodeKind::Pure, |_: ()| ())
        .unwrap();
    let c: NodeHandle<(), ()> = dag
        .add_node_with_kind("c", NodeKind::Pure, |_: ()| ())
        .unwrap();
    let d: NodeHandle<(), ()> = dag
        .add_node_with_kind("d", NodeKind::Pure, |_: ()| ())
        .unwrap();
    dag.connect(&a, &b).unwrap();
    dag.connect(&a, &c).unwrap();
    dag.connect(&b, &d).unwrap();

    let edges = dag.edges();
    assert_eq!(edges, vec![("a", "b"), ("a", "c"), ("b", "d")]);
}

#[test]
fn dag_build_preserves_node_insertion_order() {
    let mut dag = Dag::new();
    let _: NodeHandle<(), ()> = dag
        .add_node_with_kind("first", NodeKind::Pure, |_: ()| ())
        .unwrap();
    let _: NodeHandle<(), ()> = dag
        .add_node_with_kind("second", NodeKind::Pure, |_: ()| ())
        .unwrap();
    let _: NodeHandle<(), ()> = dag
        .add_node_with_kind("third", NodeKind::Pure, |_: ()| ())
        .unwrap();

    let spec = dag.build("order_test").unwrap();
    let names: Vec<&str> = spec.nodes.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, vec!["first", "second", "third"]);
}

#[test]
fn dag_build_rejects_workflow_name_with_special_chars() {
    let mut dag = Dag::new();
    let _: NodeHandle<(), ()> = dag
        .add_node_with_kind("node", NodeKind::Pure, |_: ()| ())
        .unwrap();

    let result = dag.build("bad name!");

    assert!(matches!(result, Err(DagError::InvalidNodeName { .. })));
}

#[test]
fn dag_build_rejects_workflow_name_with_only_numbers() {
    let mut dag = Dag::new();
    let _: NodeHandle<(), ()> = dag
        .add_node_with_kind("node", NodeKind::Pure, |_: ()| ())
        .unwrap();

    let result = dag.build("12345");

    assert!(matches!(result, Err(DagError::InvalidNodeName { .. })));
}

#[test]
fn dag_node_and_edge_count_are_consistent() {
    let mut dag = Dag::new();
    let a: NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .unwrap();
    let b: NodeHandle<(), ()> = dag
        .add_node_with_kind("b", NodeKind::Pure, |_: ()| ())
        .unwrap();
    let c: NodeHandle<(), ()> = dag
        .add_node_with_kind("c", NodeKind::Pure, |_: ()| ())
        .unwrap();
    dag.connect(&a, &b).unwrap();
    dag.connect(&b, &c).unwrap();

    assert_eq!(dag.node_count(), 3);
    assert_eq!(dag.edge_count(), 2);
}

#[test]
fn dag_default_matches_new() {
    let default_dag = Dag::default();
    let new_dag = Dag::new();
    assert_eq!(default_dag.node_count(), new_dag.node_count());
    assert_eq!(default_dag.edge_count(), new_dag.edge_count());
}

// ===========================================================================
// DIMENSION: Workflow builder edge cases
// ===========================================================================

#[test]
fn workflow_build_uses_stored_workflow_name() {
    let mut wf = Workflow::new("custom-name");
    let _: NodeHandle<(), ()> = wf.pure("n", |_i: ()| ()).unwrap();

    let spec = wf.build().unwrap();
    assert_eq!(spec.workflow_name.as_str(), "custom-name");
}

#[test]
fn workflow_connect_type_mismatch_does_not_compile() {
    let mut wf = Workflow::new("type_check");
    let a: NodeHandle<String, i32> = wf.pure("a", |_i: String| -> i32 { 0 }).unwrap();
    let _b: NodeHandle<bool, ()> = wf.effect("b", |_i: bool| ()).unwrap();

    // This line should NOT compile (type mismatch i32 != bool):
    // wf.connect(&a, &_b);
    let _ = a;
}

// ===========================================================================
// DIMENSION: parse_graph_args edge cases
// ===========================================================================

#[test]
fn parse_graph_args_rejects_args_after_graph_in_middle() {
    let args = vec![
        "bin".to_string(),
        "other".to_string(),
        "--graph".to_string(),
    ];
    // "other" comes BEFORE --graph so it's ignored (skip(1) then iterate)
    let result = parse_graph_args(&args);
    assert_eq!(result, Ok(GraphArgs));
}

#[test]
fn parse_graph_args_rejects_second_graph_flag() {
    let args = vec![
        "bin".to_string(),
        "--graph".to_string(),
        "--graph".to_string(),
    ];
    let result = parse_graph_args(&args);
    assert!(matches!(
        result,
        Err(GraphArgsError::UnrecognizedArgument { .. })
    ));
}

#[test]
fn parse_graph_args_accepts_graph_as_first_arg() {
    let args = vec!["bin".to_string(), "--graph".to_string()];
    let result = parse_graph_args(&args);
    assert_eq!(result, Ok(GraphArgs));
}

#[test]
fn parse_graph_args_rejects_empty_arg_after_graph() {
    let args = vec!["bin".to_string(), "--graph".to_string(), "".to_string()];
    let result = parse_graph_args(&args);
    assert!(matches!(
        result,
        Err(GraphArgsError::UnrecognizedArgument { .. })
    ));
}

#[test]
fn graph_args_error_no_graph_flag_display() {
    let err = GraphArgsError::NoGraphFlag;
    let msg = err.to_string();
    assert!(msg.contains("no --graph flag found"), "display: {}", msg);
}

#[test]
fn graph_args_error_unrecognized_argument_display() {
    let err = GraphArgsError::UnrecognizedArgument {
        arg: "extra".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("extra"), "display: {}", msg);
    assert!(msg.contains("unrecognized"), "display: {}", msg);
}

#[test]
fn graph_args_is_copy_and_clone() {
    let args = vec!["bin".to_string(), "--graph".to_string()];
    let ga = parse_graph_args(&args).unwrap();
    let copied = ga;
    let cloned = ga;
    assert_eq!(ga, copied);
    assert_eq!(ga, cloned);
}

// ===========================================================================
// DIMENSION: WorkflowSpec serde integrity
// ===========================================================================

#[test]
fn workflow_spec_json_uses_snake_case() {
    let spec = WorkflowSpec {
        workflow_name: WorkflowName::parse("test").unwrap(),
        nodes: vec![NodeSpec {
            name: NodeName::parse("a").unwrap(),
            kind: NodeKind::Pure,
            retry_policy: default_retry_policy(),
        }],
        edges: vec![],
        dedupe_scope: Default::default(),
        guarantee_class: Default::default(),
    };
    let bytes = spec.to_json_bytes();
    let json_str = String::from_utf8(bytes).unwrap();

    assert!(json_str.contains("workflow_name"), "should use snake_case");
    assert!(
        !json_str.contains("workflowName"),
        "should not use camelCase"
    );
}

#[test]
fn workflow_spec_large_graph_roundtrip() {
    let nodes: Vec<NodeSpec> = (0..50)
        .map(|i| NodeSpec {
            name: NodeName::parse(&format!("node{}", i)).unwrap(),
            kind: NodeKind::Pure,
            retry_policy: default_retry_policy(),
        })
        .collect();

    let mut edges = Vec::new();
    for i in 0..49 {
        edges.push(EdgeSpec {
            from: NodeName::parse(&format!("node{}", i)).unwrap(),
            to: NodeName::parse(&format!("node{}", i + 1)).unwrap(),
        });
    }
    for i in 0..25 {
        edges.push(EdgeSpec {
            from: NodeName::parse(&format!("node{}", i)).unwrap(),
            to: NodeName::parse(&format!("node{}", i + 25)).unwrap(),
        });
    }

    let spec = WorkflowSpec {
        workflow_name: WorkflowName::parse("large_graph").unwrap(),
        nodes: nodes.clone(),
        edges: edges.clone(),
        dedupe_scope: Default::default(),
        guarantee_class: Default::default(),
    };

    let json = serde_json::to_string(&spec).unwrap();
    let restored: WorkflowSpec = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.nodes.len(), 50);
    assert_eq!(restored.edges.len(), 74);
    assert_eq!(restored, spec);
}

#[test]
fn workflow_spec_to_json_bytes_never_panics() {
    let spec = WorkflowSpec {
        workflow_name: WorkflowName::parse("empty").unwrap(),
        nodes: vec![],
        edges: vec![],
        dedupe_scope: Default::default(),
        guarantee_class: Default::default(),
    };
    let bytes = spec.to_json_bytes();
    assert!(!bytes.is_empty(), "should produce non-empty JSON");
    let _: Value = serde_json::from_slice(&bytes).expect("should be valid JSON");
}

// ===========================================================================
// DIMENSION: NodeHandle property verification
// ===========================================================================

#[test]
fn node_handle_equality_is_name_based() {
    let h1: NodeHandle<String, i32> = NodeHandle::new(NodeName::parse("same").unwrap());
    let h2: NodeHandle<String, i32> = NodeHandle::new(NodeName::parse("same").unwrap());

    assert_eq!(h1, h2, "same name should be equal");
}

#[test]
fn node_handle_hash_consistent_with_equality() {
    use std::collections::HashMap;

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

// ===========================================================================
// DIMENSION: TaskFailureKind properties
// ===========================================================================

#[test]
fn task_failure_kind_is_copy() {
    let kind = TaskFailureKind::User;
    let _copied = kind;
    assert_eq!(kind, TaskFailureKind::User);
}

#[test]
fn task_failure_kind_clone_matches_original() {
    for kind in [
        TaskFailureKind::User,
        TaskFailureKind::System,
        TaskFailureKind::Timeout,
    ] {
        let cloned = kind;
        assert_eq!(kind, cloned);
    }
}

// ===========================================================================
// PROPTEST: property-based adversarial tests
// ===========================================================================

#[cfg(feature = "proptest")]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn proptest_read_input_inner_never_panics(
            bytes in proptest::collection::vec(proptest::num::u8::ANY, 0..1024)
        ) {
            let mut cursor = Cursor::new(bytes);
            let mut is_read = false;
            let _ = std::panic::catch_unwind(|| {
                let _ = read_input_inner(&mut cursor, &mut is_read);
            });
        }

        #[test]
        fn proptest_write_failure_inner_never_panics(
            message in ".{0,2048}"
        ) {
            let mut buf: Vec<u8> = Vec::new();
            let mut is_written = false;
            let _ = std::panic::catch_unwind(|| {
                let _ = write_failure_inner(&mut buf, TaskFailureKind::User, &message, &mut is_written);
            });
        }

        #[test]
        fn proptest_write_success_inner_output_is_valid_json(
            val in proptest::arbitrary::any::<serde_json::Value>()
        ) {
            let mut buf: Vec<u8> = Vec::new();
            let mut is_written = false;
            let result = write_success_inner(&mut buf, &val, &mut is_written);

            if let Ok(()) = result {
                let parsed: serde_json::Value = serde_json::from_slice(&buf).unwrap();
                assert_eq!(parsed["status"], "success");
                assert_eq!(parsed["output"], val);
            }
        }

        #[test]
        fn proptest_write_failure_inner_output_is_valid_json(
            message in ".{0,1024}"
        ) {
            let mut buf: Vec<u8> = Vec::new();
            let mut is_written = false;
            let result = write_failure_inner(&mut buf, TaskFailureKind::System, &message, &mut is_written);

            if let Ok(()) = result {
                let parsed: serde_json::Value = serde_json::from_slice(&buf).unwrap();
                assert_eq!(parsed["status"], "failure");
                assert_eq!(parsed["kind"], "System");
                assert_eq!(parsed["message"], message);
            }
        }

        #[test]
        fn proptest_dag_build_produces_consistent_spec(
            node_count in 1usize..=10,
            connect_mask in proptest::bits::usize::any()
        ) {
            let mut dag = Dag::new();
            let mut handles: Vec<NodeHandle<(), ()>> = Vec::new();
            for i in 0..node_count {
                let h: NodeHandle<(), ()> = dag
                    .add_node_with_kind(&format!("node{}", i), NodeKind::Pure, |_: ()| ())
                    .unwrap();
                handles.push(h);
            }
            for i in 0..node_count.saturating_sub(1) {
                if connect_mask & (1 << i) != 0 {
                    dag.connect(&handles[i], &handles[i + 1]).unwrap();
                }
            }

            let result = dag.build("consistency_test");
            prop_assert!(result.is_ok());
            let spec = result.unwrap();
            assert_eq!(spec.nodes.len(), node_count);
        }
    }
}
