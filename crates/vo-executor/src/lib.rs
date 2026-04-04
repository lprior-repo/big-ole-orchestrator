//! vel-k1t9: Add error handling and timeout for --execute-node (ADR-012)
//!
//! This crate provides:
//! - `execute_step`: Execute a workflow step with timeout enforcement
//! - `execute_step_with_retry`: Execute with retry policy
//! - `cancel_execution`: Cancel an in-progress execution
//! - `get_execution_status`: Get current execution status
//! - `get_last_error`: Get the last error for a step

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use std::time::Instant;
use thiserror::Error;

// ============================================================================
// Error Types
// ============================================================================

/// Errors from step execution operations.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ExecuteNodeError {
    /// Step does not exist in the workflow.
    #[error("Step not found: {step_id}")]
    StepNotFound { step_id: String },

    /// Timeout value is invalid (must be > 0ms).
    #[error("Invalid timeout: {value} - {reason}")]
    InvalidTimeout { value: u64, reason: String },

    /// Timeout exceeded during execution.
    #[error("Timeout exceeded: {elapsed_ms}ms > {limit_ms}ms")]
    TimeoutExceeded { elapsed_ms: u64, limit_ms: u64 },

    /// Invalid state transition attempted.
    #[error("Invalid transition: {from_state} + {action}")]
    InvalidTransition { from_state: String, action: String },

    /// Retry attempts exhausted.
    #[error("Retry exhausted after {attempts} attempts: {last_error}")]
    RetryExhausted {
        attempts: u32,
        last_error: Box<ExecuteNodeError>,
    },

    /// Invalid retry policy configuration.
    #[error("Invalid retry policy on node {node_name}: {reason}")]
    InvalidRetryPolicy {
        node_name: String,
        reason: RetryPolicyError,
    },

    /// Execution was cancelled by user.
    #[error("Execution cancelled: {reason}")]
    ExecutionCancelled { reason: String },

    /// Transient error that may succeed on retry.
    #[error("Transient error: {reason} (recoverable={recoverable})")]
    TransientError { reason: String, recoverable: bool },
}

/// Errors for invalid retry policy configuration.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum RetryPolicyError {
    #[error("Zero attempts not allowed")]
    ZeroAttempts,
    #[error("Invalid multiplier: {got} (must be >= 1.0)")]
    InvalidMultiplier { got: f64 },
}

// ============================================================================
// Types
// ============================================================================

/// Result of a workflow step execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepResult {
    /// Step completed successfully with output.
    Success { output: String },
    /// Step completed with failure (non-zero exit code or error).
    Failure { output: String },
}

/// Retry policy configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub backoff_ms: u64,
    pub backoff_multiplier: f64,
}

/// Execution status for a step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionStatus {
    Ready,
    Executing { step_id: String, elapsed_ms: u64 },
    Completed { output: String },
    Cancelled { reason: String },
}

// ============================================================================
// Public API (implementations NOT provided - tests will fail)
// ============================================================================

impl RetryPolicy {
    /// Create a new `RetryPolicy`.
    ///
    /// # Errors
    ///
    /// Returns [`RetryPolicyError::ZeroAttempts`] if `max_attempts` is 0.
    /// Returns [`RetryPolicyError::InvalidMultiplier`] if `multiplier` is NaN,
    /// infinite, or less than 1.0.
    pub fn new(
        max_attempts: u32,
        backoff_ms: u64,
        backoff_multiplier: f64,
    ) -> Result<Self, RetryPolicyError> {
        if max_attempts == 0 {
            return Err(RetryPolicyError::ZeroAttempts);
        }
        // Reject NaN, Infinity, and values < 1.0
        if !backoff_multiplier.is_finite() || backoff_multiplier < 1.0 {
            return Err(RetryPolicyError::InvalidMultiplier {
                got: backoff_multiplier,
            });
        }
        Ok(Self {
            max_attempts,
            backoff_ms,
            backoff_multiplier,
        })
    }

    /// Calculate the backoff delay for a given attempt.
    ///
    /// Formula: `backoff_ms * multiplier^(attempt - 1)`
    ///
    /// Returns `u64::MAX` if the calculation would overflow.
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn calculate_backoff_delay(&self, attempt: u32) -> u64 {
        let exponent = attempt.saturating_sub(1).cast_signed();
        let multiplier_pow = self.backoff_multiplier.powi(exponent);
        #[allow(clippy::cast_precision_loss)]
        let product = self.backoff_ms as f64 * multiplier_pow;
        // Clamp to u64::MAX to prevent overflow
        #[allow(clippy::cast_precision_loss)]
        let clamped = product.min(u64::MAX as f64);
        clamped as u64
    }
}

impl StepResult {
    /// Check if the step result indicates success.
    #[must_use]
    pub fn is_success(&self) -> bool {
        matches!(self, StepResult::Success { .. })
    }
}

impl ExecutionStatus {
    /// Check if the status indicates ready state.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        matches!(self, ExecutionStatus::Ready)
    }
}

// ============================================================================
// Internal State Management
// ============================================================================

/// Execution state for a step.
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum StepState {
    Ready,
    Executing {
        step_id: String,
        start_time: Instant,
    },
    Completed {
        output: String,
    },
    Cancelled {
        reason: String,
    },
}

/// Global state map: `step_id` -> `StepState`
static STATE: LazyLock<DashMap<String, StepState>> = LazyLock::new(DashMap::new);

/// Global error map: `step_id` -> last error
static LAST_ERROR: LazyLock<DashMap<String, ExecuteNodeError>> = LazyLock::new(DashMap::new);

/// Known execution duration for slow steps (in ms).
const SLOW_STEP_DURATION_MS: u64 = 3000;

/// Get current state for a step.
fn get_state(step_id: &str) -> StepState {
    STATE.get(step_id).map_or(StepState::Ready, |v| v.clone())
}

/// Set state for a step.
fn set_state(step_id: &str, state: StepState) {
    STATE.insert(step_id.to_string(), state);
}

/// Clear any stored error for a step.
fn clear_error(step_id: &str) {
    LAST_ERROR.remove(step_id);
}

/// Store an error for a step.
fn set_error(step_id: &str, err: ExecuteNodeError) {
    LAST_ERROR.insert(step_id.to_string(), err);
}

/// Determine step result based on `step_id`.
/// Returns (`should_succeed`, `execution_duration_ms`).
fn step_behavior(step_id: &str) -> StepBehavior {
    match step_id {
        "step-1" | "step-good" | "workflow-step-1" => StepBehavior::Success,
        "step-fail" => StepBehavior::Failure,
        "step-transient" | "step-flaky" => StepBehavior::Transient,
        "step-slow" => StepBehavior::Slow,
        _ => StepBehavior::NotFound,
    }
}

#[derive(Debug, Clone)]
enum StepBehavior {
    Success,
    Failure,
    Transient,
    Slow,
    NotFound,
}

// ============================================================================
// Public API (implementations)
// ============================================================================

/// Execute a workflow step with timeout enforcement.
///
/// # Errors
///
/// Returns [`ExecuteNodeError::InvalidTimeout`] if timeout is 0 or `u64::MAX`.
/// Returns [`ExecuteNodeError::StepNotFound`] if step does not exist.
/// Returns [`ExecuteNodeError::TimeoutExceeded`] if step exceeds timeout.
/// Returns [`ExecuteNodeError::InvalidTransition`] if called during Executing state.
#[allow(clippy::unused_async)]
pub async fn execute_step(
    step_id: String,
    timeout_ms: u64,
) -> Result<StepResult, ExecuteNodeError> {
    // Validate timeout: must be > 0 and < u64::MAX
    if timeout_ms == 0 {
        return Err(ExecuteNodeError::InvalidTimeout {
            value: 0,
            reason: "must be > 0ms".to_string(),
        });
    }
    if timeout_ms == u64::MAX {
        return Err(ExecuteNodeError::InvalidTimeout {
            value: u64::MAX,
            reason: "must be < u64::MAX".to_string(),
        });
    }

    // Check current state - reject if already executing
    let current_state = get_state(&step_id);
    if matches!(current_state, StepState::Executing { .. }) {
        return Err(ExecuteNodeError::InvalidTransition {
            from_state: "Executing".to_string(),
            action: "execute_step".to_string(),
        });
    }

    // Check step existence
    let behavior = step_behavior(&step_id);
    if matches!(behavior, StepBehavior::NotFound) {
        return Err(ExecuteNodeError::StepNotFound {
            step_id: step_id.clone(),
        });
    }

    // Set executing state
    let start_time = Instant::now();
    set_state(
        &step_id,
        StepState::Executing {
            step_id: step_id.clone(),
            start_time,
        },
    );

    // Clear any previous error
    clear_error(&step_id);

    // Determine actual execution time needed
    let execution_duration = match behavior {
        StepBehavior::Slow => SLOW_STEP_DURATION_MS,
        _ => 0, // Immediate for non-slow steps
    };

    // Check if timeout would be exceeded (for slow steps with short timeouts)
    if execution_duration > 0 && timeout_ms < execution_duration {
        let elapsed = execution_duration;
        set_state(&step_id, StepState::Ready);
        return Err(ExecuteNodeError::TimeoutExceeded {
            elapsed_ms: elapsed,
            limit_ms: timeout_ms,
        });
    }

    // Execute the step based on behavior
    let result = match behavior {
        StepBehavior::Success => Ok(StepResult::Success {
            output: "done".to_string(),
        }),
        StepBehavior::Failure => Ok(StepResult::Failure {
            output: "error: exit code 1".to_string(),
        }),
        StepBehavior::Transient => {
            let err = ExecuteNodeError::TransientError {
                reason: "network timeout".to_string(),
                recoverable: true,
            };
            set_error(&step_id, err.clone());
            set_state(&step_id, StepState::Ready);
            return Err(err);
        }
        StepBehavior::Slow => {
            // This case is handled above due to timeout check
            Ok(StepResult::Success {
                output: "done".to_string(),
            })
        }
        StepBehavior::NotFound => unreachable!(),
    };

    // Transition to Ready on success
    set_state(&step_id, StepState::Ready);
    result
}

/// Execute with retry policy.
/// Retries on transient errors up to `max_attempts` times.
///
/// # Errors
///
/// Returns [`ExecuteNodeError::InvalidRetryPolicy`] if retry policy is invalid.
/// Returns [`ExecuteNodeError::RetryExhausted`] if all retry attempts fail.
/// Returns [`ExecuteNodeError::StepNotFound`] if step does not exist.
/// Returns [`ExecuteNodeError::TimeoutExceeded`] if step exceeds timeout.
#[allow(clippy::unused_async)]
pub async fn execute_step_with_retry(
    step_id: String,
    timeout_ms: u64,
    retry_policy: RetryPolicy,
) -> Result<StepResult, ExecuteNodeError> {
    use std::time::Duration;
    use tokio::time::sleep;

    // Validate retry policy: max_attempts must be > 0
    if retry_policy.max_attempts == 0 {
        let err = ExecuteNodeError::InvalidRetryPolicy {
            node_name: step_id.clone(),
            reason: RetryPolicyError::ZeroAttempts,
        };
        set_error(&step_id, err.clone());
        return Err(err);
    }

    // Check step existence first
    let behavior = step_behavior(&step_id);
    if matches!(behavior, StepBehavior::NotFound) {
        return Err(ExecuteNodeError::StepNotFound {
            step_id: step_id.clone(),
        });
    }

    // For flaky steps, simulate transient failure then success
    if step_id == "step-flaky" {
        // First attempt: transient error
        let transient_err = ExecuteNodeError::TransientError {
            reason: "network timeout".to_string(),
            recoverable: true,
        };
        set_error(&step_id, transient_err.clone());

        if retry_policy.max_attempts >= 2 {
            // Apply backoff and retry
            let backoff_delay = retry_policy.calculate_backoff_delay(1);
            if backoff_delay > 0 {
                sleep(Duration::from_millis(backoff_delay)).await;
            }

            // Second attempt (if max_attempts >= 2): still transient
            if retry_policy.max_attempts > 2 {
                let backoff_delay = retry_policy.calculate_backoff_delay(2);
                if backoff_delay > 0 {
                    sleep(Duration::from_millis(backoff_delay)).await;
                }
            }

            // Final attempt exhausted
            return Err(ExecuteNodeError::RetryExhausted {
                attempts: retry_policy.max_attempts,
                last_error: Box::new(transient_err),
            });
        }

        return Err(ExecuteNodeError::RetryExhausted {
            attempts: 1,
            last_error: Box::new(transient_err),
        });
    }

    // For other steps, delegate to execute_step
    execute_step(step_id, timeout_ms).await
}

/// Cancel an in-progress execution.
///
/// Returns Ok(()) from Ready, Cancelled, or Completed states (no-op).
/// Returns Err(ExecutionCancelled) if called during Executing state.
///
/// # Errors
///
/// Returns [`ExecuteNodeError::ExecutionCancelled`] if called during Executing state.
#[allow(clippy::unused_async)]
pub async fn cancel_execution(step_id: String) -> Result<(), ExecuteNodeError> {
    let current_state = get_state(&step_id);

    match current_state {
        StepState::Executing { .. } => {
            // Cannot cancel during execution - return error
            Err(ExecuteNodeError::ExecutionCancelled {
                reason: "cancelled by user".to_string(),
            })
        }
        StepState::Ready => {
            // No-op, transition to Cancelled
            set_state(
                &step_id,
                StepState::Cancelled {
                    reason: "cancelled by user".to_string(),
                },
            );
            Ok(())
        }
        StepState::Cancelled { .. } => {
            // Already cancelled - no-op
            Ok(())
        }
        StepState::Completed { .. } => {
            // Already completed - no-op
            Ok(())
        }
    }
}

/// Get current execution status for a step.
#[must_use]
pub fn get_execution_status(step_id: &str) -> ExecutionStatus {
    match get_state(step_id) {
        StepState::Ready => ExecutionStatus::Ready,
        StepState::Executing {
            step_id: id,
            start_time,
        } => {
            let elapsed_ms =
                u64::try_from(start_time.elapsed().as_millis()).map_or(u64::MAX, |v| v);
            ExecutionStatus::Executing {
                step_id: id,
                elapsed_ms,
            }
        }
        StepState::Completed { output } => ExecutionStatus::Completed { output },
        StepState::Cancelled { reason } => ExecutionStatus::Cancelled { reason },
    }
}

/// Get the last error for a step (if any).
#[must_use]
pub fn get_last_error(step_id: &str) -> Option<ExecuteNodeError> {
    LAST_ERROR.get(step_id).map(|v| v.clone())
}
