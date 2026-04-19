//! Tests for the compensation registry.

use super::*;
use vo_types::{CompensationPolicy, CompensationStatus};

#[test]
fn test_register_single_compensation() {
    let mut registry = CompensationRegistry::new();
    registry
        .register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();

    let entry = registry.get("fx-1").expect("entry exists");
    assert_eq!(entry.status, CompensationRegistryStatus::Registered);
    assert_eq!(entry.policy, CompensationPolicy::Automatic);
    assert_eq!(entry.effect_id, "fx-1");
}

#[test]
fn test_register_duplicate_fails() {
    let mut registry = CompensationRegistry::new();
    registry
        .register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();

    let result = registry.register("fx-1".to_string(), CompensationPolicy::Automatic, vec![]);
    assert!(matches!(
        result,
        Err(CompensationRegistryError::AlreadyRegistered(_))
    ));
}

#[test]
fn test_queue_pending_transitions_to_pending() {
    let mut registry = CompensationRegistry::new();
    registry
        .register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    registry.queue_pending("fx-1").unwrap();

    let entry = registry.get("fx-1").expect("entry exists");
    assert_eq!(entry.status, CompensationRegistryStatus::Pending);
}

#[test]
fn test_queue_pending_none_policy_fails() {
    let mut registry = CompensationRegistry::new();
    registry
        .register("fx-1".to_string(), CompensationPolicy::None, vec![])
        .unwrap();

    let result = registry.queue_pending("fx-1");
    assert!(matches!(
        result,
        Err(CompensationRegistryError::PolicyViolation { .. })
    ));
}

#[test]
fn test_compensation_order_is_reverse_registration() {
    let mut registry = CompensationRegistry::new();
    registry
        .register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    registry
        .register("fx-2".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    registry
        .register("fx-3".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();

    registry.queue_pending("fx-1").unwrap();
    registry.queue_pending("fx-2").unwrap();
    registry.queue_pending("fx-3").unwrap();

    let order = registry.get_compensation_order();
    assert_eq!(order, vec!["fx-3", "fx-2", "fx-1"]);
}

#[test]
fn test_dependencies_block_execution() {
    let mut registry = CompensationRegistry::new();
    registry
        .register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    registry
        .register(
            "fx-2".to_string(),
            CompensationPolicy::Automatic,
            vec!["fx-1".to_string()],
        )
        .unwrap();

    registry.queue_pending("fx-1").unwrap();
    registry.queue_pending("fx-2").unwrap();

    assert!(registry.can_execute("fx-1"));
    assert!(!registry.can_execute("fx-2"));
}

#[test]
fn test_dependencies_satisfied_allows_execution() {
    let mut registry = CompensationRegistry::new();
    registry
        .register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    registry
        .register(
            "fx-2".to_string(),
            CompensationPolicy::Automatic,
            vec!["fx-1".to_string()],
        )
        .unwrap();

    registry.queue_pending("fx-1").unwrap();
    registry.queue_pending("fx-2").unwrap();

    registry.start_compensation("fx-1").unwrap();
    registry.succeed("fx-1").unwrap();

    assert!(registry.can_execute("fx-2"));
}

#[test]
fn test_start_and_succeed_compensation() {
    let mut registry = CompensationRegistry::new();
    registry
        .register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    registry.queue_pending("fx-1").unwrap();
    registry.start_compensation("fx-1").unwrap();

    let entry = registry.get("fx-1").expect("entry exists");
    assert_eq!(entry.status, CompensationRegistryStatus::InProgress);
    assert!(entry.started_at.is_some());

    registry.succeed("fx-1").unwrap();

    let entry = registry.get("fx-1").expect("entry exists");
    assert_eq!(entry.status, CompensationRegistryStatus::Succeeded);
    assert!(entry.completed_at.is_some());
}

#[test]
fn test_fail_compensation() {
    let mut registry = CompensationRegistry::new();
    registry
        .register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    registry.queue_pending("fx-1").unwrap();
    registry.start_compensation("fx-1").unwrap();
    registry.fail("fx-1").unwrap();

    let entry = registry.get("fx-1").expect("entry exists");
    assert_eq!(entry.status, CompensationRegistryStatus::Failed);
}

#[test]
fn test_compensation_with_dependencies_respects_order() {
    let mut registry = CompensationRegistry::new();
    registry
        .register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    registry
        .register("fx-2".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    registry
        .register(
            "fx-3".to_string(),
            CompensationPolicy::Automatic,
            vec!["fx-1".to_string(), "fx-2".to_string()],
        )
        .unwrap();

    registry.queue_pending("fx-1").unwrap();
    registry.queue_pending("fx-2").unwrap();
    registry.queue_pending("fx-3").unwrap();

    let order = registry.get_compensation_order();
    assert_eq!(order, vec!["fx-3", "fx-2", "fx-1"]);
}

#[test]
fn test_compensation_entry_new() {
    let entry = CompensationEntry::new("fx-1".to_string(), CompensationPolicy::Automatic, vec![]);

    assert_eq!(entry.effect_id, "fx-1");
    assert_eq!(entry.policy, CompensationPolicy::Automatic);
    assert_eq!(entry.status, CompensationRegistryStatus::Registered);
    assert!(entry.registered_at.as_u64() > 0);
    assert!(entry.compensation_effect_id.is_none());
    assert!(entry.started_at.is_none());
    assert!(entry.completed_at.is_none());
    assert!(entry.dependencies.is_empty());
}

#[test]
fn test_compensation_entry_with_timeout() {
    let entry = CompensationEntry::new("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .with_timeout(5000);

    assert_eq!(entry.timeout_ms, Some(5000));
}

#[test]
fn test_compensation_entry_with_compensation_effect_id() {
    let entry = CompensationEntry::new("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .with_compensation_effect_id("comp-1".to_string());

    assert_eq!(entry.compensation_effect_id, Some("comp-1".to_string()));
}

#[test]
fn test_compensation_entry_is_terminal() {
    let succeeded =
        CompensationEntry::new("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
            .with_compensation_effect_id("comp-1".to_string());
    let mut succeeded = succeeded.clone();
    succeeded.status = CompensationRegistryStatus::Succeeded;

    let failed = succeeded.clone();
    let mut failed = failed;
    failed.status = CompensationRegistryStatus::Failed;

    let timed_out = failed.clone();
    let mut timed_out = timed_out;
    timed_out.status = CompensationRegistryStatus::TimedOut;

    let pending = succeeded.clone();
    let mut pending = pending;
    pending.status = CompensationRegistryStatus::Pending;

    let in_progress = pending.clone();
    let mut in_progress = in_progress;
    in_progress.status = CompensationRegistryStatus::InProgress;

    let registered = in_progress.clone();
    let mut registered = registered;
    registered.status = CompensationRegistryStatus::Registered;

    assert!(succeeded.is_terminal());
    assert!(failed.is_terminal());
    assert!(timed_out.is_terminal());
    assert!(!pending.is_terminal());
    assert!(!in_progress.is_terminal());
    assert!(!registered.is_terminal());
}

#[test]
fn test_compensation_entry_is_timed_out() {
    use std::thread::sleep;
    use std::time::Duration;

    let entry = CompensationEntry::new("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .with_timeout(100);
    let mut entry = entry.clone();
    entry.status = CompensationRegistryStatus::InProgress;
    entry.started_at = Some(TimestampMs::now());

    sleep(Duration::from_millis(150));

    assert!(entry.is_timed_out(TimestampMs::now()));

    let entry = entry.clone();
    assert!(!entry.is_timed_out(entry.started_at.unwrap()));
}

#[test]
fn test_shared_compensation_registry_new() {
    let registry = SharedCompensationRegistry::new();
    assert_eq!(registry.version(), 0);
}

#[test]
fn test_shared_compensation_registry_register() {
    let registry = SharedCompensationRegistry::new();
    registry
        .register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();

    let entry = registry.get("fx-1").expect("entry exists");
    assert_eq!(entry.policy, CompensationPolicy::Automatic);
}

#[test]
fn test_shared_compensation_registry_version_increments() {
    let registry = SharedCompensationRegistry::new();
    assert_eq!(registry.version(), 0);

    registry
        .register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    assert_eq!(registry.version(), 1);

    registry
        .register("fx-2".to_string(), CompensationPolicy::Manual, vec![])
        .unwrap();
    assert_eq!(registry.version(), 2);
}

#[test]
fn test_shared_compensation_registry_compensation_order() {
    let registry = SharedCompensationRegistry::new();
    registry
        .register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    registry
        .register("fx-2".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    registry
        .register("fx-3".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();

    registry.queue_pending("fx-1").unwrap();
    registry.queue_pending("fx-2").unwrap();
    registry.queue_pending("fx-3").unwrap();

    let order = registry.get_compensation_order();
    assert_eq!(order, vec!["fx-3", "fx-2", "fx-1"]);
}

#[test]
fn test_compensation_status_from_compensation_status() {
    assert_eq!(
        CompensationRegistryStatus::from(CompensationStatus::NotNeeded),
        CompensationRegistryStatus::Succeeded
    );
    assert_eq!(
        CompensationRegistryStatus::from(CompensationStatus::Pending),
        CompensationRegistryStatus::Pending
    );
    assert_eq!(
        CompensationRegistryStatus::from(CompensationStatus::InProgress),
        CompensationRegistryStatus::InProgress
    );
    assert_eq!(
        CompensationRegistryStatus::from(CompensationStatus::Succeeded),
        CompensationRegistryStatus::Succeeded
    );
    assert_eq!(
        CompensationRegistryStatus::from(CompensationStatus::Failed),
        CompensationRegistryStatus::Failed
    );
}

#[test]
fn test_compensation_registry_empty() {
    let registry = CompensationRegistry::new();
    assert!(registry.get("fx-1").is_none());
    assert!(registry.pending_compensations().next().is_none());
    assert!(registry.in_progress_compensations().next().is_none());
    assert!(registry.ambiguous_compensations().next().is_none());
    assert!(registry.timed_out_compensations().next().is_none());
    assert!(registry.compensations_awaiting_execution().is_empty());
    assert_eq!(registry.get_compensation_order().len(), 0);
}

#[test]
fn test_compensation_registry_all_entries() {
    let mut registry = CompensationRegistry::new();
    registry
        .register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    registry
        .register("fx-2".to_string(), CompensationPolicy::Manual, vec![])
        .unwrap();

    let entries: Vec<_> = registry.all_entries().collect();
    assert_eq!(entries.len(), 2);
}

// ============================================================================
// Forward-Effect to Compensation-Effect Linkage Tests
// ============================================================================

#[test]
fn test_register_compensation_for_committed_effect() {
    let mut registry = CompensationRegistry::new();

    registry
        .register_compensation_for_committed_effect(
            "fx-1".to_string(),
            CompensationPolicy::Automatic,
            "comp-1".to_string(),
            vec![],
        )
        .unwrap();

    let entry = registry.get("fx-1").expect("entry exists");
    assert_eq!(entry.effect_id, "fx-1");
    assert_eq!(entry.policy, CompensationPolicy::Automatic);
    assert_eq!(entry.compensation_effect_id, Some("comp-1".to_string()));
    assert_eq!(entry.status, CompensationRegistryStatus::Registered);
}

#[test]
fn test_register_compensation_for_committed_effect_duplicate_fails() {
    let mut registry = CompensationRegistry::new();

    registry
        .register_compensation_for_committed_effect(
            "fx-1".to_string(),
            CompensationPolicy::Automatic,
            "comp-1".to_string(),
            vec![],
        )
        .unwrap();

    let result = registry.register_compensation_for_committed_effect(
        "fx-1".to_string(),
        CompensationPolicy::Automatic,
        "comp-1b".to_string(),
        vec![],
    );

    assert!(matches!(
        result,
        Err(CompensationRegistryError::AlreadyRegistered(_))
    ));
}

#[test]
fn test_get_compensation_effect_id() {
    let mut registry = CompensationRegistry::new();

    registry
        .register_compensation_for_committed_effect(
            "fx-1".to_string(),
            CompensationPolicy::Automatic,
            "comp-1".to_string(),
            vec![],
        )
        .unwrap();

    let comp_id = registry.get_compensation_effect_id("fx-1");
    assert_eq!(comp_id, Some(&"comp-1".to_string()));

    let missing = registry.get_compensation_effect_id("fx-2");
    assert!(missing.is_none());
}

#[test]
fn test_has_compensation() {
    let mut registry = CompensationRegistry::new();

    registry
        .register_compensation_for_committed_effect(
            "fx-1".to_string(),
            CompensationPolicy::Automatic,
            "comp-1".to_string(),
            vec![],
        )
        .unwrap();

    assert!(registry.has_compensation("fx-1"));
    assert!(!registry.has_compensation("fx-2"));
}

#[test]
fn test_is_irreversible() {
    let mut registry = CompensationRegistry::new();

    registry
        .register_compensation_for_committed_effect(
            "fx-1".to_string(),
            CompensationPolicy::None,
            "comp-1".to_string(),
            vec![],
        )
        .unwrap();

    registry
        .register_compensation_for_committed_effect(
            "fx-2".to_string(),
            CompensationPolicy::Automatic,
            "comp-2".to_string(),
            vec![],
        )
        .unwrap();

    assert!(registry.is_irreversible("fx-1"));
    assert!(!registry.is_irreversible("fx-2"));
    assert!(!registry.is_irreversible("fx-3"));
}

#[test]
fn test_compensations_ready_to_execute() {
    let mut registry = CompensationRegistry::new();

    registry
        .register_compensation_for_committed_effect(
            "fx-1".to_string(),
            CompensationPolicy::Automatic,
            "comp-1".to_string(),
            vec![],
        )
        .unwrap();

    registry
        .register_compensation_for_committed_effect(
            "fx-2".to_string(),
            CompensationPolicy::Automatic,
            "comp-2".to_string(),
            vec!["fx-1".to_string()],
        )
        .unwrap();

    // Queue both for execution
    registry.queue_pending("fx-1").unwrap();
    registry.queue_pending("fx-2").unwrap();

    // Only fx-1 is ready (fx-2 depends on fx-1)
    let ready = registry.compensations_ready_to_execute();
    assert_eq!(ready, vec!["fx-1"]);

    // Execute fx-1
    registry.start_compensation("fx-1").unwrap();
    registry.succeed("fx-1").unwrap();

    // Now fx-2 is ready
    let ready = registry.compensations_ready_to_execute();
    assert_eq!(ready, vec!["fx-2"]);
}

#[test]
fn test_compensation_linkage_preserves_dependencies() {
    let mut registry = CompensationRegistry::new();

    registry
        .register_compensation_for_committed_effect(
            "fx-1".to_string(),
            CompensationPolicy::Automatic,
            "comp-1".to_string(),
            vec![],
        )
        .unwrap();

    registry
        .register_compensation_for_committed_effect(
            "fx-2".to_string(),
            CompensationPolicy::Automatic,
            "comp-2".to_string(),
            vec!["fx-1".to_string()],
        )
        .unwrap();

    let entry2 = registry.get("fx-2").expect("entry exists");
    assert_eq!(entry2.dependencies, vec!["fx-1".to_string()]);
    assert_eq!(entry2.compensation_effect_id, Some("comp-2".to_string()));
}

#[test]
fn test_compensation_linkage_with_manual_policy() {
    let mut registry = CompensationRegistry::new();

    registry
        .register_compensation_for_committed_effect(
            "fx-1".to_string(),
            CompensationPolicy::Manual,
            "comp-1".to_string(),
            vec![],
        )
        .unwrap();

    let entry = registry.get("fx-1").expect("entry exists");
    assert_eq!(entry.policy, CompensationPolicy::Manual);
    assert_eq!(entry.compensation_effect_id, Some("comp-1".to_string()));
    assert_eq!(entry.status, CompensationRegistryStatus::Registered);

    // Manual policy CAN be queued (only None policy blocks queueing)
    // Manual means it requires explicit operator approval, not that it can't be queued
    registry.queue_pending("fx-1").unwrap();
    let entry = registry.get("fx-1").expect("entry exists");
    assert_eq!(entry.status, CompensationRegistryStatus::Pending);
}

#[test]
fn test_none_policy_blocks_queue_pending() {
    let mut registry = CompensationRegistry::new();

    registry
        .register_compensation_for_committed_effect(
            "fx-1".to_string(),
            CompensationPolicy::None,
            "comp-1".to_string(),
            vec![],
        )
        .unwrap();

    // None policy cannot be queued - irreversible effects don't need compensation
    let result = registry.queue_pending("fx-1");
    assert!(matches!(
        result,
        Err(CompensationRegistryError::PolicyViolation { .. })
    ));
}

#[test]
fn test_compensation_linkage_with_timeout() {
    let mut registry = CompensationRegistry::new();

    registry
        .register_compensation_for_committed_effect(
            "fx-1".to_string(),
            CompensationPolicy::Automatic,
            "comp-1".to_string(),
            vec![],
        )
        .unwrap();

    let entry = registry.get("fx-1").expect("entry exists");
    assert!(entry.timeout_ms.is_none());

    // Timeout is set separately via register_with_timeout
    // This test verifies linkage works independently of timeout
    assert_eq!(entry.compensation_effect_id, Some("comp-1".to_string()));
}
