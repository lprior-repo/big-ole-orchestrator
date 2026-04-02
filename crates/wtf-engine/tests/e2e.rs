//! End-to-end tests for wtf-engine BinaryRegistry.
//!
//! Tests full lifecycle flows, concurrent access, and multi-operation scenarios
//! using real filesystem operations, real subprocesses, and real threading.

mod common;

use std::collections::HashSet;
use std::sync::{Arc, Barrier};

use common::*;
use wtf_engine::*;

// ===========================================================================
// Full lifecycle (B-REG-51)
// ===========================================================================

// B-REG-51
#[test]
fn full_lifecycle_register_resolve_deactivate_reap_transitions_correctly() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let source = make_test_binary(temp_dir.path(), &valid_three_node_graph());
    let source_path = bp(&source);
    let name = wn("lifecycle-wf");

    // Step 1: register
    let result = registry.register(&source_path, name.clone());
    assert_eq!(result, Ok(()));

    // Step 2: resolve — should return 3-node definition
    let (versioned_path, _binary_hash, definition) =
        registry.resolve(&name).expect("resolve after register");
    assert_eq!(definition.nodes.len(), 3);

    // Step 3: deactivate
    let result = registry.deactivate(&name);
    assert_eq!(result, Ok(()));

    // Step 4: resolve — should return WorkflowDeactivated
    assert!(matches!(
        registry.resolve(&name),
        Err(BinaryRegistryError::WorkflowDeactivated { .. })
    ));

    // Step 5: reap
    let report = registry.reap(|_| false);
    assert_eq!(report.reaped, vec![name.clone()]);
    assert!(report.skipped.is_empty());
    assert!(report.failures.is_empty());

    // Step 6: resolve — should return NotFound
    assert!(matches!(
        registry.resolve(&name),
        Err(BinaryRegistryError::NotFound { .. })
    ));

    // Step 7: versioned binary should no longer exist on disk
    assert!(
        matches!(std::fs::metadata(versioned_path.as_path()), Err(_)),
        "versioned binary should be deleted after reap"
    );
}

// ===========================================================================
// Concurrent access (B-REG-52)
// ===========================================================================

// B-REG-52
#[test]
fn registry_handles_concurrent_register_and_resolve_from_multiple_threads() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let registry = Arc::new(registry);

    // Pre-create 4 test binaries with unique names
    let mut sources = Vec::new();
    let graph0 = valid_graph_single_node("node-0");
    sources.push(make_test_binary(temp_dir.path(), &graph0));
    let graph1 = valid_graph_single_node("node-1");
    sources.push(make_test_binary(temp_dir.path(), &graph1));
    let graph2 = valid_graph_single_node("node-2");
    sources.push(make_test_binary(temp_dir.path(), &graph2));
    let graph3 = valid_graph_single_node("node-3");
    sources.push(make_test_binary(temp_dir.path(), &graph3));

    let barrier = Arc::new(Barrier::new(5)); // 4 workers + 1 coordinator

    // When: 4 threads concurrently register
    let mut handles = Vec::new();

    let registry0 = Arc::clone(&registry);
    let barrier0 = Arc::clone(&barrier);
    let source_path0 = bp(&sources[0]);
    let name0 = wn("wf-0");
    handles.push(std::thread::spawn(move || {
        barrier0.wait();
        registry0.register(&source_path0, name0)
    }));

    let registry1 = Arc::clone(&registry);
    let barrier1 = Arc::clone(&barrier);
    let source_path1 = bp(&sources[1]);
    let name1 = wn("wf-1");
    handles.push(std::thread::spawn(move || {
        barrier1.wait();
        registry1.register(&source_path1, name1)
    }));

    let registry2 = Arc::clone(&registry);
    let barrier2 = Arc::clone(&barrier);
    let source_path2 = bp(&sources[2]);
    let name2 = wn("wf-2");
    handles.push(std::thread::spawn(move || {
        barrier2.wait();
        registry2.register(&source_path2, name2)
    }));

    let registry3 = Arc::clone(&registry);
    let barrier3 = Arc::clone(&barrier);
    let source_path3 = bp(&sources[3]);
    let name3 = wn("wf-3");
    handles.push(std::thread::spawn(move || {
        barrier3.wait();
        registry3.register(&source_path3, name3)
    }));

    // Coordinator waits for all workers
    barrier.wait();

    // Wait for all register threads to complete
    let mut handles_iter = handles.into_iter();
    handles_iter
        .next()
        .unwrap()
        .join()
        .expect("thread should not panic");
    handles_iter
        .next()
        .unwrap()
        .join()
        .expect("thread should not panic");
    handles_iter
        .next()
        .unwrap()
        .join()
        .expect("thread should not panic");
    handles_iter
        .next()
        .unwrap()
        .join()
        .expect("thread should not panic");

    // Then: 4 threads concurrently resolve
    let barrier2 = Arc::new(Barrier::new(5));
    let mut resolve_handles = Vec::new();

    let registry_r0 = Arc::clone(&registry);
    let barrier_r0 = Arc::clone(&barrier2);
    let name_r0 = wn("wf-0");
    resolve_handles.push(std::thread::spawn(move || {
        barrier_r0.wait();
        if matches!(registry_r0.resolve(&name_r0), Err(_)) {
            panic!("resolve should succeed");
        }
    }));

    let registry_r1 = Arc::clone(&registry);
    let barrier_r1 = Arc::clone(&barrier2);
    let name_r1 = wn("wf-1");
    resolve_handles.push(std::thread::spawn(move || {
        barrier_r1.wait();
        if matches!(registry_r1.resolve(&name_r1), Err(_)) {
            panic!("resolve should succeed");
        }
    }));

    let registry_r2 = Arc::clone(&registry);
    let barrier_r2 = Arc::clone(&barrier2);
    let name_r2 = wn("wf-2");
    resolve_handles.push(std::thread::spawn(move || {
        barrier_r2.wait();
        if matches!(registry_r2.resolve(&name_r2), Err(_)) {
            panic!("resolve should succeed");
        }
    }));

    let registry_r3 = Arc::clone(&registry);
    let barrier_r3 = Arc::clone(&barrier2);
    let name_r3 = wn("wf-3");
    resolve_handles.push(std::thread::spawn(move || {
        barrier_r3.wait();
        if matches!(registry_r3.resolve(&name_r3), Err(_)) {
            panic!("resolve should succeed");
        }
    }));

    barrier2.wait();

    let mut r_handles_iter = resolve_handles.into_iter();
    r_handles_iter
        .next()
        .unwrap()
        .join()
        .expect("thread should not panic");
    r_handles_iter
        .next()
        .unwrap()
        .join()
        .expect("thread should not panic");
    r_handles_iter
        .next()
        .unwrap()
        .join()
        .expect("thread should not panic");
    r_handles_iter
        .next()
        .unwrap()
        .join()
        .expect("thread should not panic");

    // Then: len should be 4
    assert_eq!(registry.len(), 4);

    // And each resolve should succeed
    let _val0 = registry
        .resolve(&wn("wf-0"))
        .expect("resolve should succeed");
    let _val1 = registry
        .resolve(&wn("wf-1"))
        .expect("resolve should succeed");
    let _val2 = registry
        .resolve(&wn("wf-2"))
        .expect("resolve should succeed");
    let _val3 = registry
        .resolve(&wn("wf-3"))
        .expect("resolve should succeed");
}

// ===========================================================================
// Concurrent list (B-REG-62)
// ===========================================================================

// B-REG-62
#[test]
fn list_returns_exactly_n_entries_after_registering_n_workflows_concurrently() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let registry = Arc::new(registry);

    // Pre-create 8 test binaries
    let mut sources = Vec::new();
    let graph0 = valid_graph_single_node("node-0");
    sources.push(make_test_binary(temp_dir.path(), &graph0));
    let graph1 = valid_graph_single_node("node-1");
    sources.push(make_test_binary(temp_dir.path(), &graph1));
    let graph2 = valid_graph_single_node("node-2");
    sources.push(make_test_binary(temp_dir.path(), &graph2));
    let graph3 = valid_graph_single_node("node-3");
    sources.push(make_test_binary(temp_dir.path(), &graph3));
    let graph4 = valid_graph_single_node("node-4");
    sources.push(make_test_binary(temp_dir.path(), &graph4));
    let graph5 = valid_graph_single_node("node-5");
    sources.push(make_test_binary(temp_dir.path(), &graph5));
    let graph6 = valid_graph_single_node("node-6");
    sources.push(make_test_binary(temp_dir.path(), &graph6));
    let graph7 = valid_graph_single_node("node-7");
    sources.push(make_test_binary(temp_dir.path(), &graph7));

    let barrier = Arc::new(Barrier::new(9)); // 8 workers + 1 coordinator

    // When: 8 threads concurrently register
    let mut handles = Vec::new();
    let registry0 = Arc::clone(&registry);
    let barrier0 = Arc::clone(&barrier);
    let source_path0 = bp(&sources[0]);
    let name0 = wn("wf-0");
    handles.push(std::thread::spawn(move || {
        barrier0.wait();
        registry0.register(&source_path0, name0)
    }));

    let registry1 = Arc::clone(&registry);
    let barrier1 = Arc::clone(&barrier);
    let source_path1 = bp(&sources[1]);
    let name1 = wn("wf-1");
    handles.push(std::thread::spawn(move || {
        barrier1.wait();
        registry1.register(&source_path1, name1)
    }));

    let registry2 = Arc::clone(&registry);
    let barrier2 = Arc::clone(&barrier);
    let source_path2 = bp(&sources[2]);
    let name2 = wn("wf-2");
    handles.push(std::thread::spawn(move || {
        barrier2.wait();
        registry2.register(&source_path2, name2)
    }));

    let registry3 = Arc::clone(&registry);
    let barrier3 = Arc::clone(&barrier);
    let source_path3 = bp(&sources[3]);
    let name3 = wn("wf-3");
    handles.push(std::thread::spawn(move || {
        barrier3.wait();
        registry3.register(&source_path3, name3)
    }));

    let registry4 = Arc::clone(&registry);
    let barrier4 = Arc::clone(&barrier);
    let source_path4 = bp(&sources[4]);
    let name4 = wn("wf-4");
    handles.push(std::thread::spawn(move || {
        barrier4.wait();
        registry4.register(&source_path4, name4)
    }));

    let registry5 = Arc::clone(&registry);
    let barrier5 = Arc::clone(&barrier);
    let source_path5 = bp(&sources[5]);
    let name5 = wn("wf-5");
    handles.push(std::thread::spawn(move || {
        barrier5.wait();
        registry5.register(&source_path5, name5)
    }));

    let registry6 = Arc::clone(&registry);
    let barrier6 = Arc::clone(&barrier);
    let source_path6 = bp(&sources[6]);
    let name6 = wn("wf-6");
    handles.push(std::thread::spawn(move || {
        barrier6.wait();
        registry6.register(&source_path6, name6)
    }));

    let registry7 = Arc::clone(&registry);
    let barrier7 = Arc::clone(&barrier);
    let source_path7 = bp(&sources[7]);
    let name7 = wn("wf-7");
    handles.push(std::thread::spawn(move || {
        barrier7.wait();
        registry7.register(&source_path7, name7)
    }));

    // Coordinator waits for all workers
    barrier.wait();

    // Wait for all register threads to complete
    let mut handles_iter = handles.into_iter();
    handles_iter
        .next()
        .unwrap()
        .join()
        .expect("thread should not panic");
    handles_iter
        .next()
        .unwrap()
        .join()
        .expect("thread should not panic");
    handles_iter
        .next()
        .unwrap()
        .join()
        .expect("thread should not panic");
    handles_iter
        .next()
        .unwrap()
        .join()
        .expect("thread should not panic");
    handles_iter
        .next()
        .unwrap()
        .join()
        .expect("thread should not panic");
    handles_iter
        .next()
        .unwrap()
        .join()
        .expect("thread should not panic");
    handles_iter
        .next()
        .unwrap()
        .join()
        .expect("thread should not panic");
    handles_iter
        .next()
        .unwrap()
        .join()
        .expect("thread should not panic");

    // Then: list should have exactly 8 entries
    let entries = registry.list();
    assert_eq!(entries.len(), 8);

    let names: HashSet<&str> = entries.iter().map(|(n, _)| n.as_str()).collect();
    let expected: HashSet<&str> = [
        "wf-0", "wf-1", "wf-2", "wf-3", "wf-4", "wf-5", "wf-6", "wf-7",
    ]
    .iter()
    .copied()
    .collect();
    assert_eq!(names, expected);
}
