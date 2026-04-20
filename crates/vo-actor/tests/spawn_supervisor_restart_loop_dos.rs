//! BLACKHAT (BH-005): Restart Loop DoS — SpawnSupervisor
//!
//! Adversarial tests verifying that rapid actor crashes cannot cause resource
//! exhaustion via restart loops. Tests restart limit enforcement, backoff
//! containment, and resource cleanup.
//!
//! Attack Vectors:
//!   BH-RL01: Restart loop contained after max_spawn_attempts
//!   BH-RL02: Resource count bounded — records don't accumulate unbounded
//!   BH-RL03: Exponential backoff prevents rapid restart storms
//!   BH-RL04: Concurrent crash loops are independently contained
//!   BH-RL05: should_respawn boundary — no off-by-one escape
//!   BH-RL06: is_zombie_state detection for high-attempt records
//!   BH-RL07: Respawn saturating_add cannot overflow to bypass limit
//!   BH-RL08: Storage failure during restart loop doesn't cause unbounded work
//!   BH-RL09: Metrics stay bounded during sustained crash loop
//!   BH-RL10: Failed records at limit never re-enter spawn phase

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use vo_actor::spawn_supervisor::{
    calculate_backoff_delay, is_zombie_state, should_respawn, ProcessHandle,
    ProcessManager, SpawnPhase, SpawnRecord, SpawnStorage, SpawnSupervisor, SpawnSupervisorError,
    WorkQueue,
};
use vo_types::InstanceId;

fn test_instance_id() -> InstanceId {
    use ulid::Ulid;
    let ulid = Ulid::new();
    InstanceId::from_bytes(ulid.to_bytes())
}

// =============================================================================
// Mock Infrastructure (reused from spawn_supervisor_integration.rs)
// =============================================================================

#[derive(Debug, Default)]
struct MockSpawnStorage {
    records: std::sync::Mutex<Vec<SpawnRecord>>,
    should_fail: std::sync::Mutex<bool>,
    save_error: std::sync::Mutex<Option<SpawnSupervisorError>>,
    save_count: std::sync::atomic::AtomicU64,
    scan_count: std::sync::atomic::AtomicU64,
}

impl MockSpawnStorage {
    fn new() -> Self {
        Self::default()
    }

    fn set_save_error(&self, err: SpawnSupervisorError) {
        *self.save_error.lock().unwrap() = Some(err);
    }

    fn add_record(&self, record: SpawnRecord) {
        self.records.lock().unwrap().push(record);
    }

    fn get_records(&self) -> Vec<SpawnRecord> {
        self.records.lock().unwrap().clone()
    }

    fn record_count(&self) -> usize {
        self.records.lock().unwrap().len()
    }

    fn set_should_fail(&self, should_fail: bool) {
        *self.should_fail.lock().unwrap() = should_fail;
    }

    fn save_call_count(&self) -> u64 {
        self.save_count.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn scan_call_count(&self) -> u64 {
        self.scan_count.load(std::sync::atomic::Ordering::SeqCst)
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
        self.save_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if *self.should_fail.lock().unwrap() {
            return Err(SpawnSupervisorError::StorageError(
                "Mock storage failure".to_string(),
            ));
        }
        if let Some(err) = self.save_error.lock().unwrap().take() {
            return Err(err);
        }
        let mut records = self.records.lock().unwrap();
        if let Some(pos) = records
            .iter()
            .position(|r| r.instance_id == record.instance_id)
        {
            records[pos] = record.clone();
        } else {
            records.push(record.clone());
        }
        Ok(())
    }

    async fn delete_spawn_record(
        &self,
        instance_id: &InstanceId,
    ) -> Result<(), SpawnSupervisorError> {
        let mut records = self.records.lock().unwrap();
        records.retain(|r| r.instance_id != *instance_id);
        Ok(())
    }

    async fn scan_spawns_by_phase(&self, phase: SpawnPhase, _max: u32) -> Vec<SpawnRecord> {
        self.scan_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
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

/// ProcessManager that always fails spawn — simulates a permanently broken process.
#[derive(Debug)]
struct AlwaysFailProcessManager {
    pid_counter: std::sync::atomic::AtomicU32,
}

impl AlwaysFailProcessManager {
    fn new() -> Self {
        Self {
            pid_counter: std::sync::atomic::AtomicU32::new(1000),
        }
    }
}

#[async_trait::async_trait]
impl ProcessManager for AlwaysFailProcessManager {
    async fn spawn_process(&self, command: &str) -> Result<ProcessHandle, SpawnSupervisorError> {
        let pid = self.pid_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Err(SpawnSupervisorError::SpawnFailed {
            command: command.to_string(),
            error: format!("Simulated crash (pid {pid})"),
        })
    }

    async fn check_health(&self, _pid: u32) -> Result<bool, SpawnSupervisorError> {
        Ok(false)
    }

    async fn is_zombie(&self, _pid: u32) -> Result<bool, SpawnSupervisorError> {
        Ok(false)
    }

    async fn terminate(&self, _pid: u32) -> Result<(), SpawnSupervisorError> {
        Ok(())
    }

    async fn wait(&self, _pid: u32) -> Result<i32, SpawnSupervisorError> {
        Ok(1)
    }
}

/// ProcessManager that succeeds on spawn but always fails health check.
#[derive(Debug)]
struct HealthCheckFailProcessManager {
    pid_counter: std::sync::atomic::AtomicU32,
}

impl HealthCheckFailProcessManager {
    fn new() -> Self {
        Self {
            pid_counter: std::sync::atomic::AtomicU32::new(2000),
        }
    }
}

#[async_trait::async_trait]
impl ProcessManager for HealthCheckFailProcessManager {
    async fn spawn_process(&self, command: &str) -> Result<ProcessHandle, SpawnSupervisorError> {
        let pid = self.pid_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(ProcessHandle::new(pid, command.to_string()))
    }

    async fn check_health(&self, _pid: u32) -> Result<bool, SpawnSupervisorError> {
        Ok(false)
    }

    async fn is_zombie(&self, _pid: u32) -> Result<bool, SpawnSupervisorError> {
        Ok(false)
    }

    async fn terminate(&self, _pid: u32) -> Result<(), SpawnSupervisorError> {
        Ok(())
    }

    async fn wait(&self, _pid: u32) -> Result<i32, SpawnSupervisorError> {
        Ok(1)
    }
}

#[derive(Debug, Default)]
struct MockWorkQueue {
    enqueued_spawns: std::sync::Mutex<Vec<InstanceId>>,
    enqueued_resumes: std::sync::Mutex<Vec<InstanceId>>,
    should_fail: std::sync::Mutex<bool>,
    enqueue_count: std::sync::atomic::AtomicU64,
}

impl MockWorkQueue {
    fn new() -> Self {
        Self::default()
    }

    fn get_enqueued_spawns(&self) -> Vec<InstanceId> {
        self.enqueued_spawns.lock().unwrap().clone()
    }

    fn enqueue_spawn_count(&self) -> u64 {
        self.enqueue_count.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn set_should_fail(&self, should_fail: bool) {
        *self.should_fail.lock().unwrap() = should_fail;
    }
}

#[async_trait::async_trait]
impl WorkQueue for MockWorkQueue {
    async fn enqueue_spawn(
        &self,
        instance_id: InstanceId,
        _command: String,
    ) -> Result<(), SpawnSupervisorError> {
        self.enqueue_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if *self.should_fail.lock().unwrap() {
            return Err(SpawnSupervisorError::DispatchError(
                "Queue full".to_string(),
            ));
        }
        self.enqueued_spawns.lock().unwrap().push(instance_id);
        Ok(())
    }

    async fn enqueue_resume(&self, instance_id: InstanceId) -> Result<(), SpawnSupervisorError> {
        if *self.should_fail.lock().unwrap() {
            return Err(SpawnSupervisorError::DispatchError(
                "Queue full".to_string(),
            ));
        }
        self.enqueued_resumes.lock().unwrap().push(instance_id);
        Ok(())
    }
}

// =============================================================================
// BH-RL01: Restart loop contained after max_spawn_attempts
// =============================================================================

/// Simulates a process that always crashes on spawn. After N cycles, the
/// supervisor must stop trying to respawn it. The total number of respawns
/// must be strictly bounded by max_spawn_attempts.
#[tokio::test]
async fn bh_rl01_restart_loop_stops_at_max_attempts() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(AlwaysFailProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let max_attempts = 3u32;
    let instance_id = test_instance_id();

    // Start with a Failed record at attempt 0 — will be respawned each cycle
    let record = SpawnRecord {
        spawn_id: None,
        instance_id: instance_id.clone(),
        command: "./crasher".to_string(),
        spawn_phase: SpawnPhase::Failed,
        health_checks: 0,
        spawn_attempts: 0,
        last_error: None,
    };
    storage.add_record(record);

    let supervisor = SpawnSupervisor::new(
        Duration::from_millis(1),
        1,
        Duration::from_millis(1),
        1.0,
        max_attempts,
        storage.clone(),
        process_manager,
        work_queue.clone(),
    )
    .expect("Valid config");

    // Run multiple cycles — more than max_attempts to ensure it stops
    for _ in 0..10 {
        let _ = supervisor.process_cycle().await;
    }

    // The record must not have been respawned more than max_attempts times.
    // Each respawn increments spawn_attempts by 1 via saturating_add.
    // After max_attempts, should_respawn returns false.
    let final_record = storage
        .get_spawn_record(&instance_id)
        .await
        .expect("Record should exist");

    // INVARIANT: spawn_attempts must be bounded by max_attempts
    assert!(
        final_record.spawn_attempts <= max_attempts,
        "spawn_attempts ({}) must not exceed max_spawn_attempts ({}) — restart loop not contained!",
        final_record.spawn_attempts,
        max_attempts,
    );

    // INVARIANT: no more work queued after limit reached
    let enqueued = work_queue.enqueue_spawn_count();
    assert!(
        enqueued <= max_attempts as u64,
        "enqueue_spawn called {} times but max_attempts is {} — work queue not bounded!",
        enqueued,
        max_attempts,
    );
}

// =============================================================================
// BH-RL02: Resource count bounded — one record per instance, never duplicated
// =============================================================================

/// Verifies that crash looping never creates duplicate records in storage.
/// Even after many cycles, there must be exactly one record per instance.
#[tokio::test]
async fn bh_rl02_storage_record_count_stays_bounded() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(AlwaysFailProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let instance_id = test_instance_id();
    let record = SpawnRecord {
        spawn_id: None,
        instance_id: instance_id.clone(),
        command: "./crasher".to_string(),
        spawn_phase: SpawnPhase::Failed,
        health_checks: 0,
        spawn_attempts: 0,
        last_error: None,
    };
    storage.add_record(record);

    let supervisor = SpawnSupervisor::new(
        Duration::from_millis(1),
        1,
        Duration::from_millis(1),
        1.0,
        5,
        storage.clone(),
        process_manager,
        work_queue,
    )
    .expect("Valid config");

    // Run many cycles
    for _ in 0..20 {
        let _ = supervisor.process_cycle().await;
    }

    // INVARIANT: exactly one record per instance — no duplicates
    let record_count = storage.record_count();
    assert_eq!(
        record_count, 1,
        "Expected exactly 1 record per instance, found {} — records are accumulating (memory leak)!",
        record_count,
    );
}

// =============================================================================
// BH-RL03: Exponential backoff prevents rapid restart storms
// =============================================================================

/// Verifies that the backoff delay grows monotonically with each attempt.
/// An attacker cannot force rapid restarts by manipulating spawn_attempts.
#[test]
fn bh_rl03_backoff_grows_monotonically_preventing_rapid_restarts() {
    let initial_ms = 100u64;
    let multiplier = 2.0;

    let mut prev_delay = 0u64;
    for attempt in 1..=20 {
        let delay = calculate_backoff_delay(initial_ms, multiplier, attempt);
        assert!(
            delay >= prev_delay,
            "Backoff must be monotonically increasing: attempt {} delay {}ms < prev {}ms",
            attempt,
            delay,
            prev_delay,
        );
        prev_delay = delay;
    }

    // Verify the delays grow exponentially, not linearly
    let delay_1 = calculate_backoff_delay(initial_ms, multiplier, 1);
    let delay_5 = calculate_backoff_delay(initial_ms, multiplier, 5);
    assert!(
        delay_5 > delay_1 * 4,
        "Exponential backoff at attempt 5 ({}) should be > 4x attempt 1 ({})",
        delay_5,
        delay_1 * 4,
    );
}

/// Even with multiplier=1.0 (no growth), the initial backoff still applies.
#[test]
fn bh_rl03_backoff_with_multiplier_1_still_delays() {
    let initial_ms = 500u64;
    for attempt in 1..=10 {
        let delay = calculate_backoff_delay(initial_ms, 1.0, attempt);
        assert_eq!(
            delay, initial_ms,
            "With multiplier 1.0, backoff should always equal initial ({})ms, got {}ms at attempt {}",
            initial_ms, delay, attempt,
        );
    }
}

// =============================================================================
// BH-RL04: Concurrent crash loops are independently contained
// =============================================================================

/// Simulates multiple instances crashing simultaneously. Each must be
/// independently bounded — one instance hitting its limit must not prevent
/// others from being contained or allow unbounded growth.
#[tokio::test]
async fn bh_rl04_concurrent_crash_loops_independently_contained() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(AlwaysFailProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let max_attempts = 3u32;

    // Create 5 instances all in Failed state
    let instance_ids: Vec<InstanceId> = (0..5).map(|_| test_instance_id()).collect();
    for id in &instance_ids {
        let record = SpawnRecord {
            spawn_id: None,
            instance_id: id.clone(),
            command: "./crasher".to_string(),
            spawn_phase: SpawnPhase::Failed,
            health_checks: 0,
            spawn_attempts: 0,
            last_error: None,
        };
        storage.add_record(record);
    }

    let supervisor = SpawnSupervisor::new(
        Duration::from_millis(1),
        1,
        Duration::from_millis(1),
        1.0,
        max_attempts,
        storage.clone(),
        process_manager,
        work_queue.clone(),
    )
    .expect("Valid config");

    // Run enough cycles to exhaust all limits
    for _ in 0..20 {
        let _ = supervisor.process_cycle().await;
    }

    // INVARIANT: each instance independently bounded
    for id in &instance_ids {
        let record = storage
            .get_spawn_record(id)
            .await
            .expect("Record must exist");
        assert!(
            record.spawn_attempts <= max_attempts,
            "Instance {} spawn_attempts ({}) exceeds max ({}) — concurrent loop not contained!",
            id, record.spawn_attempts, max_attempts,
        );
    }

    // INVARIANT: total storage records == number of instances (no duplicates)
    assert_eq!(
        storage.record_count(),
        instance_ids.len(),
        "Storage should have exactly {} records, found {} — concurrent loops creating duplicates!",
        instance_ids.len(),
        storage.record_count(),
    );

    // INVARIANT: total enqueues bounded by instances * max_attempts
    let total_enqueues = work_queue.enqueue_spawn_count();
    assert!(
        total_enqueues <= instance_ids.len() as u64 * max_attempts as u64,
        "Total enqueues ({}) exceeds instances ({}) * max_attempts ({}) — not bounded!",
        total_enqueues,
        instance_ids.len(),
        max_attempts,
    );
}

// =============================================================================
// BH-RL05: should_respawn boundary — no off-by-one escape
// =============================================================================

/// Verifies should_respawn returns false at exactly max_attempts.
/// An off-by-one would allow one extra restart, leaking resources.
#[test]
fn bh_rl05_should_respawn_rejects_at_exact_limit() {
    let instance_id = test_instance_id();
    let max_attempts = 5u32;

    // At the limit exactly — must NOT respawn
    let at_limit = SpawnRecord {
        spawn_id: None,
        instance_id: instance_id.clone(),
        command: "./test".to_string(),
        spawn_phase: SpawnPhase::Failed,
        health_checks: 0,
        spawn_attempts: max_attempts,
        last_error: None,
    };
    assert!(
        !should_respawn(&at_limit, max_attempts),
        "should_respawn(attempts={}, max={}) must return false — off-by-one allows extra restart!",
        max_attempts, max_attempts,
    );

    // One below the limit — must respawn
    let below_limit = SpawnRecord {
        spawn_attempts: max_attempts - 1,
        ..at_limit.clone()
    };
    assert!(
        should_respawn(&below_limit, max_attempts),
        "should_respawn(attempts={}, max={}) must return true — one below limit should be retried!",
        max_attempts - 1, max_attempts,
    );

    // One above the limit — must NOT respawn
    let above_limit = SpawnRecord {
        spawn_attempts: max_attempts + 1,
        ..at_limit.clone()
    };
    assert!(
        !should_respawn(&above_limit, max_attempts),
        "should_respawn(attempts={}, max={}) must return false — above limit must never restart!",
        max_attempts + 1, max_attempts,
    );
}

/// Verifies should_respawn with max_attempts=1 — single attempt, no respawns.
#[test]
fn bh_rl05_should_respawn_with_max_one_no_respawn() {
    let instance_id = test_instance_id();
    let record = SpawnRecord {
        spawn_id: None,
        instance_id,
        command: "./test".to_string(),
        spawn_phase: SpawnPhase::Failed,
        health_checks: 0,
        spawn_attempts: 1,
        last_error: None,
    };

    // With max_attempts=1, a record at attempt 1 must not respawn
    assert!(
        !should_respawn(&record, 1),
        "should_respawn(attempts=1, max=1) must be false — single-attempt policy violated!",
    );
}

/// Verifies should_respawn rejects records not in Failed phase.
#[test]
fn bh_rl05_should_respawn_rejects_non_failed_phase() {
    let instance_id = test_instance_id();
    let max_attempts = 5u32;

    for phase in [
        SpawnPhase::Spawn,
        SpawnPhase::HealthCheck,
        SpawnPhase::Running,
        SpawnPhase::Shutdown,
        SpawnPhase::Terminated,
    ] {
        let record = SpawnRecord {
            spawn_id: None,
            instance_id: instance_id.clone(),
            command: "./test".to_string(),
            spawn_phase: phase,
            health_checks: 0,
            spawn_attempts: 0,
            last_error: None,
        };
        assert!(
            !should_respawn(&record, max_attempts),
            "should_respawn must reject {:?} phase — only Failed should be respawned!",
            phase,
        );
    }
}

// =============================================================================
// BH-RL06: is_zombie_state detection for high-attempt records
// =============================================================================

/// Zombie detection must fire for records with >3 attempts in Failed phase.
#[test]
fn bh_rl06_zombie_state_detected_at_high_attempts() {
    let instance_id = test_instance_id();

    // 4 attempts — zombie
    let zombie = SpawnRecord {
        spawn_id: None,
        instance_id: instance_id.clone(),
        command: "./test".to_string(),
        spawn_phase: SpawnPhase::Failed,
        health_checks: 0,
        spawn_attempts: 4,
        last_error: None,
    };
    assert!(
        is_zombie_state(&zombie),
        "is_zombie_state must detect Failed + attempts=4 as zombie",
    );

    // 100 attempts — definitely zombie
    let mega_zombie = SpawnRecord {
        spawn_attempts: 100,
        ..zombie.clone()
    };
    assert!(
        is_zombie_state(&mega_zombie),
        "is_zombie_state must detect Failed + attempts=100 as zombie",
    );

    // 3 attempts — NOT zombie (boundary: must be > 3, not >= 3)
    let borderline = SpawnRecord {
        spawn_attempts: 3,
        ..zombie.clone()
    };
    assert!(
        !is_zombie_state(&borderline),
        "is_zombie_state(attempts=3) must NOT be zombie — threshold is > 3",
    );

    // Failed but 1 attempt — not zombie
    let low_attempts = SpawnRecord {
        spawn_attempts: 1,
        ..zombie.clone()
    };
    assert!(
        !is_zombie_state(&low_attempts),
        "is_zombie_state(attempts=1) must NOT be zombie",
    );
}

/// Non-failed phases are never zombies, even with high attempts.
#[test]
fn bh_rl06_non_failed_phase_never_zombie() {
    let instance_id = test_instance_id();

    for phase in [
        SpawnPhase::Spawn,
        SpawnPhase::HealthCheck,
        SpawnPhase::Running,
        SpawnPhase::Shutdown,
        SpawnPhase::Terminated,
    ] {
        let record = SpawnRecord {
            spawn_id: None,
            instance_id: instance_id.clone(),
            command: "./test".to_string(),
            spawn_phase: phase,
            health_checks: 0,
            spawn_attempts: 100,
            last_error: None,
        };
        assert!(
            !is_zombie_state(&record),
            "is_zombie_state must return false for {:?} phase even with 100 attempts!",
            phase,
        );
    }
}

// =============================================================================
// BH-RL07: Respawn saturating_add cannot overflow to bypass limit
// =============================================================================

/// Tests that spawn_attempts saturating at u32::MAX doesn't bypass
/// the max_spawn_attempts check via overflow.
#[test]
fn bh_rl07_respawn_at_u32_max_saturates_safely() {
    let instance_id = test_instance_id();

    let at_max = SpawnRecord {
        spawn_id: None,
        instance_id: instance_id.clone(),
        command: "./test".to_string(),
        spawn_phase: SpawnPhase::Failed,
        health_checks: 0,
        spawn_attempts: u32::MAX,
        last_error: None,
    };

    // Respawn uses saturating_add(1), so it stays at u32::MAX
    let respawned = at_max.respawn(None);
    assert_eq!(
        respawned.spawn_attempts,
        u32::MAX,
        "saturating_add must not overflow past u32::MAX",
    );

    // u32::MAX >= any reasonable max_attempts — must NOT respawn
    assert!(
        !should_respawn(&respawned, 5),
        "should_respawn at u32::MAX attempts must return false — no overflow escape!",
    );

    // is_zombie_state must correctly detect this as zombie
    assert!(
        is_zombie_state(&at_max),
        "u32::MAX attempts in Failed phase must be detected as zombie",
    );
}

/// Simulates the full cycle: record at u32::MAX attempts in Spawn phase
/// must be skipped by process_cycle.
#[tokio::test]
async fn bh_rl07_u32_max_attempts_in_spawn_phase_skipped() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(AlwaysFailProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let instance_id = test_instance_id();
    let record = SpawnRecord {
        spawn_id: None,
        instance_id: instance_id.clone(),
        command: "./crasher".to_string(),
        spawn_phase: SpawnPhase::Spawn,
        health_checks: 0,
        spawn_attempts: u32::MAX,
        last_error: None,
    };
    storage.add_record(record);

    let supervisor = SpawnSupervisor::new(
        Duration::from_millis(1),
        1,
        Duration::from_millis(1),
        1.0,
        5,
        storage.clone(),
        process_manager,
        work_queue.clone(),
    )
    .expect("Valid config");

    let result = supervisor
        .process_cycle()
        .await
        .expect("Cycle should succeed");

    // Record must be skipped (spawn_attempts > max_spawn_attempts)
    assert!(
        result.errors >= 1,
        "Record at u32::MAX attempts must be counted as error",
    );

    // No work enqueued for this record
    assert_eq!(
        work_queue.enqueue_spawn_count(),
        0,
        "No spawn work should be enqueued for u32::MAX attempts",
    );

    // Record stays in Spawn phase (not transitioned to anything)
    let saved = storage.get_spawn_record(&instance_id).await.expect("must exist");
    assert_eq!(
        saved.spawn_phase,
        SpawnPhase::Spawn,
        "Record at u32::MAX attempts should remain in Spawn phase (skipped)",
    );
}

// =============================================================================
// BH-RL08: Storage failure during restart loop doesn't cause unbounded work
// =============================================================================

/// When storage.save_spawn_record fails during respawn, the supervisor
/// must not enqueue duplicate work or loop infinitely. The respawn path
/// saves the respawned (Spawn phase) record first, then enqueues — if save
/// fails, enqueue is skipped.
#[tokio::test]
async fn bh_rl08_storage_failure_during_respawn_no_unbounded_work() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(AlwaysFailProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let instance_id = test_instance_id();
    let record = SpawnRecord {
        spawn_id: None,
        instance_id: instance_id.clone(),
        command: "./crasher".to_string(),
        spawn_phase: SpawnPhase::Failed,
        health_checks: 0,
        spawn_attempts: 1,
        last_error: None,
    };
    storage.add_record(record);

    // Make all saves fail permanently — simulates total storage outage
    storage.set_should_fail(true);

    let supervisor = SpawnSupervisor::new(
        Duration::from_millis(1),
        1,
        Duration::from_millis(1),
        1.0,
        5,
        storage.clone(),
        process_manager,
        work_queue.clone(),
    )
    .expect("Valid config");

    // Run many cycles with storage broken
    for _ in 0..10 {
        let _ = supervisor.process_cycle().await;
    }

    // The record stays in Failed because save always fails —
    // the respawn record is never persisted
    let saved = storage.get_spawn_record(&instance_id).await.expect("must exist");
    assert_eq!(
        saved.spawn_phase,
        SpawnPhase::Failed,
        "Record should remain in Failed when storage save always fails during respawn",
    );

    // No work enqueued because save failed (the code continues to next record)
    assert_eq!(
        work_queue.enqueue_spawn_count(),
        0,
        "No work should be enqueued when respawn save fails — prevents phantom work!",
    );

    // Storage should not have accumulated duplicate records
    assert_eq!(
        storage.record_count(),
        1,
        "No duplicate records from failed saves",
    );

    // spawn_attempts stays at original value since respawn never persisted
    assert_eq!(
        saved.spawn_attempts, 1,
        "spawn_attempts should not change when respawn save fails",
    );
}

// =============================================================================
// BH-RL09: Metrics stay bounded during sustained crash loop
// =============================================================================

/// Verifies that after max_attempts is reached, further cycles do not
/// increment respawn or enqueue metrics. The system stabilizes.
#[tokio::test]
async fn bh_rl09_metrics_stabilize_after_max_attempts_reached() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(HealthCheckFailProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let max_attempts = 3u32;
    let instance_id = test_instance_id();

    // Start in Failed phase — exercises the full crash-loop path:
    // Failed → respawn → Spawn → (spawn succeeds) → HealthCheck → (fails) → Failed → ...
    let record = SpawnRecord {
        spawn_id: None,
        instance_id: instance_id.clone(),
        command: "./crasher".to_string(),
        spawn_phase: SpawnPhase::Failed,
        health_checks: 0,
        spawn_attempts: 0,
        last_error: None,
    };
    storage.add_record(record);

    let supervisor = SpawnSupervisor::new(
        Duration::from_millis(1),
        1,
        Duration::from_millis(1),
        1.0,
        max_attempts,
        storage.clone(),
        process_manager,
        work_queue.clone(),
    )
    .expect("Valid config");

    // Run enough cycles to exhaust all attempts through the full lifecycle
    for _ in 0..10 {
        let _ = supervisor.process_cycle().await;
    }

    // After exhaustion, record is Failed at max_attempts.
    // Snapshot the metrics that must stabilize.
    let metrics_after_exhaustion = (
        supervisor.metrics.spawns_failed.get(),
        supervisor.metrics.respawns.get(),
        supervisor.metrics.dispatch_errors.get(),
        work_queue.enqueue_spawn_count(),
    );

    // Run 10 more cycles — these metrics must NOT change
    for _ in 0..10 {
        let _ = supervisor.process_cycle().await;
    }

    let metrics_after_extra = (
        supervisor.metrics.spawns_failed.get(),
        supervisor.metrics.respawns.get(),
        supervisor.metrics.dispatch_errors.get(),
        work_queue.enqueue_spawn_count(),
    );

    assert_eq!(
        metrics_after_exhaustion, metrics_after_extra,
        "Metrics must stabilize after max_attempts reached — further cycles must not accumulate metrics! \
         Before: {:?}, After: {:?}",
        metrics_after_exhaustion, metrics_after_extra,
    );
}

/// Verify spawns_failed metric is incremented when max_spawn_attempts exceeded
/// during the Spawn-phase scan (line 612 of spawn_supervisor.rs).
#[tokio::test]
async fn bh_rl09_spawns_failed_increments_on_attempt_exceeded() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(AlwaysFailProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let instance_id = test_instance_id();
    let record = SpawnRecord {
        spawn_id: None,
        instance_id,
        command: "./crasher".to_string(),
        spawn_phase: SpawnPhase::Spawn,
        health_checks: 0,
        spawn_attempts: 100,
        last_error: None,
    };
    storage.add_record(record);

    let supervisor = SpawnSupervisor::new(
        Duration::from_millis(1),
        1,
        Duration::from_millis(1),
        1.0,
        5,
        storage.clone(),
        process_manager,
        work_queue,
    )
    .expect("Valid config");

    supervisor
        .process_cycle()
        .await
        .expect("cycle succeeds");

    // spawns_failed must increment for the skipped record
    assert!(
        supervisor.metrics.spawns_failed.get() >= 1,
        "spawns_failed must increment when spawn_attempts > max_spawn_attempts",
    );
}

// =============================================================================
// BH-RL10: Failed records at limit never re-enter spawn phase
// =============================================================================

/// A record that has reached max_spawn_attempts must never transition
/// back to Spawn phase, even across many process_cycle calls.
#[tokio::test]
async fn bh_rl10_failed_at_limit_stays_failed_across_cycles() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(AlwaysFailProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let max_attempts = 5u32;
    let instance_id = test_instance_id();

    // Start with a record already at max attempts in Failed phase
    let record = SpawnRecord {
        spawn_id: None,
        instance_id: instance_id.clone(),
        command: "./crasher".to_string(),
        spawn_phase: SpawnPhase::Failed,
        health_checks: 0,
        spawn_attempts: max_attempts,
        last_error: None,
    };
    storage.add_record(record);

    let supervisor = SpawnSupervisor::new(
        Duration::from_millis(1),
        1,
        Duration::from_millis(1),
        1.0,
        max_attempts,
        storage.clone(),
        process_manager,
        work_queue.clone(),
    )
    .expect("Valid config");

    // Run many cycles — the record must never leave Failed
    for cycle in 0..20 {
        let _ = supervisor.process_cycle().await;

        let saved = storage.get_spawn_record(&instance_id).await.expect("must exist");
        assert_eq!(
            saved.spawn_phase,
            SpawnPhase::Failed,
            "After cycle {}: record at max_attempts must stay Failed, got {:?}",
            cycle,
            saved.spawn_phase,
        );
        assert_eq!(
            saved.spawn_attempts, max_attempts,
            "After cycle {}: spawn_attempts must not change from {}",
            cycle, max_attempts,
        );
    }

    // No work should ever be enqueued for this record
    assert_eq!(
        work_queue.enqueue_spawn_count(),
        0,
        "No work should be enqueued for record at max_attempts",
    );
}

/// Health check failure path: when health check fails and we're at
/// max_spawn_attempts, the record should NOT be respawned.
#[tokio::test]
async fn bh_rl10_health_check_failure_at_max_no_respawn() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(HealthCheckFailProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let max_attempts = 1u32;
    let instance_id = test_instance_id();

    // Record at attempt 1 (already at max) in Spawn phase
    let record = SpawnRecord {
        spawn_id: None,
        instance_id: instance_id.clone(),
        command: "./crasher".to_string(),
        spawn_phase: SpawnPhase::Spawn,
        health_checks: 0,
        spawn_attempts: max_attempts,
        last_error: None,
    };
    storage.add_record(record);

    let supervisor = SpawnSupervisor::new(
        Duration::from_millis(1),
        1,
        Duration::from_millis(1),
        1.0,
        max_attempts,
        storage.clone(),
        process_manager,
        work_queue.clone(),
    )
    .expect("Valid config");

    let _ = supervisor.process_cycle().await;

    // spawn_attempts == max_spawn_attempts, so no respawn scheduled
    // (the check is `record.spawn_attempts < self.max_spawn_attempts` at line 684)
    // Since 1 < 1 is false, no respawn happens
    let saved = storage.get_spawn_record(&instance_id).await.expect("must exist");
    // The record will be in Failed or HealthCheck depending on path
    // But crucially, no respawn work should be enqueued
    assert_eq!(
        work_queue.enqueue_spawn_count(),
        0,
        "No respawn work when spawn_attempts == max_spawn_attempts",
    );
}

// =============================================================================
// BH-RL11 (bonus): Full lifecycle — spawn to crash loop exhaustion
// =============================================================================

/// End-to-end adversarial: spawn a record, let it crash and restart
/// through the full lifecycle until exhausted. Verify the final state
/// is terminal and resource-bounded.
#[tokio::test]
async fn bh_rl11_full_lifecycle_crash_to_exhaustion() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(HealthCheckFailProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let max_attempts = 4u32;
    let instance_id = test_instance_id();

    // Start fresh — Spawn phase, attempt 1.
    // HealthCheckFailProcessManager succeeds on spawn but fails health checks,
    // exercising: Spawn → (spawn succeeds) → HealthCheck → (fails) → Failed → respawn → ...
    let record = SpawnRecord::new(instance_id.clone(), "./crasher".to_string(), None);
    storage.add_record(record);

    let supervisor = SpawnSupervisor::new(
        Duration::from_millis(1),
        1,
        Duration::from_millis(1),
        2.0,
        max_attempts,
        storage.clone(),
        process_manager,
        work_queue.clone(),
    )
    .expect("Valid config");

    // Run enough cycles to go through the full lifecycle
    for _ in 0..30 {
        let _ = supervisor.process_cycle().await;
    }

    // Verify final state
    let final_record = storage.get_spawn_record(&instance_id).await.expect("must exist");

    // The record should have exhausted its attempts
    assert!(
        final_record.spawn_attempts >= max_attempts,
        "After full lifecycle, spawn_attempts ({}) should be >= max ({})",
        final_record.spawn_attempts,
        max_attempts,
    );

    // Only one record in storage — no leaks
    assert_eq!(
        storage.record_count(),
        1,
        "After full lifecycle, exactly 1 record should exist",
    );

    // Metrics: health checks failed because the process passes spawn
    // but fails health checks every time
    let health_checks_failed = supervisor.metrics.health_checks_failed.get();
    assert!(
        health_checks_failed > 0,
        "health_checks_failed must be > 0 after crash loop",
    );

    // Respawns are bounded — both the Spawn-phase handler and Failed-phase
    // scan may increment this counter, so the bound is 2 * max_attempts
    let respawns = supervisor.metrics.respawns.get();
    assert!(
        respawns <= (max_attempts * 2) as u64,
        "respawns ({}) must be bounded by 2 * max_attempts ({})",
        respawns, max_attempts,
    );
}
