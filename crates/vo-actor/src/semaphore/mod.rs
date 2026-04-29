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

pub mod calc;
pub mod enforcer;
pub mod execution;
pub mod types;
pub mod workflow;

pub use calc::{calculate_backpressure_status, estimate_wait_ms, is_workflow_saturated};
pub use enforcer::{InvariantCheck, InvariantEnforcer, InvariantError};
pub use execution::{ExecutionSemaphore, PermitGuard};
pub use types::{
    AdmissionDecision, BackpressureStatus, RejectionReason, SemaphoreConfig,
    DEFAULT_MAX_CONCURRENT_BINARIES, DEFAULT_MAX_PER_WORKFLOW, DEFAULT_MAX_WAITERS_FOR_SHED,
};
pub use workflow::WorkflowSemaphoreMap;
