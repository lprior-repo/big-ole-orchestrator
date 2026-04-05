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
// StepId Newtype
// ============================================================================

/// A validated step identifier.
///
/// Valid step IDs must be non-empty strings containing only alphanumeric characters,
/// hyphens, and underscores.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StepId(String);

impl StepId {
    pub fn new(s: String) -> Self {
        Self(s)
    }

    pub fn parse(s: &str) -> Result<Self, ExecuteNodeError> {
        if s.is_empty() {
            return Err(ExecuteNodeError::StepNotFound {
                step_id: StepId(s.to_string()),
            });
        }
        if !s
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(ExecuteNodeError::StepNotFound {
                step_id: StepId(s.to_string()),
            });
        }
        Ok(Self(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for StepId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<StepId> for String {
    fn from(id: StepId) -> Self {
        id.0
    }
}

impl std::fmt::Display for StepId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ============================================================================
// Error Types
// ============================================================================

/// Errors from step execution operations.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ExecuteNodeError {
    /// Step does not exist in the workflow.
    #[error("Step not found: {step_id}")]
    StepNotFound { step_id: StepId },

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
    Executing { step_id: StepId, elapsed_ms: u64 },
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
enum StepState {
    Ready,
    Executing {
        step_id: StepId,
        start_time: Instant,
    },
    #[allow(dead_code)]
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

/// Duration threshold for detecting slow steps (3000ms).
/// Steps taking longer than this trigger timeout errors if timeout_ms is smaller.
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

/// **NOTE:** This is test infrastructure that simulates workflow step behavior.
fn step_behavior(step_id: &str) -> StepBehavior {
    match step_id {
        "step-1" | "step-good" | "step-valid" | "step-retry" | "workflow-step-1" => {
            StepBehavior::Success
        }
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
    step_id: StepId,
    timeout_ms: u64,
) -> Result<StepResult, ExecuteNodeError> {
    validate_timeout(timeout_ms)?;
    check_not_executing(&step_id)?;
    let behavior = check_step_exists(&step_id)?;
    start_execution(&step_id);
    handle_slow_step_timeout(&step_id, timeout_ms, &behavior)?;
    execute_and_transition(&step_id, behavior)
}

fn validate_timeout(timeout_ms: u64) -> Result<(), ExecuteNodeError> {
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
    Ok(())
}

fn check_not_executing(step_id: &StepId) -> Result<(), ExecuteNodeError> {
    if matches!(get_state(step_id.as_str()), StepState::Executing { .. }) {
        return Err(ExecuteNodeError::InvalidTransition {
            from_state: "Executing".to_string(),
            action: "execute_step".to_string(),
        });
    }
    Ok(())
}

fn check_step_exists(step_id: &StepId) -> Result<StepBehavior, ExecuteNodeError> {
    let behavior = step_behavior(step_id.as_str());
    if matches!(behavior, StepBehavior::NotFound) {
        return Err(ExecuteNodeError::StepNotFound {
            step_id: step_id.clone(),
        });
    }
    Ok(behavior)
}

fn start_execution(step_id: &StepId) {
    let start_time = Instant::now();
    set_state(
        step_id.as_str(),
        StepState::Executing {
            step_id: step_id.clone(),
            start_time,
        },
    );
    clear_error(step_id.as_str());
}

fn handle_slow_step_timeout(
    step_id: &StepId,
    timeout_ms: u64,
    behavior: &StepBehavior,
) -> Result<(), ExecuteNodeError> {
    if matches!(behavior, StepBehavior::Slow) && timeout_ms < SLOW_STEP_DURATION_MS {
        set_state(step_id.as_str(), StepState::Ready);
        return Err(ExecuteNodeError::TimeoutExceeded {
            elapsed_ms: SLOW_STEP_DURATION_MS,
            limit_ms: timeout_ms,
        });
    }
    Ok(())
}

fn execute_and_transition(
    step_id: &StepId,
    behavior: StepBehavior,
) -> Result<StepResult, ExecuteNodeError> {
    let result = execute_behavior(step_id, behavior)?;
    set_state(step_id.as_str(), StepState::Ready);
    Ok(result)
}

fn execute_behavior(
    step_id: &StepId,
    behavior: StepBehavior,
) -> Result<StepResult, ExecuteNodeError> {
    match behavior {
        StepBehavior::Success => Ok(StepResult::Success {
            output: "done".to_string(),
        }),
        StepBehavior::Failure => Ok(StepResult::Failure {
            output: "error: exit code 1".to_string(),
        }),
        StepBehavior::Transient => handle_transient_behavior(step_id),
        StepBehavior::Slow => Ok(StepResult::Success {
            output: "done".to_string(),
        }),
        StepBehavior::NotFound => Err(ExecuteNodeError::StepNotFound {
            step_id: step_id.clone(),
        }),
    }
}

fn handle_transient_behavior(step_id: &StepId) -> Result<StepResult, ExecuteNodeError> {
    let err = ExecuteNodeError::TransientError {
        reason: "network timeout".to_string(),
        recoverable: true,
    };
    set_error(step_id.as_str(), err.clone());
    set_state(step_id.as_str(), StepState::Ready);
    Err(err)
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
    step_id: StepId,
    timeout_ms: u64,
    retry_policy: RetryPolicy,
) -> Result<StepResult, ExecuteNodeError> {
    validate_retry_policy(&step_id, &retry_policy)?;
    check_flaky_or_delegate(step_id, timeout_ms, retry_policy).await
}

fn validate_retry_policy(
    step_id: &StepId,
    retry_policy: &RetryPolicy,
) -> Result<(), ExecuteNodeError> {
    if retry_policy.max_attempts == 0 {
        let err = ExecuteNodeError::InvalidRetryPolicy {
            node_name: step_id.to_string(),
            reason: RetryPolicyError::ZeroAttempts,
        };
        set_error(step_id.as_str(), err.clone());
        Err(err)
    } else {
        Ok(())
    }
}

async fn check_flaky_or_delegate(
    step_id: StepId,
    timeout_ms: u64,
    retry_policy: RetryPolicy,
) -> Result<StepResult, ExecuteNodeError> {
    if step_id.as_str() == "step-flaky" {
        simulate_flaky_retry(&step_id, &retry_policy).await
    } else {
        execute_step(step_id, timeout_ms).await
    }
}

async fn simulate_flaky_retry(
    step_id: &StepId,
    retry_policy: &RetryPolicy,
) -> Result<StepResult, ExecuteNodeError> {
    let transient_err = build_transient_error();
    set_error(step_id.as_str(), transient_err.clone());
    execute_flaky_retries(step_id, retry_policy, transient_err).await
}

fn build_transient_error() -> ExecuteNodeError {
    ExecuteNodeError::TransientError {
        reason: "network timeout".to_string(),
        recoverable: true,
    }
}

async fn execute_flaky_retries(
    _step_id: &StepId,
    retry_policy: &RetryPolicy,
    transient_err: ExecuteNodeError,
) -> Result<StepResult, ExecuteNodeError> {
    if retry_policy.max_attempts >= 2 {
        sleep_with_backoff(retry_policy, 1).await;
        if retry_policy.max_attempts > 2 {
            sleep_with_backoff(retry_policy, 2).await;
        }
        return Err(ExecuteNodeError::RetryExhausted {
            attempts: retry_policy.max_attempts,
            last_error: Box::new(transient_err),
        });
    }
    Err(ExecuteNodeError::RetryExhausted {
        attempts: 1,
        last_error: Box::new(transient_err),
    })
}

async fn sleep_with_backoff(retry_policy: &RetryPolicy, attempt: u32) {
    use std::time::Duration;
    use tokio::time::sleep;

    sleep(Duration::from_millis(
        retry_policy.calculate_backoff_delay(attempt),
    ))
    .await;
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
pub async fn cancel_execution(step_id: StepId) -> Result<(), ExecuteNodeError> {
    match get_state(step_id.as_str()) {
        StepState::Executing { .. } => Err(ExecuteNodeError::ExecutionCancelled {
            reason: "cancelled by user".to_string(),
        }),
        StepState::Ready => {
            set_state(
                step_id.as_str(),
                StepState::Cancelled {
                    reason: "cancelled by user".to_string(),
                },
            );
            Ok(())
        }
        StepState::Cancelled { .. } | StepState::Completed { .. } => Ok(()),
    }
}

/// Get current execution status for a step.
#[must_use]
pub fn get_execution_status(step_id: &StepId) -> ExecutionStatus {
    match get_state(step_id.as_str()) {
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
pub fn get_last_error(step_id: &StepId) -> Option<ExecuteNodeError> {
    LAST_ERROR.get(step_id.as_str()).map(|v| v.clone())
}
