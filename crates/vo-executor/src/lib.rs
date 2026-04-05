//! vel-k1t9: Add error handling and timeout for --execute-node (ADR-012)
//!
//! This crate provides:
//! - `execute_step`: Execute a workflow step with timeout enforcement
//! - `execute_step_with_retry`: Execute with retry policy
//! - `cancel_execution`: Cancel an in-progress execution
//! - `get_execution_status`: Get current execution status
//! - `get_last_error`: Get the last error for a step

pub mod errors;
pub mod execution;
pub mod state;
pub mod types;

// Re-export for convenience
pub use errors::{ExecuteNodeError, RetryPolicyError};
pub use execution::{
    cancel_execution, execute_step, execute_step_with_retry, get_execution_status,
};
pub use state::{clear_error, set_error};
pub use execution::get_last_error;
pub use types::{ExecutionStatus, RetryPolicy, StepId, StepResult};
