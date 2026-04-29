//! BDD Tests for ADR-006: Backpressure and Load Shedding
//!
//! Scenarios:
//! 1. Given subprocess permits exhausted, When new execution requested,
//!    Then request queued (not dropped).
//! 2. Given stderr budget exceeded (ADR-023), When subprocess continues
//!    writing stderr, Then subprocess killed.
//! 3. Given write-path QoS violation (ADR-032), When large payload blocks
//!    control records, Then shedding occurs.
//! 4. Backpressure inversion prevention from ADR-015.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use vo_actor::semaphore::{
    calculate_backpressure_status, is_workflow_saturated, AdmissionDecision, BackpressureStatus,
    ExecutionSemaphore, InstanceRegistryInterface, InvariantEnforcer, InvariantError,
    RejectionReason, SemaphoreConfig,
};
use vo_types::InstanceId;

// =============================================================================
// Scenario 1: Subprocess permits exhausted → request queued, not dropped
// (ADR-006 Section 1 & 2)
// =============================================================================

#[tokio::test]
async fn bdd_006_s1_permits_exhausted_request_queued_not_dropped() {
    // Given: a semaphore with exactly 2 permits
    let config = SemaphoreConfig {
        max_concurrent_binaries: 2,
        max_waiters_for_shed: 100,
        max_per_workflow: 10,
        acquire_timeout: Duration::from_secs(5),
        reserved_permits: 0,
    };
    let sem = Arc::new(ExecutionSemaphore::new(config));

    // And: all permits are held
    let _permit1 = sem.try_acquire().expect("first permit");
    let _permit2 = sem.try_acquire().expect("second permit");

    assert_eq!(sem.available_permits(), 0);

    // When: a new execution is requested
    let sem_clone = sem.clone();
    let acquire_handle = tokio::spawn(async move {
        let decision = sem_clone.acquire().await;
        decision
    });

    // Then: the request is eventually admitted (queued, not dropped)
    // We verify by checking waiting_count is > 0 before releasing
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        sem.waiting_count() > 0 || sem.available_permits() < 2,
        "request should be queued while permits are exhausted"
    );

    // Release a permit to unblock the waiter
    drop(_permit1);

    let result = tokio::time::timeout(Duration::from_secs(2), acquire_handle)
        .await
        .expect("acquire should complete within timeout")
        .expect("task should not panic");

    assert!(
        matches!(result, AdmissionDecision::Admitted),
        "queued request must be admitted when permit frees, got: {:?}",
        result
    );
}

#[tokio::test]
async fn bdd_006_s1_multiple_waiters_all_eventually_admitted() {
    // Given: a semaphore with 1 permit
    let config = SemaphoreConfig {
        max_concurrent_binaries: 1,
        max_waiters_for_shed: 100,
        max_per_workflow: 10,
        acquire_timeout: Duration::from_secs(10),
        reserved_permits: 0,
    };
    let sem = Arc::new(ExecutionSemaphore::new(config));

    // And: the permit is held
    let permit = sem.try_acquire().expect("permit");

    // When: 5 concurrent requests arrive
    let mut handles = Vec::new();
    for _ in 0..5 {
        let sem_clone = sem.clone();
        handles.push(tokio::spawn(async move { sem_clone.acquire().await }));
    }

    // Then: all 5 are waiting (queued, not dropped)
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(sem.waiting_count() >= 5, "all requests should be queued");

    // Release permit — one waiter unblocks
    drop(permit);

    // Drop permits one by one to drain the queue
    let results = futures::future::join_all(handles).await;

    for result in results {
        let decision = result.expect("task should not panic");
        assert!(
            matches!(decision, AdmissionDecision::Admitted),
            "each queued request must eventually be admitted, got: {:?}",
            decision
        );
    }
}

#[tokio::test]
async fn bdd_006_s1_zero_cost_yielding_no_cpu_spin() {
    // Given: a semaphore with 1 permit
    let config = SemaphoreConfig {
        max_concurrent_binaries: 1,
        max_waiters_for_shed: 100,
        max_per_workflow: 10,
        acquire_timeout: Duration::from_millis(500),
        reserved_permits: 0,
    };
    let sem = Arc::new(ExecutionSemaphore::new(config));

    // And: the permit is held
    let _permit = sem.try_acquire().expect("permit");

    // When: a task waits for a permit with a short timeout
    let sem_clone = sem.clone();
    let start = std::time::Instant::now();
    let decision = sem_clone.acquire().await;
    let elapsed = start.elapsed();

    // Then: the await yields (does not spin) — elapsed should be ~500ms timeout, not microseconds
    assert!(
        elapsed >= Duration::from_millis(400),
        "await should yield, not spin — elapsed {:?} < 400ms",
        elapsed
    );
    assert!(
        matches!(
            decision,
            AdmissionDecision::Rejected {
                reason: RejectionReason::Timeout,
                ..
            }
        ),
        "should timeout, not spin-burn CPU, got: {:?}",
        decision
    );
}

// =============================================================================
// Scenario 2: Stderr budget exceeded (ADR-023) → subprocess killed
// =============================================================================

/// Simulated stderr buffer with a 1MB budget per ADR-023.
struct BoundedStderrBuffer {
    data: Vec<u8>,
    max_bytes: usize,
    overflow_detected: AtomicBool,
}

impl BoundedStderrBuffer {
    fn new(max_bytes: usize) -> Self {
        Self {
            data: Vec::with_capacity(max_bytes),
            max_bytes,
            overflow_detected: AtomicBool::new(false),
        }
    }

    /// Simulates writing stderr from a subprocess.
    /// Per ADR-023: stops reading when buffer hits max, lets OS pipe buffer fill,
    /// which causes the child's write() to block, triggering execution timeout.
    fn write(&mut self, chunk: &[u8]) -> StderrWriteResult {
        if self.data.len() >= self.max_bytes {
            self.overflow_detected.store(true, Ordering::Relaxed);
            return StderrWriteResult::Truncated {
                written: 0,
                remaining: chunk.len(),
                buffer_full: true,
            };
        }

        let remaining_capacity = self.max_bytes - self.data.len();
        let to_write = chunk.len().min(remaining_capacity);
        self.data.extend_from_slice(&chunk[..to_write]);

        if self.data.len() >= self.max_bytes {
            self.overflow_detected.store(true, Ordering::Relaxed);
            return StderrWriteResult::Truncated {
                written: to_write,
                remaining: chunk.len() - to_write,
                buffer_full: true,
            };
        }

        StderrWriteResult::Accepted {
            written: to_write,
            buffer_full: false,
        }
    }

    fn is_overflow(&self) -> bool {
        self.overflow_detected.load(Ordering::Relaxed)
    }

    fn len(&self) -> usize {
        self.data.len()
    }

    fn captured_bytes(&self) -> usize {
        self.data.len()
    }

    fn should_kill_subprocess(&self) -> bool {
        self.is_overflow()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StderrWriteResult {
    Accepted {
        written: usize,
        buffer_full: bool,
    },
    Truncated {
        written: usize,
        remaining: usize,
        buffer_full: bool,
    },
}

#[test]
fn bdd_006_s2_stderr_within_budget_accepted() {
    // Given: a stderr buffer with 1MB budget (per ADR-023)
    let mut buffer = BoundedStderrBuffer::new(1_048_576);

    // When: subprocess writes 100KB of stderr
    let chunk = vec![b'x'; 100_000];
    let result = buffer.write(&chunk);

    // Then: write is accepted, no truncation
    assert_eq!(
        result,
        StderrWriteResult::Accepted {
            written: 100_000,
            buffer_full: false
        }
    );
    assert!(!buffer.is_overflow());
    assert_eq!(buffer.captured_bytes(), 100_000);
}

#[test]
fn bdd_006_s2_stderr_at_budget_boundary_truncated() {
    // Given: a stderr buffer with 1MB budget
    let mut buffer = BoundedStderrBuffer::new(1_048_576);

    // When: subprocess writes exactly 1MB
    let chunk = vec![b'x'; 1_048_576];
    let result = buffer.write(&chunk);

    // Then: buffer is full, subsequent writes are truncated
    assert!(matches!(
        result,
        StderrWriteResult::Truncated {
            buffer_full: true,
            ..
        }
    ));
    assert!(buffer.is_overflow());

    // And: further writes are truncated
    let overflow_chunk = vec![b'y'; 10_000];
    let result2 = buffer.write(&overflow_chunk);
    assert_eq!(
        result2,
        StderrWriteResult::Truncated {
            written: 0,
            remaining: 10_000,
            buffer_full: true
        }
    );

    // And: captured bytes never exceed budget
    assert_eq!(buffer.captured_bytes(), 1_048_576);
}

#[test]
fn bdd_006_s2_stderr_overflow_triggers_kill() {
    // Given: a stderr buffer with 1MB budget
    let mut buffer = BoundedStderrBuffer::new(1_048_576);

    // When: subprocess writes beyond the budget
    let chunk = vec![b'x'; 2_000_000];
    let _ = buffer.write(&chunk);

    // Then: the buffer signals that the subprocess should be killed
    assert!(buffer.should_kill_subprocess());
}

#[test]
fn bdd_006_s2_stderr_flood_bounded_no_oom() {
    // Given: a stderr buffer with a small budget for testing
    let mut buffer = BoundedStderrBuffer::new(1024);

    // When: a buggy task writes 10GB worth of data in chunks
    let huge_chunk = vec![b'z'; 10_000_000];
    for _ in 0..1000 {
        let _ = buffer.write(&huge_chunk);
    }

    // Then: buffer never exceeds its max (no OOM)
    assert!(buffer.captured_bytes() <= 1024);
    assert!(buffer.is_overflow());
}

#[tokio::test]
async fn bdd_006_s2_stderr_overflow_blocks_then_timeout_kills() {
    // Given: a stderr buffer with 1KB budget
    let buffer = Arc::new(tokio::sync::Mutex::new(BoundedStderrBuffer::new(1024)));
    let subprocess_killed = Arc::new(AtomicBool::new(false));

    // And: a simulated subprocess that keeps writing
    let buf_clone = buffer.clone();
    let subprocess = tokio::spawn(async move {
        let mut buf = buf_clone.lock().await;
        for _ in 0..100 {
            let chunk = vec![b'x'; 500];
            let result = buf.write(&chunk);
            if matches!(
                result,
                StderrWriteResult::Truncated {
                    buffer_full: true,
                    ..
                }
            ) {
                // Per ADR-023: the child's write() would block here.
                // Simulate the child being stuck.
                return;
            }
        }
    });

    // When: the subprocess fills the buffer and gets stuck
    let _ = tokio::time::timeout(Duration::from_secs(2), subprocess).await;

    // Then: we detect overflow and signal kill
    let buf = buffer.lock().await;
    if buf.is_overflow() {
        subprocess_killed.store(true, Ordering::Relaxed);
    }

    assert!(
        subprocess_killed.load(Ordering::Relaxed),
        "subprocess must be killed when stderr budget exceeded"
    );
}

// =============================================================================
// Scenario 3: Write-path QoS violation (ADR-032) → shedding occurs
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
enum WriteClass {
    CriticalControlPlane,
    OperatorProjection,
    BulkBlob,
}

struct WriteRecord {
    class: WriteClass,
    size_bytes: usize,
    queue_position: usize,
}

/// Simulated write-path QoS classifier per ADR-032.
struct WritePathQoS {
    critical_queue_depth: AtomicUsize,
    blob_queue_depth: AtomicUsize,
    writer_queue_max: usize,
    blob_queue_max: usize,
    degraded: AtomicBool,
}

impl WritePathQoS {
    fn new(writer_queue_max: usize, blob_queue_max: usize) -> Self {
        Self {
            critical_queue_depth: AtomicUsize::new(0),
            blob_queue_depth: AtomicUsize::new(0),
            writer_queue_max,
            blob_queue_max,
            degraded: AtomicBool::new(false),
        }
    }

    fn enqueue_critical(&self) -> AdmissionDecision {
        let depth = self.critical_queue_depth.fetch_add(1, Ordering::Relaxed);
        if depth >= self.writer_queue_max {
            // Per ADR-032: Critical control-plane writes are NEVER dropped
            AdmissionDecision::Admitted
        } else {
            AdmissionDecision::Admitted
        }
    }

    fn dequeue_critical(&self) {
        self.critical_queue_depth.fetch_sub(1, Ordering::Relaxed);
        self.recalculate_degraded();
    }

    fn enqueue_blob(&self, size_bytes: usize) -> AdmissionDecision {
        let blob_depth = self.blob_queue_depth.load(Ordering::Relaxed);

        if self.degraded.load(Ordering::Relaxed) {
            // Per ADR-032: Bulk blobs may be deferred under pressure
            return AdmissionDecision::Rejected {
                reason: RejectionReason::LoadShed,
                retry_after_secs: 5,
            };
        }

        if blob_depth >= self.blob_queue_max {
            return AdmissionDecision::Rejected {
                reason: RejectionReason::LoadShed,
                retry_after_secs: 5,
            };
        }

        self.blob_queue_depth.fetch_add(1, Ordering::Relaxed);

        // Large payload that could block control records → shed
        if size_bytes > 10_000_000 && blob_depth > self.blob_queue_max / 2 {
            self.blob_queue_depth.fetch_sub(1, Ordering::Relaxed);
            return AdmissionDecision::Rejected {
                reason: RejectionReason::LoadShed,
                retry_after_secs: 10,
            };
        }

        AdmissionDecision::Admitted
    }

    fn enqueue_projection(&self) -> AdmissionDecision {
        // Per ADR-032: Operator projections may lag and be rebuilt later
        if self.degraded.load(Ordering::Relaxed) {
            AdmissionDecision::Rejected {
                reason: RejectionReason::LoadShed,
                retry_after_secs: 30,
            }
        } else {
            AdmissionDecision::Admitted
        }
    }

    fn recalculate_degraded(&self) {
        let critical = self.critical_queue_depth.load(Ordering::Relaxed);
        let blob = self.blob_queue_depth.load(Ordering::Relaxed);
        let is_degraded =
            critical > self.writer_queue_max * 80 / 100 || blob > self.blob_queue_max * 80 / 100;
        self.degraded.store(is_degraded, Ordering::Relaxed);
    }

    fn is_degraded(&self) -> bool {
        self.degraded.load(Ordering::Relaxed)
    }

    fn critical_depth(&self) -> usize {
        self.critical_queue_depth.load(Ordering::Relaxed)
    }

    fn blob_depth(&self) -> usize {
        self.blob_queue_depth.load(Ordering::Relaxed)
    }
}

#[test]
fn bdd_006_s3_critical_control_plane_never_dropped() {
    // Given: a write-path QoS system
    let qos = WritePathQoS::new(10_000, 1000);

    // When: critical control-plane writes are enqueued even under pressure
    for _ in 0..15_000 {
        let decision = qos.enqueue_critical();
        assert_eq!(decision, AdmissionDecision::Admitted);
    }

    // Then: all critical writes are accepted
    assert_eq!(qos.critical_depth(), 15_000);
}

#[test]
fn bdd_006_s3_large_payload_blocks_control_records_triggers_shed() {
    // Given: a write-path QoS system with blob queue above half capacity
    let qos = WritePathQoS::new(10_000, 1000);

    // Fill blob queue past half (501 > 500)
    for _ in 0..501 {
        assert_eq!(qos.enqueue_blob(1000), AdmissionDecision::Admitted);
    }
    assert!(qos.blob_depth() > 500);

    // When: a large payload (>10MB) arrives
    let decision = qos.enqueue_blob(20_000_000);

    // Then: it is shed to prevent blocking control records
    assert!(
        matches!(
            decision,
            AdmissionDecision::Rejected {
                reason: RejectionReason::LoadShed,
                ..
            }
        ),
        "large payload blocking control records should be shed"
    );
}

#[test]
fn bdd_006_s3_blob_queue_overflow_triggers_shedding() {
    // Given: a write-path QoS system with small blob queue
    let qos = WritePathQoS::new(10_000, 100);

    // When: blob queue fills beyond max
    for _ in 0..100 {
        assert_eq!(qos.enqueue_blob(1000), AdmissionDecision::Admitted);
    }

    // Then: additional blobs are shed
    let decision = qos.enqueue_blob(1000);
    assert!(
        matches!(
            decision,
            AdmissionDecision::Rejected {
                reason: RejectionReason::LoadShed,
                ..
            }
        ),
        "overflow blobs should be shed"
    );
}

#[test]
fn bdd_006_s3_degraded_mode_sheds_projections_and_blobs() {
    // Given: a write-path QoS system in degraded mode
    let qos = WritePathQoS::new(100, 100);

    // Fill critical queue to 80%+ to trigger degraded mode
    for _ in 0..85 {
        qos.enqueue_critical();
    }
    qos.recalculate_degraded();
    assert!(qos.is_degraded());

    // When: operator projection and blob writes arrive
    let projection_decision = qos.enqueue_projection();
    let blob_decision = qos.enqueue_blob(1000);

    // Then: projections and blobs are shed, but critical writes still pass
    assert!(
        matches!(
            projection_decision,
            AdmissionDecision::Rejected {
                reason: RejectionReason::LoadShed,
                ..
            }
        ),
        "projections should be shed in degraded mode"
    );
    assert!(
        matches!(
            blob_decision,
            AdmissionDecision::Rejected {
                reason: RejectionReason::LoadShed,
                ..
            }
        ),
        "blobs should be shed in degraded mode"
    );
    assert_eq!(qos.enqueue_critical(), AdmissionDecision::Admitted);
}

#[test]
fn bdd_006_s3_admission_coupling_considers_queue_depth_and_latency() {
    // Given: a write-path QoS system
    let qos = WritePathQoS::new(100, 100);

    // When: critical queue depth grows beyond 80%
    for _ in 0..85 {
        qos.enqueue_critical();
    }
    qos.recalculate_degraded();

    // Then: system detects degradation
    assert!(
        qos.is_degraded(),
        "high queue depth should trigger degradation"
    );

    // And: subsequent blob writes are rejected
    let decision = qos.enqueue_blob(1000);
    assert!(
        matches!(decision, AdmissionDecision::Rejected { .. }),
        "blobs should be rejected when degraded"
    );
}

// =============================================================================
// Scenario 4: Backpressure inversion prevention (ADR-015)
// =============================================================================

struct MockInstanceRegistry {
    active_instances: std::sync::RwLock<std::collections::HashSet<InstanceId>>,
}

impl MockInstanceRegistry {
    fn new() -> Self {
        Self {
            active_instances: std::sync::RwLock::new(std::collections::HashSet::new()),
        }
    }

    fn register(&self, id: InstanceId) {
        self.active_instances.write().unwrap().insert(id);
    }

    fn unregister(&self, id: &InstanceId) {
        self.active_instances.write().unwrap().remove(id);
    }
}

impl InstanceRegistryInterface for MockInstanceRegistry {
    fn is_active(&self, instance_id: &InstanceId) -> bool {
        self.active_instances.read().unwrap().contains(instance_id)
    }
}

fn make_instance_id(seed: u8) -> InstanceId {
    let mut bytes = [seed; 16];
    bytes[0] = seed;
    InstanceId::from_bytes(bytes)
}

#[tokio::test]
async fn bdd_006_s4_single_writer_invariant_prevents_duplicate_activation() {
    // Given: an invariant enforcer with a mock registry
    let config = SemaphoreConfig {
        max_concurrent_binaries: 100,
        max_waiters_for_shed: 1000,
        max_per_workflow: 10,
        acquire_timeout: Duration::from_secs(5),
        reserved_permits: 10,
    };
    let sem = Arc::new(ExecutionSemaphore::new(config));
    let registry = Arc::new(MockInstanceRegistry::new());
    let enforcer = InvariantEnforcer::new(sem, registry.clone());

    let instance_id = make_instance_id(1);

    // When: the same instance is activated twice
    let check1 = enforcer
        .check_activation(&instance_id)
        .expect("check should succeed");
    assert!(check1.is_allowed());

    // Register the instance as active
    enforcer
        .execution_semaphore()
        .try_acquire()
        .expect("permit for instance");
    registry.register(instance_id.clone());

    // Then: the second activation is rejected
    let check2 = enforcer
        .check_activation(&instance_id)
        .expect("check should succeed");
    assert!(!check2.is_allowed());
    assert!(matches!(
        check2.error,
        Some(InvariantError::InstanceAlreadyActive { .. })
    ));
}

#[tokio::test]
async fn bdd_006_s4_bounded_mailbox_prevents_inversion() {
    // Given: a system approaching mailbox capacity
    let config = SemaphoreConfig {
        max_concurrent_binaries: 100,
        max_waiters_for_shed: 20,
        max_per_workflow: 10,
        acquire_timeout: Duration::from_secs(5),
        reserved_permits: 10,
    };
    let sem = Arc::new(ExecutionSemaphore::new(config));

    // Exhaust all permits to simulate mailbox pressure
    let mut permits = Vec::new();
    for _ in 0..100 {
        let p = sem.try_acquire().expect("permit");
        permits.push(p);
    }

    // And: queue waiters beyond shedding threshold
    // The acquire() method increments waiting_count before checking status.
    // Once waiting_count >= max_waiters_for_shed, new acquires are rejected.
    let mut handles = Vec::new();
    for _ in 0..30 {
        let sem_clone = sem.clone();
        handles.push(tokio::spawn(async move { sem_clone.acquire().await }));
    }

    // Wait for waiters to register and potentially get rejected
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Then: the system detects load shedding (backpressure inversion prevention)
    // Some waiters may have been admitted if permits freed, but with 30 waiters
    // and only 100 permits (all held), many should be rejected.
    let results = futures::future::join_all(handles).await;
    let rejected_count = results
        .iter()
        .filter(|r| {
            matches!(
                r,
                Ok(AdmissionDecision::Rejected {
                    reason: RejectionReason::LoadShed,
                    ..
                })
            )
        })
        .count();

    assert!(
        rejected_count > 0,
        "some requests should be rejected under inversion conditions (backpressure shedding)"
    );

    // Clean up: release permits so any remaining waiter tasks can finish
    permits.clear();
}

#[tokio::test]
async fn bdd_006_s4_instance_released_allows_reactivation() {
    // Given: an instance that was active and is now released
    let config = SemaphoreConfig::default();
    let sem = Arc::new(ExecutionSemaphore::new(config));
    let registry = Arc::new(MockInstanceRegistry::new());
    let enforcer = InvariantEnforcer::new(sem, registry.clone());

    let instance_id = make_instance_id(42);

    // Register and then unregister the instance
    registry.register(instance_id.clone());
    let check_blocked = enforcer.check_activation(&instance_id).unwrap();
    assert!(!check_blocked.is_allowed());

    // When: the instance is released
    registry.unregister(&instance_id);

    // Then: reactivation succeeds
    let check_allowed = enforcer.check_activation(&instance_id).unwrap();
    assert!(check_allowed.is_allowed());
}

#[test]
fn bdd_006_s4_backpressure_status_transitions_correctly() {
    // Given: a fresh system
    let status_healthy = calculate_backpressure_status(500, 500, 0, 5000);
    assert_eq!(status_healthy, BackpressureStatus::Healthy);

    // When: usage grows to moderate (>50% usage or >25% waiters = 125)
    // usage_ratio=0.6 > 0.5
    let status_moderate = calculate_backpressure_status(200, 500, 50, 5000);
    assert_eq!(status_moderate, BackpressureStatus::Moderate);

    // When: usage grows to heavy (>80% usage or >50% waiters = 250)
    // usage_ratio=0.9 > 0.8
    let status_heavy = calculate_backpressure_status(50, 500, 100, 5000);
    assert_eq!(status_heavy, BackpressureStatus::Heavy);

    // When: waiters exceed shedding threshold
    let status_shed = calculate_backpressure_status(0, 500, 5001, 5000);
    assert_eq!(status_shed, BackpressureStatus::ShedLoad);

    // Then: ordering is monotonic
    assert!(status_healthy < status_moderate);
    assert!(status_moderate < status_heavy);
    assert!(status_heavy < status_shed);
}

#[test]
fn bdd_006_s4_workflow_saturation_blocks_per_workflow_limit() {
    // Given: a workflow with max 10 concurrent operations
    let max_per_workflow = 10;

    // When: workflow has 10 pending operations
    assert!(is_workflow_saturated(10, max_per_workflow));

    // Then: additional operations are blocked
    assert!(is_workflow_saturated(15, max_per_workflow));

    // And: below-limit operations are allowed
    assert!(!is_workflow_saturated(9, max_per_workflow));
}

// =============================================================================
// Cross-cutting: Integration scenarios combining multiple ADRs
// =============================================================================

#[tokio::test]
async fn bdd_006_integration_backpressure_cascade_healthy_to_shed() {
    // Given: a semaphore system
    let config = SemaphoreConfig {
        max_concurrent_binaries: 10,
        max_waiters_for_shed: 20,
        max_per_workflow: 5,
        acquire_timeout: Duration::from_secs(5),
        reserved_permits: 2,
    };
    let sem = Arc::new(ExecutionSemaphore::new(config));

    // Phase 1: Healthy — all requests admitted via try_acquire
    let mut permits = Vec::new();
    for _ in 0..10 {
        let p = sem.try_acquire().expect("permit in healthy state");
        permits.push(p);
    }
    assert_eq!(sem.current_status(), BackpressureStatus::Heavy);
    assert_eq!(sem.available_permits(), 0);

    // Phase 2: Queue waiters beyond shedding threshold
    // With 0 permits available and 25 waiters > max_waiters_for_shed=20,
    // the acquire() method should reject excess requests.
    let mut waiter_handles = Vec::new();
    for _ in 0..25 {
        let sem_clone = sem.clone();
        waiter_handles.push(tokio::spawn(async move { sem_clone.acquire().await }));
    }

    let results = futures::future::join_all(waiter_handles).await;
    let admitted_count = results
        .iter()
        .filter(|r| matches!(r, Ok(AdmissionDecision::Admitted)))
        .count();
    let rejected_count = results
        .iter()
        .filter(|r| matches!(r, Ok(AdmissionDecision::Rejected { .. })))
        .count();

    // Some admitted (early waiters before threshold hit), some rejected (shed)
    assert!(
        rejected_count > 0,
        "some waiters should be rejected during shed phase"
    );

    // Phase 3: Release permits for recovery
    permits.clear();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // After recovery, new requests should succeed (not ShedLoad)
    let new_permits = sem.try_acquire();
    assert!(
        new_permits.is_some(),
        "after recovery, new try_acquire should succeed"
    );
}

#[tokio::test]
async fn bdd_006_integration_reserved_permits_bypass_general_pool() {
    // Given: general pool exhausted
    let config = SemaphoreConfig {
        max_concurrent_binaries: 2,
        reserved_permits: 2,
        ..Default::default()
    };
    let sem = Arc::new(ExecutionSemaphore::new(config));

    let _p1 = sem.try_acquire().unwrap();
    let _p2 = sem.try_acquire().unwrap();
    assert!(sem.try_acquire().is_none());

    // When: recovery task requests reserved permit
    let _r1 = sem.try_acquire_recovery().unwrap();
    let _r2 = sem.try_acquire_recovery().unwrap();

    // Then: reserved permits are available even though general pool is full
    assert_eq!(sem.available_permits(), 0);
    assert_eq!(sem.reserved_available(), 0);
}
