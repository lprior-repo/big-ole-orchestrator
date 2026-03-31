//! Adversarial tests for wtf-engine BinaryRegistry.
//!
//! Red Queen Generation 1 — attacking contract invariants, edge cases,
//! and failure modes. Exit codes are ground truth.

mod common;

use std::fs::Permissions;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::sync::{Arc, Barrier};
use std::thread;

use common::*;
use wtf_engine::*;

// ===========================================================================
// Contract Violations
// ===========================================================================

/// ADV-001: INV-8 violation — reap() checks global active_instances,
/// not per-workflow. When workflow B has an active instance, workflow A
/// (deactivated, zero instances) is incorrectly skipped.
#[test]
fn adv001_reap_should_reap_deactivated_with_zero_instances_even_when_others_have_active() {
    let (temp_dir, registry) = create_test_registry();

    let src_a = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    registry
        .register(&bp(&src_a), wn("wf-a"))
        .expect("register a");
    registry.deactivate(&wn("wf-a")).expect("deactivate a");

    let src_b = make_test_binary(temp_dir.path(), &valid_two_node_graph());
    registry
        .register(&bp(&src_b), wn("wf-b"))
        .expect("register b");

    let report = registry.reap(|name| name.as_str() == "wf-b");

    assert!(
        report.reaped.iter().any(|n| n.as_str() == "wf-a"),
        "INV-8 DEFECT: wf-a has zero active instances and should be reaped, \
         but was skipped due to global active_instances check"
    );
    assert!(
        !report.skipped.iter().any(|n| n.as_str() == "wf-a"),
        "INV-8 DEFECT: wf-a should not be in skipped list"
    );
}

// ===========================================================================
// Concurrent Access
// ===========================================================================

/// ADV-002: Two threads register DIFFERENT binaries under the SAME workflow
/// name. DashMap insert is last-write-wins. No panic should occur.
#[test]
fn adv002_concurrent_register_same_workflow_last_writer_wins_no_panic() {
    let (temp_dir, registry) = create_test_registry();
    let registry = Arc::new(registry);

    let src1 = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    let src2 = make_test_binary(temp_dir.path(), &valid_two_node_graph());

    let barrier = Arc::new(Barrier::new(3));
    let mut handles: Vec<thread::JoinHandle<Result<(), BinaryRegistryError>>> = Vec::new();

    for src in [src1, src2] {
        let reg = Arc::clone(&registry);
        let bar = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            bar.wait();
            reg.register(&bp(&src), wn("same-wf"))
        }));
    }

    barrier.wait();
    let results: Vec<Result<(), BinaryRegistryError>> = handles
        .into_iter()
        .map(|h| h.join().expect("no panic"))
        .collect();

    assert!(
        results.iter().all(|r| r.is_ok()),
        "both concurrent registers should succeed: {:?}",
        results
    );

    let resolved = registry.resolve(&wn("same-wf")).expect("resolve");
    assert!(
        resolved.2.nodes.len() == 1 || resolved.2.nodes.len() == 2,
        "definition should be from one of the two registrations"
    );
    assert_eq!(registry.len(), 1, "only one entry should exist");
}

/// ADV-003: Concurrent deactivate + resolve — resolve should return either
/// Ok or WorkflowDeactivated, never panic or NotFound.
#[test]
fn adv003_concurrent_deactivate_and_resolve_no_panic_or_wrong_error() {
    let (temp_dir, registry) = create_test_registry();
    let registry = Arc::new(registry);

    let src = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    registry
        .register(&bp(&src), wn("race-wf"))
        .expect("register");

    let barrier = Arc::new(Barrier::new(3));
    let mut handles: Vec<thread::JoinHandle<()>> = Vec::new();

    let reg1 = Arc::clone(&registry);
    let bar1 = Arc::clone(&barrier);
    handles.push(thread::spawn(move || {
        bar1.wait();
        let _ = reg1.deactivate(&wn("race-wf"));
    }));

    let reg2 = Arc::clone(&registry);
    let bar2 = Arc::clone(&barrier);
    handles.push(thread::spawn(move || {
        bar2.wait();
        for _ in 0..100 {
            match reg2.resolve(&wn("race-wf")) {
                Ok(_) | Err(BinaryRegistryError::WorkflowDeactivated { .. }) => {}
                Err(other) => panic!(
                    "resolve returned unexpected error during concurrent deactivate: {:?}",
                    other
                ),
            }
        }
    }));

    barrier.wait();
    for h in handles {
        h.join().expect("no panic");
    }
}

/// ADV-004: Concurrent register + deactivate — no panic, consistent state.
#[test]
fn adv004_concurrent_register_and_deactivate_no_panic() {
    let (temp_dir, registry) = create_test_registry();
    let registry = Arc::new(registry);

    let barrier = Arc::new(Barrier::new(3));
    let mut handles: Vec<thread::JoinHandle<()>> = Vec::new();

    let reg1 = Arc::clone(&registry);
    let bar1 = Arc::clone(&barrier);
    let src = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    handles.push(thread::spawn(move || {
        bar1.wait();
        for _ in 0..50 {
            let _ = reg1.register(&bp(&src), wn("reg-deact-wf"));
        }
    }));

    let reg2 = Arc::clone(&registry);
    let bar2 = Arc::clone(&barrier);
    handles.push(thread::spawn(move || {
        bar2.wait();
        for _ in 0..50 {
            let _ = reg2.deactivate(&wn("reg-deact-wf"));
        }
    }));

    barrier.wait();
    for h in handles {
        h.join().expect("no panic");
    }

    assert!(registry.len() <= 1, "at most one entry should exist");
}

/// ADV-005: Concurrent reap + resolve — resolve should never panic.
#[test]
fn adv005_concurrent_reap_and_resolve_no_panic() {
    let (temp_dir, registry) = create_test_registry();
    let registry = Arc::new(registry);

    let src = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    registry
        .register(&bp(&src), wn("reap-resolve-wf"))
        .expect("register");
    registry
        .deactivate(&wn("reap-resolve-wf"))
        .expect("deactivate");

    let barrier = Arc::new(Barrier::new(3));
    let mut handles: Vec<thread::JoinHandle<()>> = Vec::new();

    let reg1 = Arc::clone(&registry);
    let bar1 = Arc::clone(&barrier);
    handles.push(thread::spawn(move || {
        bar1.wait();
        for _ in 0..100 {
            let _ = reg1.reap(|_| false);
        }
    }));

    let reg2 = Arc::clone(&registry);
    let bar2 = Arc::clone(&barrier);
    handles.push(thread::spawn(move || {
        bar2.wait();
        for _ in 0..100 {
            match reg2.resolve(&wn("reap-resolve-wf")) {
                Ok(_)
                | Err(BinaryRegistryError::WorkflowDeactivated { .. })
                | Err(BinaryRegistryError::NotFound { .. }) => {}
                Err(other) => panic!(
                    "unexpected error during concurrent reap+resolve: {:?}",
                    other
                ),
            }
        }
    }));

    barrier.wait();
    for h in handles {
        h.join().expect("no panic");
    }
}

// ===========================================================================
// Edge Cases
// ===========================================================================

/// ADV-006: Full lifecycle restart — register, deactivate, reap, register again.
#[test]
fn adv006_full_lifecycle_restart_register_after_reap_succeeds() {
    let (temp_dir, registry) = create_test_registry();

    let src1 = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    registry
        .register(&bp(&src1), wn("restart-wf"))
        .expect("register 1");
    registry.deactivate(&wn("restart-wf")).expect("deactivate");

    let report = registry.reap(|_| false);
    assert_eq!(report.reaped, vec![wn("restart-wf")]);

    assert!(matches!(
        registry.resolve(&wn("restart-wf")),
        Err(BinaryRegistryError::NotFound { .. })
    ));

    let src2 = make_test_binary(temp_dir.path(), &valid_two_node_graph());
    let result = registry.register(&bp(&src2), wn("restart-wf"));
    assert_eq!(
        result,
        Ok(()),
        "re-registration after full lifecycle should succeed"
    );

    let (_, _, def) = registry
        .resolve(&wn("restart-wf"))
        .expect("resolve after restart");
    assert_eq!(def.nodes.len(), 2);
}

/// ADV-007: Multiple consecutive reap calls — second should be no-op.
#[test]
fn adv007_multiple_consecutive_reap_calls_are_idempotent() {
    let (temp_dir, registry) = create_test_registry();

    let src = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    registry
        .register(&bp(&src), wn("reap-twice-wf"))
        .expect("register");
    registry
        .deactivate(&wn("reap-twice-wf"))
        .expect("deactivate");

    let report1 = registry.reap(|_| false);
    assert_eq!(report1.reaped.len(), 1);

    let report2 = registry.reap(|_| false);
    assert!(
        report2.reaped.is_empty(),
        "second reap should find nothing to reap"
    );
    assert!(report2.skipped.is_empty());
    assert!(report2.failures.is_empty());
}

/// ADV-008: Reap when only Active entries exist — nothing should be reaped.
#[test]
fn adv008_reap_with_only_active_entries_reaps_nothing() {
    let (temp_dir, registry) = create_test_registry();

    let src = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    registry
        .register(&bp(&src), wn("active-only-wf"))
        .expect("register");

    let report = registry.reap(|_| false);
    assert!(report.reaped.is_empty());
    assert!(report.skipped.is_empty());
    assert!(report.failures.is_empty());
    assert_eq!(registry.len(), 1);
}

/// ADV-009: Register with source as a broken symlink — should return BinaryNotFound.
#[test]
fn adv009_register_with_broken_symlink_returns_binary_not_found() {
    let (temp_dir, registry) = create_test_registry();

    let target = temp_dir.path().join("does-not-exist-target");
    let symlink = temp_dir.path().join("broken-symlink");
    std::os::unix::fs::symlink(&target, &symlink).expect("symlink");

    let result = registry.register(&bp(&symlink), wn("broken-sym-wf"));
    assert!(
        matches!(result, Err(BinaryRegistryError::BinaryNotFound { .. })),
        "broken symlink should be BinaryNotFound"
    );
    assert_eq!(registry.len(), 0);
}

/// ADV-010: Register with source as a symlink to an executable — should succeed.
#[test]
fn adv010_register_with_symlink_to_executable_succeeds() {
    let (temp_dir, registry) = create_test_registry();

    let src = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    let symlink = temp_dir.path().join("link-to-binary");
    std::os::unix::fs::symlink(&src, &symlink).expect("symlink");

    let result = registry.register(&bp(&symlink), wn("symlink-wf"));
    assert_eq!(result, Ok(()), "symlink to executable should register");

    let (_, _, def) = registry.resolve(&wn("symlink-wf")).expect("resolve");
    assert_eq!(def.nodes.len(), 1);
}

/// ADV-011: Register binary that exits 0 with empty stdout — should fail with
/// InvalidGraphOutput (empty string is not valid JSON).
#[test]
fn adv011_register_binary_empty_graph_stdout_returns_invalid_graph_output() {
    let (temp_dir, registry) = create_test_registry();

    let src = make_test_binary_empty_stdout(temp_dir.path());
    let result = registry.register(&bp(&src), wn("empty-stdout-wf"));

    assert!(
        matches!(
            result,
            Err(BinaryRegistryError::InvalidGraphOutput { .. })
                | Err(BinaryRegistryError::WorkflowDefinitionInvalid { .. })
        ),
        "empty --graph stdout should produce InvalidGraphOutput or WorkflowDefinitionInvalid"
    );
    assert_eq!(registry.len(), 0);
}

/// ADV-012: Register binary whose --graph writes to stderr but exits 0 — should succeed.
#[test]
fn adv012_register_binary_stderr_on_graph_exit_zero_succeeds() {
    let (temp_dir, registry) = create_test_registry();

    let script_path = temp_dir
        .path()
        .join(format!("test-binary-stderr-{}", ulid::Ulid::new()));
    let mut file = std::fs::File::create(&script_path).expect("create");
    write!(
        file,
        r#"#!/bin/sh
if [ "$1" = "--graph" ]; then
    printf 'debug info\n' >&2
    cat <<'GRAPH_EOF'
{graph}
GRAPH_EOF
    exit 0
fi
exit 1
"#,
        graph = valid_single_node_graph()
    )
    .expect("write");
    drop(file);
    std::fs::set_permissions(&script_path, Permissions::from_mode(0o755)).expect("chmod");

    let result = registry.register(&bp(&script_path), wn("stderr-wf"));
    assert_eq!(
        result,
        Ok(()),
        "binary with stderr but exit 0 should succeed"
    );
}

/// ADV-013: Register binary killed by signal — should return GraphDiscoveryFailed.
#[test]
fn adv013_register_binary_killed_by_signal_returns_graph_discovery_failed() {
    let (temp_dir, registry) = create_test_registry();

    let script_path = temp_dir
        .path()
        .join(format!("test-binary-signal-{}", ulid::Ulid::new()));
    let mut file = std::fs::File::create(&script_path).expect("create");
    write!(
        file,
        r#"#!/bin/sh
if [ "$1" = "--graph" ]; then
    kill -9 $$
fi
exit 1
"#
    )
    .expect("write");
    drop(file);
    std::fs::set_permissions(&script_path, Permissions::from_mode(0o755)).expect("chmod");

    let result = registry.register(&bp(&script_path), wn("signal-wf"));
    assert!(
        matches!(
            result,
            Err(BinaryRegistryError::GraphDiscoveryFailed { .. })
        ),
        "binary killed by signal should produce GraphDiscoveryFailed"
    );
    assert_eq!(registry.len(), 0);
}

/// ADV-014: Register binary producing very large graph output (1MB) — should
/// not panic, either succeed or fail gracefully.
#[test]
fn adv014_register_binary_with_large_graph_output_does_not_panic() {
    let (temp_dir, registry) = create_test_registry();

    let padding = r#","extra_field":"#;
    let large_value = "x".repeat(1_000_000);
    let large_graph = format!(
        r#"{{"workflow_name":"large-wf","nodes":[{{"node_name":"n","retry_policy":{{"max_attempts":3,"backoff_ms":1000,"backoff_multiplier":2.0}}{padding}"{large_value}"}}],"edges":[]"}}"#
    );

    let script_path = temp_dir
        .path()
        .join(format!("test-binary-large-{}", ulid::Ulid::new()));
    let mut file = std::fs::File::create(&script_path).expect("create");
    write!(
        file,
        r#"#!/bin/sh
if [ "$1" = "--graph" ]; then
    printf '%s' '{graph}'
    exit 0
fi
exit 1
"#,
        graph = large_graph
    )
    .expect("write");
    drop(file);
    std::fs::set_permissions(&script_path, Permissions::from_mode(0o755)).expect("chmod");

    let result = registry.register(&bp(&script_path), wn("large-wf"));
    match result {
        Ok(()) => {
            let resolved = registry.resolve(&wn("large-wf"));
            assert!(
                resolved.is_ok(),
                "if register succeeded, resolve should too"
            );
        }
        Err(e) => {
            assert!(
                matches!(
                    e,
                    BinaryRegistryError::InvalidGraphOutput { .. }
                        | BinaryRegistryError::WorkflowDefinitionInvalid { .. }
                        | BinaryRegistryError::GraphDiscoveryFailed { .. }
                ),
                "large output should fail with a graph-related error, got: {:?}",
                e
            );
        }
    }
}

/// ADV-015: Concurrent register of 16 workflows then deactivate all and reap.
#[test]
fn adv015_stress_16_concurrent_registrations_then_deactivate_and_reap() {
    let (temp_dir, registry) = create_test_registry();
    let registry = Arc::new(registry);

    let n: usize = 16;
    let barrier = Arc::new(Barrier::new(n + 1));
    let mut handles: Vec<thread::JoinHandle<Result<(), BinaryRegistryError>>> = Vec::new();

    for i in 0..n {
        let reg = Arc::clone(&registry);
        let bar = Arc::clone(&barrier);
        let graph = valid_graph_single_node(&format!("node-{i}"));
        let src = make_test_binary(temp_dir.path(), &graph);
        handles.push(thread::spawn(move || {
            bar.wait();
            reg.register(&bp(&src), wn(&format!("stress-{i}")))
        }));
    }

    barrier.wait();
    for h in handles {
        let r = h.join().expect("no panic");
        assert!(r.is_ok(), "register should succeed");
    }

    assert_eq!(registry.len(), n);

    for i in 0..n {
        registry
            .deactivate(&wn(&format!("stress-{i}")))
            .expect("deactivate");
    }

    let report = registry.reap(|_| false);
    assert_eq!(report.reaped.len(), n);
    assert_eq!(registry.len(), 0);
}

/// ADV-016: Reap should not affect Active workflows even when active_instances
/// is empty.
#[test]
fn adv016_reap_preserves_active_workflows_when_active_instances_empty() {
    let (temp_dir, registry) = create_test_registry();

    let src = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    registry
        .register(&bp(&src), wn("stay-active"))
        .expect("register");

    let report = registry.reap(|_| false);

    assert!(
        report.reaped.is_empty(),
        "active workflow should not be reaped"
    );
    assert!(
        report.skipped.is_empty(),
        "active workflow should not be skipped"
    );
    assert_eq!(registry.len(), 1);
    assert!(registry.resolve(&wn("stay-active")).is_ok());
}

/// ADV-017: Register after deactivate (without reap) — re-activation.
#[test]
fn adv017_register_after_deactivate_without_reap_reactivates_workflow() {
    let (temp_dir, registry) = create_test_registry();

    let src1 = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    registry
        .register(&bp(&src1), wn("reactivate-wf"))
        .expect("register 1");
    registry
        .deactivate(&wn("reactivate-wf"))
        .expect("deactivate");

    assert!(matches!(
        registry.resolve(&wn("reactivate-wf")),
        Err(BinaryRegistryError::WorkflowDeactivated { .. })
    ));

    let src2 = make_test_binary(temp_dir.path(), &valid_two_node_graph());
    let result = registry.register(&bp(&src2), wn("reactivate-wf"));

    assert_eq!(result, Ok(()));

    let (_, _, def) = registry.resolve(&wn("reactivate-wf")).expect("resolve");
    assert_eq!(
        def.nodes.len(),
        2,
        "re-activated workflow should have new definition"
    );
}

/// ADV-018: Register with source path that is a directory — BinaryNotFound.
#[test]
fn adv018_register_with_directory_source_returns_binary_not_found() {
    let (temp_dir, registry) = create_test_registry();

    let dir_path = temp_dir.path().join("a-dir");
    std::fs::create_dir_all(&dir_path).expect("mkdir");

    let result = registry.register(&bp(&dir_path), wn("dir-wf"));
    assert!(
        matches!(result, Err(BinaryRegistryError::BinaryNotFound { .. })),
        "directory source should be BinaryNotFound"
    );
}

/// ADV-019: Concurrent reap from multiple threads — no panic, no double-reap.
#[test]
fn adv019_concurrent_reap_from_multiple_threads_no_panic() {
    let (temp_dir, registry) = create_test_registry();
    let registry = Arc::new(registry);

    let src = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    registry
        .register(&bp(&src), wn("multi-reap-wf"))
        .expect("register");
    registry
        .deactivate(&wn("multi-reap-wf"))
        .expect("deactivate");

    let barrier = Arc::new(Barrier::new(5));
    let mut handles: Vec<thread::JoinHandle<()>> = Vec::new();

    for _ in 0..4 {
        let reg = Arc::clone(&registry);
        let bar = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            bar.wait();
            let _ = reg.reap(|_| false);
        }));
    }

    barrier.wait();
    for h in handles {
        h.join().expect("no panic");
    }

    assert!(
        registry.len() <= 1,
        "at most one entry should remain (removed by first reaper)"
    );
}

/// ADV-020: Deactivate already-deactivated then reap — deactivate is idempotent,
/// reap should still work.
#[test]
fn adv020_double_deactivate_then_reap_still_works() {
    let (temp_dir, registry) = create_test_registry();

    let src = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    registry
        .register(&bp(&src), wn("double-deact-reap"))
        .expect("register");
    registry
        .deactivate(&wn("double-deact-reap"))
        .expect("deactivate 1");
    registry
        .deactivate(&wn("double-deact-reap"))
        .expect("deactivate 2");

    let report = registry.reap(|_| false);
    assert_eq!(report.reaped.len(), 1);
    assert!(matches!(
        registry.resolve(&wn("double-deact-reap")),
        Err(BinaryRegistryError::NotFound { .. })
    ));
}
