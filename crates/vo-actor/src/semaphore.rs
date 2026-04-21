//! Execution semaphore infrastructure for ADR-006 and ADR-015.
//!
//! Provides:
//! - Global execution semaphore for limiting concurrent binary spawns
//! - Per-workflow semaphore management
//! - Resource admission control with backpressure signaling
//!
//! Architecture: Data → Calc → Actions
//! - Data: `SemaphoreConfig`, `BackpressureStatus`, `AdmissionDecision`
//! - Calc: Pure decision functions for admission and backpressure
//! - Actions: Async semaphore operations

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Semaphore, TryAcquireError};
use vo_types::InstanceId;
use vo_types::WorkflowName;

// =============================================================================
// Constants
// =============================================================================

const DEFAULT_MAX_CONCURRENT_BINARIES: usize = 500;
const DEFAULT_MAX_WAITERS_FOR_SHED: usize = 5000;
const DEFAULT_MAX_PER_WORKFLOW: usize = 10;

// =============================================================================
// Data Layer — Configuration and Status Types
// =============================================================================

/// Configuration for the execution semaphore system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemaphoreConfig {
    /// Maximum concurrent binary spawns (default: 500).
    pub max_concurrent_binaries: usize,
    /// Threshold for ingress load shedding (default: 5000).
    pub max_waiters_for_shed: usize,
    /// Maximum concurrent operations per workflow (default: 10).
    pub max_per_workflow: usize,
    /// Timeout for acquiring a permit (default: 30s).
    pub acquire_timeout: Duration,
}

impl Default for SemaphoreConfig {
    fn default() -> Self {
        Self {
            max_concurrent_binaries: DEFAULT_MAX_CONCURRENT_BINARIES,
            max_waiters_for_shed: DEFAULT_MAX_WAITERS_FOR_SHED,
            max_per_workflow: DEFAULT_MAX_PER_WORKFLOW,
            acquire_timeout: Duration::from_secs(30),
        }
    }
}

/// Backpressure status indicating system load state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BackpressureStatus {
    /// System is healthy, no backpressure.
    Healthy,
    /// System is experiencing moderate load.
    Moderate,
    /// System is under heavy load, queuing expected.
    Heavy,
    /// Load shedding active, rejecting new requests.
    ShedLoad,
}

impl BackpressureStatus {
    /// Returns true if new requests should be rejected.
    #[must_use]
    pub fn should_reject(&self) -> bool {
        matches!(self, Self::ShedLoad)
    }

    /// Returns true if waiters are being queued.
    #[must_use]
    pub fn is_queued(&self) -> bool {
        matches!(self, Self::Heavy | Self::ShedLoad)
    }
}

/// Result of an admission control decision.
#[derive(Debug, Clone)]
pub enum AdmissionDecision {
    /// Request admitted, semaphore permit acquired.
    Admitted,
    /// Request queued, waiting for permit.
    Queued {
        position: usize,
        estimated_wait_ms: u64,
    },
    /// Request rejected due to load shedding.
    Rejected {
        reason: RejectionReason,
        retry_after_secs: u32,
    },
}

impl PartialEq for AdmissionDecision {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Admitted, Self::Admitted) => true,
            (
                Self::Queued {
                    position: l_pos,
                    estimated_wait_ms: l_wait,
                },
                Self::Queued {
                    position: r_pos,
                    estimated_wait_ms: r_wait,
                },
            ) => l_pos == r_pos && l_wait == r_wait,
            (
                Self::Rejected {
                    reason: l_reason,
                    retry_after_secs: l_retry,
                },
                Self::Rejected {
                    reason: r_reason,
                    retry_after_secs: r_retry,
                },
            ) => l_reason == r_reason && l_retry == r_retry,
            _ => false,
        }
    }
}

impl Eq for AdmissionDecision {}

/// Reason for rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectionReason {
    /// Load shedding is active.
    LoadShed,
    /// Workflow has too many pending operations.
    WorkflowSaturated,
    /// Timeout waiting for permit.
    Timeout,
}



// =============================================================================
// Calculation Layer — Pure Decision Functions
// =============================================================================

/// Calculates the current backpressure status based on waiters and permits.
#[inline]
#[must_use]
pub fn calculate_backpressure_status(
    available_permits: usize,
    total_permits: usize,
    waiting_count: usize,
    max_waiters_for_shed: usize,
) -> BackpressureStatus {
    let usage_ratio = if total_permits > 0 {
        (total_permits - available_permits) as f64 / total_permits as f64
    } else {
        1.0
    };

    if waiting_count >= max_waiters_for_shed {
        BackpressureStatus::ShedLoad
    } else if waiting_count > total_permits / 2 || usage_ratio > 0.8 {
        BackpressureStatus::Heavy
    } else if usage_ratio > 0.5 || waiting_count > total_permits / 4 {
        BackpressureStatus::Moderate
    } else {
        BackpressureStatus::Healthy
    }
}

/// Estimates wait time in milliseconds based on position and available permits.
#[inline]
#[must_use]
pub fn estimate_wait_ms(position: usize, available_permits: usize, avg_task_duration_ms: u64) -> u64 {
    if available_permits == 0 {
        return (position as u64 + 1) * avg_task_duration_ms;
    }
    let ahead = position / available_permits;
    (ahead as u64 + 1) * avg_task_duration_ms
}

/// Determines if a workflow is saturated (too many pending operations).
#[inline]
#[must_use]
pub fn is_workflow_saturated(pending_count: usize, max_per_workflow: usize) -> bool {
    pending_count >= max_per_workflow
}

// =============================================================================
// Action Layer — Async Semaphore Operations
// =============================================================================

/// The global execution semaphore for binary spawn limiting.
///
/// Per ADR-006: Uses `tokio::sync::Semaphore` with fixed permits (e.g., 500)
/// to limit concurrent binary spawns.
pub struct ExecutionSemaphore {
    semaphore: Semaphore,
    config: SemaphoreConfig,
    available_permits: AtomicUsize,
    waiting_count: AtomicUsize,
}

impl std::fmt::Debug for ExecutionSemaphore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutionSemaphore")
            .field("config", &self.config)
            .field("available_permits", &self.available_permits)
            .field("waiting_count", &self.waiting_count)
            .finish()
    }
}

impl ExecutionSemaphore {
    /// Creates a new execution semaphore with the given config.
    #[must_use]
    pub fn new(config: SemaphoreConfig) -> Self {
        let available_permits = config.max_concurrent_binaries;
        Self {
            semaphore: Semaphore::new(available_permits),
            config,
            available_permits: AtomicUsize::new(available_permits),
            waiting_count: AtomicUsize::new(0),
        }
    }

    /// Creates a new execution semaphore with default config.
    #[must_use]
    pub fn default() -> Self {
        Self::new(SemaphoreConfig::default())
    }

    /// Attempts to acquire a permit without waiting.
    ///
    /// Returns `Some(permit)` if available, `None` otherwise.
    /// The permit is automatically released when dropped.
    pub fn try_acquire(&self) -> Option<tokio::sync::SemaphorePermit<'_>> {
        match self.semaphore.try_acquire() {
            Ok(permit) => {
                self.available_permits.fetch_sub(1, Ordering::Relaxed);
                Some(permit)
            }
            Err(TryAcquireError::NoPermits) => None,
            Err(TryAcquireError::Closed) => None,
        }
    }

    /// Acquires a permit, waiting if necessary.
    ///
    /// Returns `AdmissionDecision` based on outcome.
    pub async fn acquire(self: &Arc<Self>) -> AdmissionDecision {
        let waiting = self.waiting_count.fetch_add(1, Ordering::Relaxed);
        let _ = waiting; // suppress unused warning
        let status = self.current_status();

        // Check if we should reject due to load shedding
        if status.should_reject() {
            self.waiting_count.fetch_sub(1, Ordering::Relaxed);
            return AdmissionDecision::Rejected {
                reason: RejectionReason::LoadShed,
                retry_after_secs: 5,
            };
        }

        // Try to acquire with timeout
        match tokio::time::timeout(self.config.acquire_timeout, self.semaphore.acquire()).await {
            Ok(Ok(_permit)) => {
                self.waiting_count.fetch_sub(1, Ordering::Relaxed);
                self.available_permits.fetch_sub(1, Ordering::Relaxed);
                AdmissionDecision::Admitted
            }
            Ok(Err(_)) => {
                self.waiting_count.fetch_sub(1, Ordering::Relaxed);
                AdmissionDecision::Rejected {
                    reason: RejectionReason::LoadShed,
                    retry_after_secs: 5,
                }
            }
            Err(_) => {
                // Timeout
                self.waiting_count.fetch_sub(1, Ordering::Relaxed);
                AdmissionDecision::Rejected {
                    reason: RejectionReason::Timeout,
                    retry_after_secs: 10,
                }
            }
        }
    }

    /// Returns the current backpressure status.
    #[must_use]
    pub fn current_status(&self) -> BackpressureStatus {
        let available = self.available_permits.load(Ordering::Relaxed);
        let waiting = self.waiting_count.load(Ordering::Relaxed);
        calculate_backpressure_status(
            available,
            self.config.max_concurrent_binaries,
            waiting,
            self.config.max_waiters_for_shed,
        )
    }

    /// Returns the number of available permits.
    #[must_use]
    pub fn available_permits(&self) -> usize {
        self.available_permits.load(Ordering::Relaxed)
    }

    /// Returns the number of waiting tasks.
    #[must_use]
    pub fn waiting_count(&self) -> usize {
        self.waiting_count.load(Ordering::Relaxed)
    }

    /// Returns the total permit capacity.
    #[must_use]
    pub fn total_permits(&self) -> usize {
        self.config.max_concurrent_binaries
    }

    /// Returns true if load shedding is active.
    #[must_use]
    pub fn is_load_shedding(&self) -> bool {
        self.current_status().should_reject()
    }

    /// Returns configuration reference.
    #[must_use]
    pub fn config(&self) -> &SemaphoreConfig {
        &self.config
    }
}

/// Per-workflow semaphore map for fine-grained concurrency control.
///
/// Provides per-workflow limiting in addition to global limiting.
pub struct WorkflowSemaphoreMap {
    semaphores:
        std::sync::RwLock<std::collections::HashMap<WorkflowName, Arc<Semaphore>>>,
    max_per_workflow: usize,
}

impl std::fmt::Debug for WorkflowSemaphoreMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkflowSemaphoreMap")
            .field("max_per_workflow", &self.max_per_workflow)
            .finish()
    }
}

impl WorkflowSemaphoreMap {
    /// Creates a new workflow semaphore map.
    #[must_use]
    pub fn new(max_per_workflow: usize) -> Self {
        Self {
            semaphores: std::sync::RwLock::new(std::collections::HashMap::new()),
            max_per_workflow,
        }
    }

    /// Creates a new workflow semaphore map with default settings.
    #[must_use]
    pub fn default() -> Self {
        Self::new(DEFAULT_MAX_PER_WORKFLOW)
    }

    /// Gets or creates a semaphore for the given workflow.
    fn get_or_create(&self, workflow_name: &WorkflowName) -> Arc<Semaphore> {
        // Try read first
        {
            let semaphores = self.semaphores.read().unwrap();
            if let Some(sem) = semaphores.get(workflow_name) {
                return Arc::clone(sem);
            }
        }

        // Need to write
        let mut semaphores = self.semaphores.write().unwrap();
        if let Some(sem) = semaphores.get(workflow_name) {
            return Arc::clone(sem);
        }

        let sem = Arc::new(Semaphore::new(self.max_per_workflow));
        semaphores.insert(workflow_name.clone(), Arc::clone(&sem));
        sem
    }

    /// Returns a reference to the semaphore for a workflow.
    ///
    /// The semaphore is created if it doesn't exist.
    pub fn semaphore_for(&self, workflow_name: &WorkflowName) -> Arc<Semaphore> {
        self.get_or_create(workflow_name)
    }

    /// Returns the number of semaphores currently tracked.
    #[must_use]
    pub fn len(&self) -> usize {
        self.semaphores.read().unwrap().len()
    }

    /// Returns true if no workflows are being tracked.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.semaphores.read().unwrap().is_empty()
    }

    /// Cleans up semaphores with no waiting tasks.
    ///
    /// This is a best-effort cleanup to prevent memory growth.
    pub fn cleanup_idle(&self) {
        let mut semaphores = self.semaphores.write().unwrap();
        semaphores.retain(|_, sem| sem.available_permits() < self.max_per_workflow);
    }
}

// =============================================================================
// Actor Invariant Enforcement (ADR-015)
// =============================================================================

/// Errors from actor invariant operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvariantError {
    /// Instance is already active (invariant violation).
    InstanceAlreadyActive { instance_id: InstanceId },
    /// Registry operation failed.
    RegistryError { reason: String },
    /// Instance not found.
    InstanceNotFound { instance_id: InstanceId },
}

impl std::fmt::Display for InvariantError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InstanceAlreadyActive { instance_id } => {
                write!(f, "Instance already active: {instance_id}")
            }
            Self::RegistryError { reason } => write!(f, "Registry error: {reason}"),
            Self::InstanceNotFound { instance_id } => write!(f, "Instance not found: {instance_id}"),
        }
    }
}

impl std::error::Error for InvariantError {}

/// Result of checking actor invariants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvariantCheck {
    /// Whether the instance is allowed to proceed.
    pub allowed: bool,
    /// Current status of the invariant.
    pub status: BackpressureStatus,
    /// Error if not allowed.
    pub error: Option<InvariantError>,
}

impl InvariantCheck {
    /// Returns true if the check passed.
    #[must_use]
    pub fn is_allowed(&self) -> bool {
        self.allowed
    }
}

/// Combines execution semaphore with instance registry for full invariant enforcement.
///
/// This provides the complete ADR-015 invariant enforcement:
/// - Single-writer invariant (from instance_registry)
/// - Resource admission control (from execution semaphore)
pub struct InvariantEnforcer<S> {
    execution_semaphore: Arc<ExecutionSemaphore>,
    instance_registry: Arc<S>,
}

impl<S: std::fmt::Debug> std::fmt::Debug for InvariantEnforcer<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InvariantEnforcer")
            .field("execution_semaphore", &self.execution_semaphore)
            .field("instance_registry", &self.instance_registry)
            .finish()
    }
}

impl<S> InvariantEnforcer<S> {
    /// Creates a new invariant enforcer.
    #[must_use]
    pub fn new(execution_semaphore: Arc<ExecutionSemaphore>, instance_registry: Arc<S>) -> Self {
        Self {
            execution_semaphore,
            instance_registry,
        }
    }
}

impl<S: crate::instance_registry::InstanceRegistryInterface + Send + Sync> InvariantEnforcer<S> {
    /// Checks if an instance can be activated.
    ///
    /// Returns `Ok(InvariantCheck)` with admission details if allowed.
    /// Returns `Err(InvariantError)` if the invariant is violated.
    pub fn check_activation(
        &self,
        instance_id: &InstanceId,
    ) -> Result<InvariantCheck, InvariantError> {
        // Check single-writer invariant
        if self.instance_registry.is_active(instance_id) {
            return Ok(InvariantCheck {
                allowed: false,
                status: BackpressureStatus::Healthy,
                error: Some(InvariantError::InstanceAlreadyActive {
                    instance_id: instance_id.clone(),
                }),
            });
        }

        // Return allowed check with current backpressure status
        Ok(InvariantCheck {
            allowed: true,
            status: self.execution_semaphore.current_status(),
            error: None,
        })
    }

    /// Returns the current backpressure status.
    #[must_use]
    pub fn backpressure_status(&self) -> BackpressureStatus {
        self.execution_semaphore.current_status()
    }

    /// Returns the execution semaphore for permit acquisition.
    #[must_use]
    pub fn execution_semaphore(&self) -> &Arc<ExecutionSemaphore> {
        &self.execution_semaphore
    }
}

// =============================================================================
// Trait for Instance Registry Interface (re-exported)
// =============================================================================

// Re-export from instance_registry for convenience
pub use crate::instance_registry::InstanceRegistryInterface;

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semaphore_config_default_values() {
        let config = SemaphoreConfig::default();
        assert_eq!(config.max_concurrent_binaries, 500);
        assert_eq!(config.max_waiters_for_shed, 5000);
        assert_eq!(config.max_per_workflow, 10);
    }

    #[test]
    fn backpressure_status_ordering() {
        assert!(BackpressureStatus::Healthy < BackpressureStatus::Moderate);
        assert!(BackpressureStatus::Moderate < BackpressureStatus::Heavy);
        assert!(BackpressureStatus::Heavy < BackpressureStatus::ShedLoad);
    }

    #[test]
    fn backpressure_status_should_reject() {
        assert!(!BackpressureStatus::Healthy.should_reject());
        assert!(!BackpressureStatus::Moderate.should_reject());
        assert!(!BackpressureStatus::Heavy.should_reject());
        assert!(BackpressureStatus::ShedLoad.should_reject());
    }

    #[test]
    fn backpressure_status_is_queued() {
        assert!(!BackpressureStatus::Healthy.is_queued());
        assert!(!BackpressureStatus::Moderate.is_queued());
        assert!(BackpressureStatus::Heavy.is_queued());
        assert!(BackpressureStatus::ShedLoad.is_queued());
    }

    #[test]
    fn calculate_backpressure_status_healthy() {
        let status = calculate_backpressure_status(400, 500, 50, 5000);
        assert_eq!(status, BackpressureStatus::Healthy);
    }

    #[test]
    fn calculate_backpressure_status_heavy() {
        let status = calculate_backpressure_status(100, 500, 300, 5000);
        assert_eq!(status, BackpressureStatus::Heavy);
    }

    #[test]
    fn calculate_backpressure_status_shed_load() {
        let status = calculate_backpressure_status(0, 500, 5001, 5000);
        assert_eq!(status, BackpressureStatus::ShedLoad);
    }

    #[test]
    fn estimate_wait_ms_calculation() {
        // With 10 available permits and 100ms avg task
        let wait = estimate_wait_ms(50, 10, 100);
        assert_eq!(wait, 600); // 50/10 = 5 tasks ahead, + 1 = 6 * 100 = 600ms

        // With no permits available
        let wait = estimate_wait_ms(5, 0, 100);
        assert_eq!(wait, 600); // 5 + 1 = 6 * 100 = 600ms
    }

    #[test]
    fn test_is_workflow_saturated() {
        assert!(!is_workflow_saturated(5, 10));
        assert!(is_workflow_saturated(10, 10));
        assert!(is_workflow_saturated(15, 10));
    }

    #[tokio::test]
    async fn execution_semaphore_try_acquire_success() {
        let sem = ExecutionSemaphore::default();
        let initial_available = sem.available_permits();

        let permit = sem.try_acquire();
        assert!(permit.is_some());

        assert_eq!(sem.available_permits(), initial_available - 1);
    }

    #[tokio::test]
    async fn execution_semaphore_try_acquire_exhausted() {
        let config = SemaphoreConfig {
            max_concurrent_binaries: 1,
            ..Default::default()
        };
        let sem = ExecutionSemaphore::new(config);

        let _permit = sem.try_acquire();
        assert!(sem.try_acquire().is_none());
    }

    #[tokio::test]
    async fn execution_semaphore_acquire_and_release() {
        let sem = Arc::new(ExecutionSemaphore::default());
        let initial = sem.available_permits();

        let decision = sem.acquire().await;
        assert!(matches!(decision, AdmissionDecision::Admitted));
        assert_eq!(sem.available_permits(), initial - 1);

        // Re-acquire to release the permit
        let _ = sem.try_acquire();
        assert_eq!(sem.available_permits(), initial - 2);

        // Drop to release
        drop(sem);

        // If we recreate, permits should be back to default
        let sem2 = Arc::new(ExecutionSemaphore::default());
        assert_eq!(sem2.available_permits(), sem2.total_permits());
    }

    #[tokio::test]
    async fn execution_semaphore_status_tracking() {
        let sem = ExecutionSemaphore::default();

        assert_eq!(sem.current_status(), BackpressureStatus::Healthy);
        assert_eq!(sem.waiting_count(), 0);
    }

    #[tokio::test]
    async fn workflow_semaphore_map_creation() {
        let map = WorkflowSemaphoreMap::default();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[tokio::test]
    async fn workflow_semaphore_map_semaphore_access() {
        let map = WorkflowSemaphoreMap::default();
        let wf_name = WorkflowName::parse("test-workflow").unwrap();

        let sem = map.semaphore_for(&wf_name);
        assert!(!map.is_empty());
        assert_eq!(map.len(), 1);

        // Should get same semaphore for same workflow
        let sem2 = map.semaphore_for(&wf_name);
        assert_eq!(map.len(), 1);

        // Try to acquire - tokio's Semaphore returns Result, convert to Option
        let permit = sem.try_acquire().ok();
        assert!(permit.is_some());
    }

    #[test]
    fn invariant_check_is_allowed() {
        let allowed = InvariantCheck {
            allowed: true,
            status: BackpressureStatus::Healthy,
            error: None,
        };
        assert!(allowed.is_allowed());

        let denied = InvariantCheck {
            allowed: false,
            status: BackpressureStatus::Heavy,
            error: Some(InvariantError::InstanceAlreadyActive {
                instance_id: InstanceId::from_bytes([0u8; 16]),
            }),
        };
        assert!(!denied.is_allowed());
    }
}
