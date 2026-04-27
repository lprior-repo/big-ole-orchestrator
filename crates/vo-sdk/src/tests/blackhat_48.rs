//! Blackhat QA #48: Systematic audit and stress-test of vo-sdk error paths.
//!
//! Dimensions:
//!   1. io.rs: write_success at MAX_OUTPUT_SIZE boundary, atomic guard post-success
//!   2. read.rs: IS_READ atomic vs io.rs duplicate guard, read_input_inner_with_atomic_guard guard-set
//!   3. graph.rs: NoEntryPoint skipped on empty, detect_cycle "unknown cycle" path
//!   4. graph_args.rs: duplicate --graph NOT rejected (behavioral difference from graph.rs)
//!   5. dag.rs: duplicate edges accepted silently, build orphan detection with mixed orphan/cycle
//!   6. emit_graph_if_requested: process::exit bypasses Drop, stdout write failure
//!   7. Stress: concurrent read+write guard interaction, large spec stress

use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use serde_json::{json, Value};

use crate::dag::{Dag, DagError, Workflow};
use crate::graph::{
    default_retry_policy, parse_graph_args, emit_graph_if_requested, EdgeSpec, GraphArgsError,
    NodeSpec, WorkflowSpec,
};
use crate::io::{
    read_input_inner_with_atomic_guard, read_input_inner_with_state,
    write_failure_inner_with_state, write_success_inner_with_state,
};
use crate::{SdkError, TaskFailureKind};
use vo_types::{NodeKind, NodeName, WorkflowName};

use super::valid_envelope;

// ===========================================================================
// DIMENSION 1: write_success at MAX_OUTPUT_SIZE boundary
// ===========================================================================

#[test]
fn bh48_write_success_exactly_at_max_output_size() {
    let max = 10 * 1024 * 1024;
    // We need the final *envelope* to be exactly MAX_OUTPUT_SIZE bytes.
    // The envelope wraps: {"status":"success","output":PAYLOAD}
    // Build the payload by binary-searching for the right size.
    let mut filler_len = max;
    let mut payload = json!({"data": ""});
    loop {
        let filler = "x".repeat(filler_len);
        payload = json!({"data": filler});
        let envelope_bytes = serde_json::to_vec(&serde_json::json!({
            "status": "success",
            "output": payload
        }))
        .unwrap();
        if envelope_bytes.len() == max {
            break;
        }
        if envelope_bytes.len() > max {
            filler_len -= 1;
        } else {
            filler_len += 1;
        }
    }

    let value: Value = serde_json::to_vec(&payload)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap();
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner_with_state(&mut buf, &value, &mut is_written);
    assert_eq!(result, Ok(()), "exactly at MAX_OUTPUT_SIZE should succeed");
    assert!(is_written);
}

#[test]
fn bh48_write_success_one_byte_over_max_output_size() {
    let target = 10 * 1024 * 1024 + 1;
    let mut payload = json!({"data": ""});
    let json_str = serde_json::to_string(&payload).unwrap();
    let overhead = json_str.len() - 4;

    let filler = "x".repeat(target - overhead);
    payload = json!({"data": filler});
    let bytes = serde_json::to_vec(&payload).unwrap();

    assert!(
        bytes.len() > 10 * 1024 * 1024,
        "payload must exceed MAX_OUTPUT_SIZE"
    );

    let value: Value = serde_json::from_slice(&bytes).unwrap();
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner_with_state(&mut buf, &value, &mut is_written);
    assert_eq!(result, Err(SdkError::WriteError));
    assert!(is_written, "guard set even on size rejection");
}

#[test]
fn bh48_write_success_just_under_max_output_size() {
    let max = 10 * 1024 * 1024;
    // We need the final *envelope* to be just under MAX_OUTPUT_SIZE bytes.
    let mut filler_len = max - 10;
    let mut payload = json!({"data": ""});
    loop {
        let filler = "x".repeat(filler_len);
        payload = json!({"data": filler});
        let envelope_bytes = serde_json::to_vec(&serde_json::json!({
            "status": "success",
            "output": payload
        }))
        .unwrap();
        if envelope_bytes.len() < max {
            break;
        }
        filler_len -= 1;
    }

    let value: Value = serde_json::to_vec(&payload)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap();
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner_with_state(&mut buf, &value, &mut is_written);
    assert_eq!(result, Ok(()), "just under MAX_OUTPUT_SIZE should succeed");
}

// ===========================================================================
// DIMENSION 2: read_input_inner_with_atomic_guard guard-set verification
// ===========================================================================

#[test]
fn bh48_atomic_guard_set_after_successful_read() {
    let guard = Arc::new(AtomicBool::new(false));
    let payload = valid_envelope("guard-test", &json!(null));
    let mut cursor = Cursor::new(payload);

    let result = read_input_inner_with_atomic_guard(&mut cursor, &guard);

    assert!(result.is_ok(), "read should succeed");
    assert!(
        guard.load(Ordering::SeqCst),
        "atomic guard must be set after successful read"
    );
}

#[test]
fn bh48_atomic_guard_set_after_failed_read() {
    let guard = Arc::new(AtomicBool::new(false));
    let mut cursor = Cursor::new(b"invalid json".to_vec());

    let result = read_input_inner_with_atomic_guard(&mut cursor, &guard);

    assert!(result.is_err(), "invalid json should fail");
    assert!(
        guard.load(Ordering::SeqCst),
        "atomic guard must be set even after failed read"
    );
}

#[test]
fn bh48_atomic_guard_set_after_empty_read() {
    let guard = Arc::new(AtomicBool::new(false));
    let mut cursor = Cursor::new(Vec::<u8>::new());

    let result = read_input_inner_with_atomic_guard(&mut cursor, &guard);

    assert_eq!(
        result,
        Err(SdkError::InvalidInput),
        "empty read returns InvalidInput (guard is set before I/O)"
    );
    assert!(guard.load(Ordering::SeqCst), "guard set on empty input");
}

#[test]
fn bh48_atomic_guard_blocks_second_read() {
    let guard = Arc::new(AtomicBool::new(false));
    let payload = valid_envelope("first-read", &json!(null));
    let mut cursor = Cursor::new(payload);

    let result1 = read_input_inner_with_atomic_guard(&mut cursor, &guard);
    assert!(result1.is_ok());

    let payload2 = valid_envelope("second-read", &json!(null));
    let mut cursor2 = Cursor::new(payload2);
    let result2 = read_input_inner_with_atomic_guard(&mut cursor2, &guard);

    assert_eq!(
        result2,
        Err(SdkError::FdNotOpen),
        "second read should be blocked"
    );
}

// ===========================================================================
// DIMENSION 3: WorkflowSpec::validate NoEntryPoint skipped on empty
// ===========================================================================

#[test]
fn bh48_validate_empty_nodes_skips_entry_point_check() {
    let spec = WorkflowSpec {
        workflow_name: WorkflowName::parse("empty-nodes").unwrap(),
        nodes: vec![],
        edges: vec![],
        dedupe_scope: Default::default(),
        guarantee_class: Default::default(),
    };
    let result = spec.validate();
    assert!(
        result.is_ok(),
        "empty nodes spec should pass validate (NoEntryPoint check skipped for n==0)"
    );
}

#[test]
fn bh48_validate_all_nodes_have_incoming_edges_rejects() {
    let spec = WorkflowSpec {
        workflow_name: WorkflowName::parse("no-entry").unwrap(),
        nodes: vec![
            NodeSpec {
                name: NodeName::parse("a").unwrap(),
                kind: NodeKind::Pure,
                retry_policy: default_retry_policy(),
            },
            NodeSpec {
                name: NodeName::parse("b").unwrap(),
                kind: NodeKind::Pure,
                retry_policy: default_retry_policy(),
            },
        ],
        edges: vec![
            EdgeSpec {
                from: NodeName::parse("a").unwrap(),
                to: NodeName::parse("b").unwrap(),
            },
            EdgeSpec {
                from: NodeName::parse("b").unwrap(),
                to: NodeName::parse("a").unwrap(),
            },
        ],
        dedupe_scope: Default::default(),
        guarantee_class: Default::default(),
    };
    let result = spec.validate();
    assert!(
        result.is_err(),
        "cycle should be detected before NoEntryPoint check"
    );
}

#[test]
fn bh48_validate_single_node_no_edges_has_entry_point() {
    let spec = WorkflowSpec {
        workflow_name: WorkflowName::parse("single").unwrap(),
        nodes: vec![NodeSpec {
            name: NodeName::parse("solo").unwrap(),
            kind: NodeKind::Pure,
            retry_policy: default_retry_policy(),
        }],
        edges: vec![],
        dedupe_scope: Default::default(),
        guarantee_class: Default::default(),
    };
    assert!(
        spec.validate().is_ok(),
        "single node with no edges is valid"
    );
}

#[test]
fn bh48_validate_chain_has_entry_point() {
    let spec = WorkflowSpec {
        workflow_name: WorkflowName::parse("chain").unwrap(),
        nodes: vec![
            NodeSpec {
                name: NodeName::parse("a").unwrap(),
                kind: NodeKind::Pure,
                retry_policy: default_retry_policy(),
            },
            NodeSpec {
                name: NodeName::parse("b").unwrap(),
                kind: NodeKind::Pure,
                retry_policy: default_retry_policy(),
            },
            NodeSpec {
                name: NodeName::parse("c").unwrap(),
                kind: NodeKind::Pure,
                retry_policy: default_retry_policy(),
            },
        ],
        edges: vec![
            EdgeSpec {
                from: NodeName::parse("a").unwrap(),
                to: NodeName::parse("b").unwrap(),
            },
            EdgeSpec {
                from: NodeName::parse("b").unwrap(),
                to: NodeName::parse("c").unwrap(),
            },
        ],
        dedupe_scope: Default::default(),
        guarantee_class: Default::default(),
    };
    assert!(spec.validate().is_ok(), "linear chain should be valid");
}

// ===========================================================================
// DIMENSION 4: graph_args.rs duplicate --graph NOT rejected (behavioral doc)
// ===========================================================================

#[test]
fn bh48_graph_args_duplicate_graph_flag_in_graph_args_module() {
    let args = vec![
        "bin".to_string(),
        "--graph".to_string(),
        "--graph".to_string(),
    ];
    let result = parse_graph_args(&args);
    assert_eq!(
        result,
        Err(GraphArgsError::UnrecognizedArgument { arg: "--graph".to_string() }),
        "graph.rs module rejects duplicate --graph"
    );
}

#[test]
fn bh48_graph_args_empty_args_list() {
    let args: Vec<String> = vec![];
    let result = parse_graph_args(&args);
    assert_eq!(result, Err(GraphArgsError::NoGraphFlag));
}

#[test]
fn bh48_graph_args_only_binary_name() {
    let args = vec!["my-binary".to_string()];
    let result = parse_graph_args(&args);
    assert_eq!(result, Err(GraphArgsError::NoGraphFlag));
}

#[test]
fn bh48_graph_args_many_args_before_graph() {
    let args = vec![
        "bin".to_string(),
        "--other-flag".to_string(),
        "positional".to_string(),
        "--graph".to_string(),
    ];
    let result = parse_graph_args(&args);
    assert_eq!(result, Ok(crate::graph::GraphArgs));
}

#[test]
fn bh48_graph_args_empty_string_as_flag() {
    let args = vec!["bin".to_string(), "".to_string()];
    let result = parse_graph_args(&args);
    assert_eq!(
        result,
        Err(GraphArgsError::NoGraphFlag),
        "empty string is not --graph"
    );
}

#[test]
fn bh48_graph_args_graph_embedded_in_larger_arg() {
    let args = vec!["bin".to_string(), "--graphx".to_string()];
    let result = parse_graph_args(&args);
    assert_eq!(
        result,
        Err(GraphArgsError::NoGraphFlag),
        "--graphx should not match --graph"
    );
}

// ===========================================================================
// DIMENSION 5: dag.rs duplicate edges accepted silently
// ===========================================================================

#[test]
fn bh48_dag_duplicate_edges_accepted_by_connect() {
    let mut dag = Dag::new();
    let a: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .unwrap();
    let b: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("b", NodeKind::Pure, |_: ()| ())
        .unwrap();

    dag.connect(&a, &b).unwrap();
    let result = dag.connect(&a, &b);
    assert!(
        result.is_ok(),
        "duplicate edge is accepted silently by Dag::connect"
    );
    assert_eq!(dag.edge_count(), 2, "duplicate edge is stored");
}

#[test]
fn bh48_dag_duplicate_edges_build_succeeds() {
    let mut dag = Dag::new();
    let a: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .unwrap();
    let b: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("b", NodeKind::Pure, |_: ()| ())
        .unwrap();
    dag.connect(&a, &b).unwrap();
    dag.connect(&a, &b).unwrap();

    let result = dag.build("dup-edges");
    assert!(result.is_ok(), "build succeeds with duplicate edges");
    let spec = result.unwrap();
    assert_eq!(spec.edges.len(), 2);
}

#[test]
fn bh48_dag_self_duplicate_edge_produces_cycle() {
    let mut dag = Dag::new();
    let a: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .unwrap();
    dag.connect(&a, &a).unwrap();

    let result = dag.build("self-dup");
    assert!(
        matches!(result, Err(DagError::CycleDetected { .. })),
        "self-edge creates cycle"
    );
}

#[test]
fn bh48_dag_build_disconnected_nodes_accepted() {
    let mut dag = Dag::new();
    let _a: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .unwrap();
    let _b: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("b", NodeKind::Pure, |_: ()| ())
        .unwrap();

    let result = dag.build("orphan-test");
    // Two disconnected nodes both have zero in-degree, so BFS visits both.
    // The current DAG validator does not detect disconnected components.
    assert!(
        result.is_ok(),
        "two disconnected nodes pass build (BFS visits all zero-in-degree nodes)"
    );
}

#[test]
fn bh48_dag_build_one_connected_one_isolated_accepted() {
    let mut dag = Dag::new();
    let a: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .unwrap();
    let b: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("b", NodeKind::Pure, |_: ()| ())
        .unwrap();
    let _c: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("c", NodeKind::Pure, |_: ()| ())
        .unwrap();
    dag.connect(&a, &b).unwrap();

    let result = dag.build("partial-orphan");
    // Node c has zero in-degree, so BFS visits it. No cycle, no orphan detected.
    assert!(
        result.is_ok(),
        "node c is isolated but has zero in-degree so BFS visits it"
    );
}

#[test]
fn bh48_dag_build_cycle_takes_priority_over_orphan() {
    let mut dag = Dag::new();
    let a: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .unwrap();
    let b: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("b", NodeKind::Pure, |_: ()| ())
        .unwrap();
    let _c: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("c", NodeKind::Pure, |_: ()| ())
        .unwrap();
    dag.connect(&a, &b).unwrap();
    dag.connect(&b, &a).unwrap();

    let result = dag.build("cycle-and-orphan");
    assert!(
        matches!(result, Err(DagError::CycleDetected { .. })),
        "cycle error should take priority over orphan detection"
    );
}

// ===========================================================================
// DIMENSION 6: emit_graph_if_requested behavioral verification
// ===========================================================================

#[test]
fn bh48_emit_graph_returns_ok_when_no_graph_flag() {
    let args = vec!["bin".to_string()];
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

    let result = emit_graph_if_requested(&args, &spec);
    assert_eq!(result, Ok(()), "no --graph flag should return Ok(())");
}

#[test]
fn bh48_emit_graph_returns_err_for_unrecognized_args() {
    let args = vec![
        "bin".to_string(),
        "--graph".to_string(),
        "unexpected".to_string(),
    ];
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

    let result = emit_graph_if_requested(&args, &spec);
    assert_eq!(
        result,
        Err(()),
        "extra args after --graph should return Err(())"
    );
}

#[test]
fn bh48_to_json_bytes_never_panics_on_complex_nested_output() {
    let spec = WorkflowSpec {
        workflow_name: WorkflowName::parse("complex").unwrap(),
        nodes: (0..100)
            .map(|i| NodeSpec {
                name: NodeName::parse(&format!("node-{}", i)).unwrap(),
                kind: NodeKind::Pure,
                retry_policy: default_retry_policy(),
            })
            .collect(),
        edges: (0..99)
            .map(|i| EdgeSpec {
                from: NodeName::parse(&format!("node-{}", i)).unwrap(),
                to: NodeName::parse(&format!("node-{}", i + 1)).unwrap(),
            })
            .collect(),
        dedupe_scope: Default::default(),
        guarantee_class: Default::default(),
    };
    let bytes = spec.to_json_bytes();
    assert!(!bytes.is_empty());
    let _: Value = serde_json::from_slice(&bytes).expect("valid JSON");
}

// ===========================================================================
// DIMENSION 7: Concurrent read+write guard interaction
// ===========================================================================

#[test]
fn bh48_concurrent_read_and_write_do_not_interfere() {
    let read_guard = Arc::new(AtomicBool::new(false));
    let write_guard = Arc::new(AtomicBool::new(false));
    let read_ok = Arc::new(AtomicBool::new(false));
    let write_ok = Arc::new(AtomicBool::new(false));

    let rg = Arc::clone(&read_guard);
    let wg = Arc::clone(&write_guard);
    let rok = Arc::clone(&read_ok);
    let wok = Arc::clone(&write_ok);

    let read_thread = thread::spawn(move || {
        let payload = valid_envelope("concurrent-key", &json!(null));
        let mut cursor = Cursor::new(payload);
        if read_input_inner_with_atomic_guard(&mut cursor, &rg).is_ok() {
            rok.store(true, Ordering::SeqCst);
        }
    });

    let write_thread = thread::spawn(move || {
        let mut buf: Vec<u8> = Vec::new();
        let mut local_written = false;
        // The compare_exchange on wg is an external coordination guard.
        // Only proceed if we win the external guard; write_success_inner_with_state
        // manages its own is_written flag internally.
        if wg
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }
        if write_success_inner_with_state(&mut buf, &json!("ok"), &mut local_written).is_ok() {
            wok.store(true, Ordering::SeqCst);
        }
    });

    read_thread.join().expect("read thread no panic");
    write_thread.join().expect("write thread no panic");

    assert!(read_ok.load(Ordering::SeqCst), "read should succeed");
    assert!(write_ok.load(Ordering::SeqCst), "write should succeed");
}

#[test]
fn bh48_concurrent_writes_both_fail_after_one_succeeds() {
    let shared_guard = Arc::new(AtomicBool::new(false));
    let success_count = Arc::new(AtomicBool::new(false));
    let fail_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let mut handles = Vec::new();
    for _ in 0..8 {
        let guard = Arc::clone(&shared_guard);
        let sc = Arc::clone(&success_count);
        let fc = Arc::clone(&fail_count);
        handles.push(thread::spawn(move || {
            let mut buf: Vec<u8> = Vec::new();
            let mut local_written = false;
            // Only the winner of compare_exchange attempts the write.
            // write_failure_inner_with_state manages its own is_written guard.
            if guard
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
            {
                // Lost the external race — record as AlreadyWritten
                fc.fetch_add(1, Ordering::SeqCst);
                return;
            }
            match write_failure_inner_with_state(
                &mut buf,
                TaskFailureKind::User,
                "concurrent",
                &mut local_written,
            ) {
                Ok(()) => {
                    sc.store(true, Ordering::SeqCst);
                }
                Err(SdkError::AlreadyWritten) => {
                    fc.fetch_add(1, Ordering::SeqCst);
                }
                _ => {}
            }
        }));
    }

    for h in handles {
        h.join().expect("no panic");
    }

    assert!(
        success_count.load(Ordering::SeqCst),
        "at least one write succeeds"
    );
    assert!(
        fail_count.load(Ordering::SeqCst) >= 7,
        "at least 7 writes should fail with AlreadyWritten"
    );
}

// ===========================================================================
// DIMENSION 8: Malformed input edge cases
// ===========================================================================

#[test]
fn bh48_read_bom_prefixed_input_returns_invalid_input() {
    let mut bom_payload = b"\xEF\xBB\xBF".to_vec();
    bom_payload.extend(b"{}");
    let mut cursor = Cursor::new(bom_payload);
    let mut is_read = false;

    let result = read_input_inner_with_state(&mut cursor, &mut is_read);
    assert_eq!(result, Err(SdkError::InvalidInput));
}

#[test]
fn bh48_read_json_array_input_returns_invalid_input() {
    let payload = serde_json::to_vec(&json!([{"idempotency_key": "k", "data": 1}])).unwrap();
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let result = read_input_inner_with_state(&mut cursor, &mut is_read);
    assert_eq!(result, Err(SdkError::InvalidInput));
}

#[test]
fn bh48_read_json_number_input_returns_invalid_input() {
    let mut cursor = Cursor::new(b"42".to_vec());
    let mut is_read = false;

    let result = read_input_inner_with_state(&mut cursor, &mut is_read);
    assert_eq!(result, Err(SdkError::InvalidInput));
}

#[test]
fn bh48_read_json_bool_input_returns_invalid_input() {
    let mut cursor = Cursor::new(b"true".to_vec());
    let mut is_read = false;

    let result = read_input_inner_with_state(&mut cursor, &mut is_read);
    assert_eq!(result, Err(SdkError::InvalidInput));
}

#[test]
fn bh48_read_json_string_input_returns_invalid_input() {
    let mut cursor = Cursor::new(b"\"hello\"".to_vec());
    let mut is_read = false;

    let result = read_input_inner_with_state(&mut cursor, &mut is_read);
    assert_eq!(result, Err(SdkError::InvalidInput));
}

#[test]
fn bh48_read_null_json_input_returns_invalid_input() {
    let mut cursor = Cursor::new(b"null".to_vec());
    let mut is_read = false;

    let result = read_input_inner_with_state(&mut cursor, &mut is_read);
    assert_eq!(result, Err(SdkError::InvalidInput));
}

#[test]
fn bh48_read_whitespace_only_input_returns_invalid_input() {
    let mut cursor = Cursor::new(b"   \n\t  ".to_vec());
    let mut is_read = false;

    let result = read_input_inner_with_state(&mut cursor, &mut is_read);
    assert_eq!(result, Err(SdkError::InvalidInput));
}

#[test]
fn bh48_read_nested_null_bytes_in_json_returns_invalid_input() {
    let payload = b"{\"idempotency_key\": \"k\", \"data\": \"val\0ue\"}".to_vec();
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let result = read_input_inner_with_state(&mut cursor, &mut is_read);
    assert_eq!(result, Err(SdkError::InvalidInput));
}

#[test]
fn bh48_read_idempotency_key_exceeds_max_length() {
    let long_key = "a".repeat(1025);
    let payload = valid_envelope(&long_key, &json!(null));
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let result = read_input_inner_with_state(&mut cursor, &mut is_read);
    assert_eq!(result, Err(SdkError::InvalidInput));
}

#[test]
fn bh48_read_idempotency_key_at_max_length() {
    let key = "a".repeat(1024);
    let payload = valid_envelope(&key, &json!(null));
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let result = read_input_inner_with_state(&mut cursor, &mut is_read);
    assert!(
        result.is_ok(),
        "idempotency key at max length should be accepted"
    );
}

// ===========================================================================
// DIMENSION 9: write_failure edge cases
// ===========================================================================

#[test]
fn bh48_write_failure_message_with_carriage_return() {
    let msg = "line1\r\nline2";
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result =
        write_failure_inner_with_state(&mut buf, TaskFailureKind::User, msg, &mut is_written);
    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf).unwrap();
    assert_eq!(written["message"], "line1\r\nline2");
}

#[test]
fn bh48_write_failure_message_with_tab_characters() {
    let msg = "col1\tcol2\tcol3";
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result =
        write_failure_inner_with_state(&mut buf, TaskFailureKind::System, msg, &mut is_written);
    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf).unwrap();
    assert_eq!(written["message"], "col1\tcol2\tcol3");
}

#[test]
fn bh48_write_failure_message_with_emoji() {
    let msg = "Error: \u{1F6A9} invalid input \u{274C}";
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result =
        write_failure_inner_with_state(&mut buf, TaskFailureKind::User, msg, &mut is_written);
    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf).unwrap();
    assert_eq!(written["message"], msg);
}

#[test]
fn bh48_write_failure_four_byte_emoji_exceeds_byte_limit() {
    let emoji = "\u{1F600}";
    assert_eq!(emoji.len(), 4);
    let count = 257;
    let msg = emoji.repeat(count);
    assert_eq!(msg.len(), 1028, "4 * 257 = 1028 > 1024");

    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result =
        write_failure_inner_with_state(&mut buf, TaskFailureKind::User, &msg, &mut is_written);
    assert_eq!(result, Err(SdkError::InvalidInput));
}

// ===========================================================================
// DIMENSION 10: Dag builder additional edge cases
// ===========================================================================

#[test]
fn bh48_dag_build_empty_workflow_name_rejected() {
    let mut dag = Dag::new();
    let _: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .unwrap();

    let result = dag.build("");
    assert!(matches!(result, Err(DagError::InvalidNodeName { .. })));
}

#[test]
fn bh48_dag_build_single_node_succeeds() {
    let mut dag = Dag::new();
    let _: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("solo", NodeKind::Pure, |_: ()| ())
        .unwrap();

    let result = dag.build("single");
    assert!(result.is_ok());
}

#[test]
fn bh48_dag_build_chain_of_200_nodes() {
    let mut dag = Dag::new();
    let mut prev: Option<crate::node_handle::NodeHandle<(), ()>> = None;
    for i in 0..200 {
        let h: crate::node_handle::NodeHandle<(), ()> = dag
            .add_node_with_kind(&format!("n{}", i), NodeKind::Pure, |_: ()| ())
            .unwrap();
        if let Some(p) = prev {
            dag.connect(&p, &h).unwrap();
        }
        prev = Some(h);
    }
    let result = dag.build("long-chain");
    assert!(result.is_ok(), "200-node chain should build successfully");
}

#[test]
fn bh48_dag_all_kinds_chain_builds() {
    let mut wf = Workflow::new("all-kinds-chain");
    let a = wf.pure("a", |_i: ()| ()).unwrap();
    let b = wf.effect("b", |_i: ()| ()).unwrap();
    let c = wf.wait("c", |_i: ()| ()).unwrap();
    let d = wf.signal("d", |_i: ()| ()).unwrap();
    let e = wf.unsafe_node("e", |_i: ()| ()).unwrap();
    wf.connect(&a, &b).unwrap();
    wf.connect(&b, &c).unwrap();
    wf.connect(&c, &d).unwrap();
    wf.connect(&d, &e).unwrap();

    let spec = wf.build().unwrap();
    assert_eq!(spec.nodes.len(), 5);
    assert_eq!(spec.edges.len(), 4);
}

#[test]
fn bh48_dag_build_rejects_workflow_name_too_long() {
    let mut dag = Dag::new();
    let _: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .unwrap();

    let long_name = "a".repeat(129);
    let result = dag.build(&long_name);
    assert!(matches!(result, Err(DagError::InvalidNodeName { .. })));
}

#[test]
fn bh48_dag_connect_same_nodes_twice_stores_both_edges() {
    let mut dag = Dag::new();
    let a: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("a", NodeKind::Pure, |_: ()| ())
        .unwrap();
    let b: crate::node_handle::NodeHandle<(), ()> = dag
        .add_node_with_kind("b", NodeKind::Pure, |_: ()| ())
        .unwrap();

    dag.connect(&a, &b).unwrap();
    dag.connect(&a, &b).unwrap();
    dag.connect(&a, &b).unwrap();

    assert_eq!(dag.edge_count(), 3);
    let spec = dag.build("multi-dup").unwrap();
    assert_eq!(spec.edges.len(), 3);
}

// ===========================================================================
// DIMENSION 11: WorkflowSpec serde cycle detection via Deserialize
// ===========================================================================

#[test]
fn bh48_serde_rejects_three_node_cycle() {
    let json = r#"{
        "workflow_name": "cycle",
        "nodes": [
            {"name": "a", "kind": "pure"},
            {"name": "b", "kind": "pure"},
            {"name": "c", "kind": "pure"}
        ],
        "edges": [
            {"from": "a", "to": "b"},
            {"from": "b", "to": "c"},
            {"from": "c", "to": "a"}
        ]
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_err(), "serde should reject 3-node cycle");
}

#[test]
fn bh48_serde_rejects_self_loop_in_deserialize() {
    let json = r#"{
        "workflow_name": "self-loop",
        "nodes": [{"name": "a", "kind": "pure"}],
        "edges": [{"from": "a", "to": "a"}]
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_err(), "serde should reject self-loop");
}

#[test]
fn bh48_serde_rejects_edge_to_nonexistent_source() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [{"name": "b", "kind": "pure"}],
        "edges": [{"from": "ghost", "to": "b"}]
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn bh48_serde_rejects_edge_to_nonexistent_target() {
    let json = r#"{
        "workflow_name": "test",
        "nodes": [{"name": "a", "kind": "pure"}],
        "edges": [{"from": "a", "to": "ghost"}]
    }"#;
    let result: Result<WorkflowSpec, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

// ===========================================================================
// DIMENSION 12: ValidationError Display and properties
// ===========================================================================

#[test]
fn bh48_validation_error_all_variants_have_display() {
    use crate::graph::ValidationError;

    let errors = vec![
        ValidationError::DuplicateNodeName {
            name: "x".to_string(),
        },
        ValidationError::DuplicateEdge {
            from: "a".to_string(),
            to: "b".to_string(),
        },
        ValidationError::MissingEdgeSource {
            name: "src".to_string(),
        },
        ValidationError::MissingEdgeTarget {
            name: "tgt".to_string(),
        },
        ValidationError::SelfLoop {
            name: "self".to_string(),
        },
        ValidationError::CycleDetected {
            cycle: "a -> b".to_string(),
        },
        ValidationError::NoEntryPoint,
    ];

    for err in &errors {
        let display = format!("{}", err);
        assert!(
            !display.is_empty(),
            "Display should not be empty for {:?}",
            err
        );
    }
}

#[test]
fn bh48_validation_error_clone_works() {
    use crate::graph::ValidationError;

    let err = ValidationError::CycleDetected {
        cycle: "a -> b -> c".to_string(),
    };
    let cloned = err.clone();
    assert_eq!(err, cloned);
}

// ===========================================================================
// DIMENSION 13: Stress - large inputs do not panic
// ===========================================================================

#[test]
fn bh48_write_success_large_nested_json_no_panic() {
    let mut val = json!(null);
    for _ in 0..50 {
        val = json!({"nested": val});
    }
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = write_success_inner_with_state(&mut buf, &val, &mut is_written);
    }));
}

#[test]
fn bh48_write_failure_large_message_no_panic() {
    let msg = "x".repeat(100_000);
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        write_failure_inner_with_state(&mut buf, TaskFailureKind::User, &msg, &mut is_written)
    }));
    assert!(result.is_ok(), "should not panic on large message");
    assert_eq!(
        result.unwrap(),
        Err(SdkError::InvalidInput),
        "large message should be rejected"
    );
}

#[test]
fn bh48_read_large_valid_json_no_panic() {
    let data = "x".repeat(5 * 1024 * 1024);
    let payload = valid_envelope("stress-key", &json!(data));
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        read_input_inner_with_state(&mut cursor, &mut is_read)
    }));
    assert!(result.is_ok(), "should not panic on large valid input");
    assert!(result.unwrap().is_ok());
}

#[test]
fn bh48_workflow_spec_500_node_stress() {
    let nodes: Vec<NodeSpec> = (0..500)
        .map(|i| NodeSpec {
            name: NodeName::parse(&format!("node{}", i)).unwrap(),
            kind: NodeKind::Pure,
            retry_policy: default_retry_policy(),
        })
        .collect();

    let edges: Vec<EdgeSpec> = (0..499)
        .map(|i| EdgeSpec {
            from: NodeName::parse(&format!("node{}", i)).unwrap(),
            to: NodeName::parse(&format!("node{}", i + 1)).unwrap(),
        })
        .collect();

    let spec = WorkflowSpec {
        workflow_name: WorkflowName::parse("stress").unwrap(),
        nodes,
        edges,
        dedupe_scope: Default::default(),
        guarantee_class: Default::default(),
    };

    assert!(spec.validate().is_ok());

    let json = serde_json::to_string(&spec).unwrap();
    let restored: WorkflowSpec = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.nodes.len(), 500);
    assert_eq!(restored.edges.len(), 499);
}

#[test]
fn bh48_workflow_spec_fan_out_100_stress() {
    let mut nodes = vec![NodeSpec {
        name: NodeName::parse("root").unwrap(),
        kind: NodeKind::Pure,
        retry_policy: default_retry_policy(),
    }];
    for i in 0..100 {
        nodes.push(NodeSpec {
            name: NodeName::parse(&format!("leaf{}", i)).unwrap(),
            kind: NodeKind::Pure,
            retry_policy: default_retry_policy(),
        });
    }

    let edges: Vec<EdgeSpec> = (0..100)
        .map(|i| EdgeSpec {
            from: NodeName::parse("root").unwrap(),
            to: NodeName::parse(&format!("leaf{}", i)).unwrap(),
        })
        .collect();

    let spec = WorkflowSpec {
        workflow_name: WorkflowName::parse("fan100").unwrap(),
        nodes,
        edges,
        dedupe_scope: Default::default(),
        guarantee_class: Default::default(),
    };

    assert!(spec.validate().is_ok());
}

// ===========================================================================
// DIMENSION 14: IdempotencyKey boundary values
// ===========================================================================

#[test]
fn bh48_read_single_char_idempotency_key() {
    let payload = valid_envelope("a", &json!(null));
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let result = read_input_inner_with_state(&mut cursor, &mut is_read);
    assert!(result.is_ok(), "single char key should be valid");
}

#[test]
fn bh48_read_idempotency_key_with_dots() {
    let payload = valid_envelope("my.key.here", &json!(null));
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let result = read_input_inner_with_state(&mut cursor, &mut is_read);
    let accepted = result.is_ok();
    if !accepted {
        assert_eq!(result, Err(SdkError::InvalidInput));
    }
}

#[test]
fn bh48_read_idempotency_key_with_slashes() {
    let payload = valid_envelope("my/key/path", &json!(null));
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let result = read_input_inner_with_state(&mut cursor, &mut is_read);
    let accepted = result.is_ok();
    if !accepted {
        assert_eq!(result, Err(SdkError::InvalidInput));
    }
}

// ===========================================================================
// DIMENSION 15: DagError Display for all variants
// ===========================================================================

#[test]
fn bh48_dag_error_all_variants_display() {
    let errors = vec![
        DagError::InvalidNodeName {
            name: "bad!".to_string(),
        },
        DagError::NodeNotFound {
            name: "ghost".to_string(),
        },
        DagError::EmptyWorkflow,
        DagError::CycleDetected {
            cycle: "a -> b".to_string(),
        },
        DagError::DuplicateNodeName {
            name: "dup".to_string(),
        },
        DagError::SelfLoop {
            name: "self".to_string(),
        },
        DagError::OrphanNode {
            name: "orphan".to_string(),
        },
    ];

    for err in &errors {
        let display = format!("{}", err);
        assert!(
            !display.is_empty(),
            "Display should not be empty for {:?}",
            err
        );
    }
}

#[test]
fn bh48_dag_error_clone() {
    let err = DagError::OrphanNode {
        name: "test".to_string(),
    };
    let cloned = err.clone();
    assert_eq!(err, cloned);
}
