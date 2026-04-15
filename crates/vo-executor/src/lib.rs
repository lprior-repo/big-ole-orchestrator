//! vel-k1t9: Add error handling and timeout for --execute-node (ADR-012)
//!
//! This crate provides:
//! - `execute_step`: Execute a workflow step with timeout enforcement
//! - `execute_step_with_retry`: Execute with retry policy
//! - `cancel_execution`: Cancel an in-progress execution
//! - `get_execution_status`: Get current execution status
//! - `get_last_error`: Get the last error for a step
//!
//! ADR-018 (Pipe Deadlocks and I/O Boundaries) is implemented in the `subprocess` module.
//! This module provides async pipe handling using tokio::select for concurrent FD3 write
//! and FD4 read operations, preventing classic Unix pipe deadlocks with large payloads.

pub mod errors;
pub mod execution;
pub mod runtime;
pub mod scheduler;
pub mod state;
pub mod subprocess;
pub mod types;

// Re-export for convenience
pub use errors::{ExecuteNodeError, RetryPolicyError};
pub use execution::{
    cancel_execution, execute_step, execute_step_with_retry, get_execution_status, get_last_error,
};
pub use runtime::{ContextError, Runtime, RuntimeError, StepContext};
pub use scheduler::{Job, JobId, JobPriority, JobResult, Schedule, SchedulerConfig};
pub use state::{clear_error, get_error_count, get_state_count, reset_all_state, set_error};
pub use state::set_executing_state_for_test;
pub use state::{clear_error, get_error_count, get_state_count, reset_all_state, set_error};
pub use subprocess::{run_subprocess, SubprocessConfig, SubprocessError, SubprocessOutput};
pub use types::{ExecutionStatus, RetryPolicy, StepId, StepResult};
