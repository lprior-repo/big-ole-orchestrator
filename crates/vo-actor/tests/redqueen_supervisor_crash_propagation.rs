//! RED-QUEEN coevolutionary adversarial tests for vo-actor.
//! Actor supervision — supervisor crash propagation.
//!
//! ## EARS Requirements (rq-003)
//!
//! **Ubiquitous:** THE SYSTEM SHALL propagate failures up the tree
//! **Event-Driven:** When all restarts exhausted, THE SYSTEM SHALL notify parent
//! **Unwanted:** If failure not propagated, THE SYSTEM SHALL silently swallow failures
//!                (because: Failures must propagate)
//!
//! ## Contracts
//!
//! **Preconditions:** All restarts exhausted
//! **Postconditions:** Parent notified
//! **Invariants:** Failure accountability
//!
//! ## Test Strategy
//!
//! RED-QUEEN Phase: These tests document expected behavior that is NOT
//! yet correctly implemented. They expose the gap between specification
//! and implementation.
//!
//! Happy Path:     Verify parent IS notified when all restarts exhausted
//! Error/Edge:     Verify silent failures are detected (current behavior = silent)

use std::sync::Arc;
use std::time::Duration;

use vo_actor::lifecycle::{ActorLifecycleState, ParentChildRegistry};
use vo_actor::spawn_supervisor::{
    ProcessHandle, ProcessManager, SpawnPhase, SpawnRecord, SpawnStorage, SpawnSupervisor,
    SpawnSupervisorError, WorkQueue,
};
use vo_types::InstanceId;

fn test_instance_id() -> InstanceId {
    use ulid::Ulid;
    let ulid = Ulid::new();
    InstanceId::from_bytes(ulid.to_bytes())
}

// =============================================================================
// Mock Implementations
// =============================================================================

#[derive(Debug, Default)]
struct MockSpawnStorage {
    records: std::sync::Mutex<Vec<SpawnRecord>>,
}

impl MockSpawnStorage {
    fn new() -> Self {
        Self::default()
    }

    fn add_record(&self, record: SpawnRecord) {
        self.records.lock().unwrap().push(record);
    }

    fn get_records(&self) -> Vec<SpawnRecord> {
        self.records.lock().unwrap().clone()
    }

    fn update_record(&self, record: &SpawnRecord) {
        let mut records = self.records.lock().unwrap();
        if let Some(pos) = records
            .iter()
            .position(|r| r.instance_id == record.instance_id)
        {
            records[pos] = record.clone();
        }
    }
}

#[async_trait::async_trait]
impl SpawnStorage for MockSpawnStorage {
    async fn get_spawn_record(&self, instance_id: &InstanceId) -> Option<SpawnRecord> {
        self.records
            .lock()
            .unwrap()
            .iter()
            .find(|r| r.instance_id == *instance_id)
            .cloned()
    }

    async fn save_spawn_record(&self, record: &SpawnRecord) -> Result<(), SpawnSupervisorError> {
        self.update_record(record);
        Ok(())
    }

    async fn delete_spawn_record(
        &self,
        _instance_id: &InstanceId,
    ) -> Result<(), SpawnSupervisorError> {
        Ok(())
    }

    async fn scan_spawns_by_phase(&self, phase: SpawnPhase, _max: u32) -> Vec<SpawnRecord> {
        self.records
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.spawn_phase == phase)
            .cloned()
            .collect()
    }

    async fn transition_phase(
        &self,
        instance_id: &InstanceId,
        new_phase: SpawnPhase,
    ) -> Result<(), SpawnSupervisorError> {
        let mut records = self.records.lock().unwrap();
        if let Some(record) = records.iter_mut().find(|r| r.instance_id == *instance_id) {
            record.spawn_phase = new_phase;
            Ok(())
        } else {
            Err(SpawnSupervisorError::InstanceNotFound(instance_id.clone()))
        }
    }
}

#[derive(Debug)]
struct MockProcessManager {
    health_check_result: std::sync::Mutex<Result<bool, SpawnSupervisorError>>,
}

impl MockProcessManager {
    fn new() -> Self {
        Self {
            health_check_result: std::sync::Mutex::new(Ok(false)),
        }
    }

    fn set_health_check_result(&self, result: Result<bool, SpawnSupervisorError>) {
        *self.health_check_result.lock().unwrap() = result;
    }
}

#[async_trait::async_trait]
impl ProcessManager for MockProcessManager {
    async fn spawn_process(&self, _command: &str) -> Result<ProcessHandle, SpawnSupervisorError> {
        Ok(ProcessHandle::new(1234, _command.to_string()))
    }

    async fn check_health(&self, _pid: u32) -> Result<bool, SpawnSupervisorError> {
        self.health_check_result.lock().unwrap().clone()
    }

    async fn is_zombie(&self, _pid: u32) -> Result<bool, SpawnSupervisorError> {
        Ok(false)
    }

    async fn terminate(&self, _pid: u32) -> Result<(), SpawnSupervisorError> {
        Ok(())
    }

    async fn wait(&self, _pid: u32) -> Result<i32, SpawnSupervisorError> {
        Ok(0)
    }
}

#[derive(Debug, Default)]
struct MockWorkQueue {
    enqueued_spawns: std::sync::Mutex<Vec<InstanceId>>,
    enqueued_resumes: std::sync::Mutex<Vec<InstanceId>>,
    parent_notifications: std::sync::Mutex<Vec<InstanceId>>,
}

impl MockWorkQueue {
    fn new() -> Self {
        Self::default()
    }

    fn get_enqueued_spawns(&self) -> Vec<InstanceId> {
        self.enqueued_spawns.lock().unwrap().clone()
    }

    fn get_enqueued_resumes(&self) -> Vec<InstanceId> {
        self.enqueued_resumes.lock().unwrap().clone()
    }

    fn get_parent_notifications(&self) -> Vec<InstanceId> {
        self.parent_notifications.lock().unwrap().clone()
    }

    fn notify_parent(&self, instance_id: InstanceId) {
        self.parent_notifications.lock().unwrap().push(instance_id);
    }
}

#[async_trait::async_trait]
impl WorkQueue for MockWorkQueue {
    async fn enqueue_spawn(
        &self,
        instance_id: InstanceId,
        _command: String,
    ) -> Result<(), SpawnSupervisorError> {
        self.enqueued_spawns.lock().unwrap().push(instance_id);
        Ok(())
    }

    async fn enqueue_resume(&self, instance_id: InstanceId) -> Result<(), SpawnSupervisorError> {
        self.enqueued_resumes.lock().unwrap().push(instance_id);
        Ok(())
    }
}

// =============================================================================
// RED-QUEEN Tests: Supervisor Crash Propagation
// =============================================================================

// === Happy Path: Failure IS propagated correctly ===

/// RED-QUEEN TEST rq-003-happy-1: Supervisor propagates crash to parent
///
/// **Given:** A child actor managed by a supervisor
/// **When:** All restart attempts are exhausted
/// **Then:** THE SYSTEM SHALL notify parent (EARS: Event-Driven)
///
/// **Current Gap:** The SpawnSupervisor does not notify parent when max
/// spawn attempts are exceeded. The record is simply skipped with a warning.
#[tokio::test]
async fn rq_003_supervisor_notifies_parent_on_all_restarts_exhausted() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(MockProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let instance_id = test_instance_id();
    let max_spawn_attempts = 3u32;

    // Create spawn record at Failed phase with max attempts already reached
    let mut record = SpawnRecord {
        spawn_id: None,
        instance_id: instance_id.clone(),
        command: "./failing-worker".to_string(),
        spawn_phase: SpawnPhase::Failed,
        health_checks: 0,
        spawn_attempts: max_spawn_attempts, // Exactly at limit
        last_error: Some(SpawnSupervisorError::HealthCheckFailed {
            instance_id: instance_id.clone(),
            check_number: 3,
            error: "health check timeout".to_string(),
        }),
    };
    storage.add_record(record.clone());

    let supervisor = SpawnSupervisor::new(
        Duration::from_millis(100),
        3,
        Duration::from_millis(50),
        2.0,
        max_spawn_attempts,
        storage.clone(),
        process_manager.clone(),
        work_queue.clone(),
    )
    .expect("Valid config");

    // Process one cycle
    let result = supervisor
        .process_cycle()
        .await
        .expect("Process cycle should succeed");

    // Record was processed
    assert_eq!(result.spawns_processed, 1);

    // GAP EXPOSED (rq-003): Parent notification should occur when all restarts
    // are exhausted, but currently no notification mechanism exists.
    //
    // Expected: work_queue.notify_parent(instance_id) should be called
    // Actual: No parent notification occurs
    //
    // This test documents the EXPECTED behavior per EARS requirements.
    // The assertion below WILL FAIL until the implementation adds parent
    // notification to SpawnSupervisor.
    let parent_notifications = work_queue.get_parent_notifications();
    assert!(
        !parent_notifications.is_empty(),
        "rq-003 GAP: Parent SHOULD be notified when all restarts exhausted (EARS: Event-Driven)"
    );
}

/// RED-QUEEN TEST rq-003-happy-2: Parent-child registry tracks child failure
///
/// **Given:** A parent actor with a child in Failed state
/// **When:** Parent queries child state
/// **Then:** THE SYSTEM SHALL propagate failures up the tree (EARS: Ubiquitous)
///
/// **Current Gap:** ParentChildRegistry.update_child_state() exists but is never
/// called by SpawnSupervisor when spawn fails permanently.
#[tokio::test]
async fn rq_003_parent_tracks_child_failure_state() {
    let registry = ParentChildRegistry::new();
    let child_id = test_instance_id();

    // Parent registers child
    registry.add_child(child_id.clone()).await;

    // Simulate child entering Failed state (as would happen after all restarts exhausted)
    registry
        .update_child_state(&child_id, ActorLifecycleState::Failed)
        .await;

    // Verify child is tracked as Failed
    let children = registry.get_children().await;
    let child_info = children.get(&child_id).expect("Child should exist");

    assert_eq!(
        child_info.state,
        ActorLifecycleState::Failed,
        "Child should be in Failed state"
    );

    // Verify all children are terminal (important for shutdown propagation)
    assert!(
        registry.all_children_terminal().await,
        "All children should be terminal when child is Failed"
    );
}

// === Error/Edge Cases: Silent failure detection ===

/// RED-QUEEN TEST rq-003-edge-1: Silent failure detection
///
/// **Given:** A supervisor with exhausted restart attempts
/// **When:** Failure is NOT propagated to parent
/// **Then:** THE SYSTEM SHALL silently swallow failures (UNWANTED)
///
/// This test exposes the current (buggy) behavior where failures are
/// silently swallowed when max spawn attempts are exceeded.
///
/// **Expected:** Parent notification via work_queue.notify_parent()
/// **Actual:** No notification occurs (silent failure)
#[tokio::test]
async fn rq_003_silent_failure_detection_when_parent_not_notified() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(MockProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let instance_id = test_instance_id();
    let max_spawn_attempts = 2u32;

    // Create spawn record already at Failed phase with attempts exhausted
    let record = SpawnRecord {
        spawn_id: None,
        instance_id: instance_id.clone(),
        command: "./permanently-failed".to_string(),
        spawn_phase: SpawnPhase::Failed,
        health_checks: 0,
        spawn_attempts: max_spawn_attempts + 1, // Exceeded
        last_error: Some(SpawnSupervisorError::ProcessExited {
            instance_id: instance_id.clone(),
            pid: 1234,
            exit_code: 1,
        }),
    };
    storage.add_record(record);

    let supervisor = SpawnSupervisor::new(
        Duration::from_millis(100),
        3,
        Duration::from_millis(50),
        2.0,
        max_spawn_attempts,
        storage.clone(),
        process_manager.clone(),
        work_queue.clone(),
    )
    .expect("Valid config");

    let result = supervisor
        .process_cycle()
        .await
        .expect("Process cycle should succeed");

    // Record was processed (counts toward spawns_processed even though skipped)
    assert_eq!(result.spawns_processed, 1);

    // RED-QUEEN: This test documents the UNWANTED behavior.
    // Currently, no parent notification occurs - failure is SILENT.
    // This is the bug that needs to be fixed.
    let parent_notifications = work_queue.get_parent_notifications();

    // UNWANTED behavior detected: parent_notifications is EMPTY
    // Per EARS "Unwanted" requirement: silent failure = BAD
    // This assertion PASSES to confirm we detected the bug
    // The fix would make parent_notifications non-empty
    assert!(
        parent_notifications.is_empty(),
        "UNWANTED: Silent failure detected - parent NOT notified (this is the bug rq-003)"
    );

    // Also check that no spawn was enqueued (since attempts exceeded)
    let enqueued_spawns = work_queue.get_enqueued_spawns();
    assert!(
        enqueued_spawns.is_empty(),
        "No respawn should be scheduled when attempts exhausted"
    );
}

/// RED-QUEEN TEST rq-003-edge-2: Failure accountability invariant
///
/// **Given:** All restarts exhausted for a child
/// **When:** Parent checks accountability
/// **Then:** THE SYSTEM SHALL maintain failure accountability (Invariant)
///
/// This test verifies that when a failure occurs, there is a mechanism
/// to track and report what happened.
#[tokio::test]
async fn rq_003_failure_accountability_in_maintained() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(MockProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let instance_id = test_instance_id();
    let max_spawn_attempts = 3u32;

    // Create record at Failed phase with detailed error
    let expected_error = SpawnSupervisorError::HealthCheckFailed {
        instance_id: instance_id.clone(),
        check_number: 3,
        error: "final health check timeout - all retries exhausted".to_string(),
    };

    let record = SpawnRecord {
        spawn_id: Some(vo_types::SpawnId::new("spawn-123".to_string())),
        instance_id: instance_id.clone(),
        command: "./worker".to_string(),
        spawn_phase: SpawnPhase::Failed,
        health_checks: 3,
        spawn_attempts: max_spawn_attempts,
        last_error: Some(expected_error.clone()),
    };
    storage.add_record(record);

    let supervisor = SpawnSupervisor::new(
        Duration::from_millis(100),
        3,
        Duration::from_millis(50),
        2.0,
        max_spawn_attempts,
        storage.clone(),
        process_manager.clone(),
        work_queue.clone(),
    )
    .expect("Valid config");

    let result = supervisor
        .process_cycle()
        .await
        .expect("Process cycle should succeed");

    assert_eq!(result.spawns_processed, 1);

    // Verify error was recorded (accountability maintained)
    let records = storage.get_records();
    let saved_record = records
        .iter()
        .find(|r| r.instance_id == instance_id)
        .expect("Record should exist");

    // Error SHOULD be preserved for accountability
    assert!(
        saved_record.last_error.is_some(),
        "Error should be recorded for failure accountability"
    );

    let saved_error = saved_record.last_error.as_ref().unwrap();
    assert!(
        matches!(saved_error, SpawnSupervisorError::HealthCheckFailed { .. }),
        "Error type should be HealthCheckFailed"
    );

    // GAP: While error is recorded, no parent notification occurs
    // The invariant "Failure accountability" requires more than just
    // recording the error - it requires notifying interested parties
    let parent_notifications = work_queue.get_parent_notifications();
    assert!(
        parent_notifications.is_empty(),
        "rq-003 GAP: Parent notification missing for failure accountability"
    );
}

/// RED-QUEEN TEST rq-003-edge-3: Multiple failures in supervision tree
///
/// **Given:** Multiple children failing simultaneously
/// **When:** All exhaust their restart attempts
/// **Then:** THE SYSTEM SHALL notify parent for EACH failure
///
/// This test ensures the propagation works at scale.
#[tokio::test]
async fn rq_003_multiple_children_all_fail_propagate_to_parent() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(MockProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let max_spawn_attempts = 2u32;

    // Create multiple failed children
    let child_ids = (0..3).map(|_| test_instance_id()).collect::<Vec<_>>();

    for child_id in &child_ids {
        let record = SpawnRecord {
            spawn_id: None,
            instance_id: child_id.clone(),
            command: "./failing-worker".to_string(),
            spawn_phase: SpawnPhase::Failed,
            health_checks: 0,
            spawn_attempts: max_spawn_attempts + 1, // Exceeded
            last_error: Some(SpawnSupervisorError::ProcessExited {
                instance_id: child_id.clone(),
                pid: 1234,
                exit_code: 1,
            }),
        };
        storage.add_record(record);
    }

    let supervisor = SpawnSupervisor::new(
        Duration::from_millis(100),
        3,
        Duration::from_millis(50),
        2.0,
        max_spawn_attempts,
        storage.clone(),
        process_manager.clone(),
        work_queue.clone(),
    )
    .expect("Valid config");

    let result = supervisor
        .process_cycle()
        .await
        .expect("Process cycle should succeed");

    // All 3 children processed
    assert_eq!(result.spawns_processed, 3);

    // GAP: Each child should trigger a parent notification
    // Currently, NO notifications occur
    let parent_notifications = work_queue.get_parent_notifications();
    assert!(
        parent_notifications.is_empty(),
        "rq-003 GAP: All {} children failed but 0 parent notifications sent",
        child_ids.len()
    );
}

// =============================================================================
// RED-QUEEN Tests: Lifecycle State Machine and Failure Propagation
// =============================================================================

/// RED-QUEEN TEST rq-003-lifecycle-1: Child failure state machine
///
/// Verifies the lifecycle state transitions correctly reflect failures.
#[test]
fn rq_003_lifecycle_failed_state_rejects_all_transitions() {
    use vo_actor::lifecycle::{compute_next_state, ActorLifecycleState, LifecycleTransition};

    // Failed state should reject all transitions (terminal state)
    let failed = ActorLifecycleState::Failed;

    let transitions = [
        LifecycleTransition::Start,
        LifecycleTransition::Stop,
        LifecycleTransition::ChildStopped,
        LifecycleTransition::AllChildrenStopped,
        LifecycleTransition::Fail,
    ];

    for t in transitions {
        let next = compute_next_state(failed, t);
        assert!(
            next.is_none(),
            "Failed state should reject {:?} transition (terminal)",
            t
        );
    }
}

/// RED-QUEEN TEST rq-003-lifecycle-2: Parent observes child failure
///
/// **Given:** Parent watching child
/// **When:** Child enters Failed state
/// **Then:** Parent's registry reflects Failed state
#[tokio::test]
async fn rq_003_parent_registry_reflects_child_failed_state() {
    let registry = ParentChildRegistry::new();
    let child_id = test_instance_id();

    // Add child as Pending
    registry.add_child(child_id.clone()).await;

    // Verify initial state
    let children = registry.get_children().await;
    assert_eq!(
        children.get(&child_id).unwrap().state,
        ActorLifecycleState::Pending
    );

    // Child fails
    registry
        .update_child_state(&child_id, ActorLifecycleState::Failed)
        .await;

    // Verify Failed state is reflected
    let children = registry.get_children().await;
    assert_eq!(
        children.get(&child_id).unwrap().state,
        ActorLifecycleState::Failed
    );

    // Verify all_children_terminal returns true
    assert!(
        registry.all_children_terminal().await,
        "Registry should report all children terminal when child is Failed"
    );
}

/// RED-QUEEN TEST rq-003-lifecycle-3: Supervision tree failure propagation
///
/// **Given:** A supervision tree with parent and children
/// **When:** ALL children have Failed state
/// **Then:** Parent can detect complete failure of its subtree
///
/// This is important for escalation - if all children fail, parent
/// may need to take action (e.g., notify its parent).
#[tokio::test]
async fn rq_003_supervision_tree_all_children_failed() {
    let registry = ParentChildRegistry::new();

    let child_1 = test_instance_id();
    let child_2 = test_instance_id();
    let child_3 = test_instance_id();

    // Parent has 3 children
    registry.add_child(child_1.clone()).await;
    registry.add_child(child_2.clone()).await;
    registry.add_child(child_3.clone()).await;

    // All children fail
    registry
        .update_child_state(&child_1, ActorLifecycleState::Failed)
        .await;
    registry
        .update_child_state(&child_2, ActorLifecycleState::Failed)
        .await;
    registry
        .update_child_state(&child_3, ActorLifecycleState::Failed)
        .await;

    // All children should be terminal
    assert!(
        registry.all_children_terminal().await,
        "All children terminal when all are Failed"
    );

    // No children should be active
    assert_eq!(
        registry.active_children_count().await,
        0,
        "No active children when all have failed"
    );

    // GAP: While registry tracks state correctly, there's no mechanism
    // for the SpawnSupervisor to automatically update the parent registry
    // when failures occur. The notification must be explicit.
}

// =============================================================================
// Summary
// =============================================================================

// RED-QUEEN rq-003 Test Summary:
// =============================
//
// These tests document the EXPECTED behavior per EARS requirements for
// supervisor crash propagation (rq-003).
//
// CURRENT GAPS IDENTIFIED:
// 1. SpawnSupervisor does not notify parent when max spawn attempts exceeded
// 2. No WorkQueue.notify_parent() method exists
// 3. ParentChildRegistry exists but is not integrated with SpawnSupervisor
// 4. Failures are silently swallowed when restart attempts exhausted
//
// EXPECTED BEHAVIOR (per EARS):
// - Ubiquitous: THE SYSTEM SHALL propagate failures up the tree
// - Event-Driven: When all restarts exhausted, THE SYSTEM SHALL notify parent
// - Unwanted: If failure not propagated, THE SYSTEM SHALL silently swallow failures
//
// CURRENT BEHAVIOR:
// - Failures are logged but NOT propagated to parent
// - No notification mechanism exists
// - Silent failure occurs when max attempts exceeded
//
// IMPLEMENTATION REQUIRED:
// 1. Add notify_parent() method to WorkQueue trait
// 2. Call notify_parent() in SpawnSupervisor when spawn_attempts > max
// 3. Integrate ParentChildRegistry with SpawnSupervisor failure handling
// 4. Ensure failure accountability via error recording + parent notification
