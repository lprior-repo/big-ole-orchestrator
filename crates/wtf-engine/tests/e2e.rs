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
        std::fs::metadata(versioned_path.as_path()).is_err(),
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
    for i in 0..4u32 {
        let graph = valid_graph_single_node(&format!("node-{i}"));
        let source = make_test_binary(temp_dir.path(), &graph);
        sources.push(source);
    }

    let barrier = Arc::new(Barrier::new(5)); // 4 workers + 1 coordinator

    // When: 4 threads concurrently register
    let mut handles = Vec::new();
    for i in 0..4u32 {
        let registry = Arc::clone(&registry);
        let barrier = Arc::clone(&barrier);
        let source_path = bp(&sources[i as usize]);
        let name = wn(&format!("wf-{i}"));

        handles.push(std::thread::spawn(move || {
            barrier.wait();
            registry.register(&source_path, name)
        }));
    }

    // Coordinator waits for all workers
    barrier.wait();

    // Wait for all register threads to complete
    for handle in handles {
        let _ = handle.join().expect("thread should not panic");
    }

    // Then: 4 threads concurrently resolve
    let barrier2 = Arc::new(Barrier::new(5));
    let mut resolve_handles = Vec::new();
    for i in 0..4u32 {
        let registry = Arc::clone(&registry);
        let barrier = Arc::clone(&barrier2);
        let name = wn(&format!("wf-{i}"));

        resolve_handles.push(std::thread::spawn(move || {
            barrier.wait();
            if registry.resolve(&name).is_err() {
                panic!("resolve should succeed");
            }
        }));
    }

    barrier2.wait();

    for handle in resolve_handles {
        handle.join().expect("thread should not panic");
    }

    // Then: len should be 4
    assert_eq!(registry.len(), 4);

    // And each resolve should succeed
    for i in 0..4u32 {
        let name = wn(&format!("wf-{i}"));
        let _ = registry.resolve(&name).expect("resolve should succeed");
    }
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
    for i in 0..8u32 {
        let graph = valid_graph_single_node(&format!("node-{i}"));
        let source = make_test_binary(temp_dir.path(), &graph);
        sources.push(source);
    }

    let barrier = Arc::new(Barrier::new(9)); // 8 workers + 1 coordinator

    // When: 8 threads concurrently register
    let mut handles = Vec::new();
    for i in 0..8u32 {
        let registry = Arc::clone(&registry);
        let barrier = Arc::clone(&barrier);
        let source_path = bp(&sources[i as usize]);
        let name = wn(&format!("wf-{i}"));

        handles.push(std::thread::spawn(move || {
            barrier.wait();
            registry.register(&source_path, name)
        }));
    }

    // Coordinator waits for all workers
    barrier.wait();

    // Wait for all register threads to complete
    for handle in handles {
        let _ = handle.join().expect("thread should not panic");
    }

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
