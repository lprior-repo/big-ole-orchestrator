//! ADR-003: Raw Binary Execution via OS Subprocesses
//!
//! This crate provides the core execution engine for veloxide workflows.
//! Workflows are compiled Rust binaries discovered via `--graph` and executed
//! via `--execute-node`, using `tokio::process::Command` as the execution boundary.
//!
//! # ADR-003 Step Classes
//!
//! - **Pure**: Child reads input, performs deterministic computation, returns output.
//! - **ManagedEffect**: Child returns `EffectIntent` for engine-side commit.
//! - **Wait / Signal**: Workflow suspends/resumes — no subprocess execution.
//! - **Unsafe**: Child may perform arbitrary external side effects (at-least-once only).
//!
//! # Key Components
//!
//! - `dispatch_node`: Route execution based on `NodeKind` (ADR-003)
//! - `execute_step` / `execute_step_with_retry`: Public execution API with timeout/retry
//! - `run_subprocess`: ADR-018 async pipe deadlock prevention
//! - `pin_binary` / `resolve_binary_path`: ADR-017 version pinning
//!
//! # IPC Contract
//!
//! Engine sends `Fd3Envelope` over fd3, child responds with `Fd4Envelope` over fd4.
//! Stderr is captured with bounded truncation (ADR-023).
//! All I/O is async via tokio to prevent pipe deadlocks (ADR-018).

pub mod dispatch;
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
pub use scheduler::{
    Job, JobId, JobKind, JobPriority, JobResult, JobState, SchedulePolicy, ScheduledJob,
    SchedulerConfig, SchedulerError, SchedulerRetryPolicy, SerializedPayload,
};
pub use state::set_executing_state_for_test;
pub use state::{clear_error, get_error_count, get_state_count, reset_all_state, set_error};
pub use subprocess::{
    pin_binary, resolve_binary_path, run_subprocess, run_subprocess_with_graceful_timeout,
    PinnedBinary, SubprocessConfig, SubprocessError, SubprocessOutput, VERSION_BASE_PATH,
    BOUNDED_READ_BUFFER_SIZE, DEFAULT_GRACE_PERIOD_MS, MAX_STEP_INPUT_BYTES, MAX_STEP_OUTPUT_BYTES,
};
pub use dispatch::{dispatch_node, NodeDispatchResult};
pub use types::{ExecutionStatus, RetryPolicy, StepId, StepResult};
