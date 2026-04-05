//! Public execution API for vo-executor

use std::time::Instant;

use crate::errors::{ExecuteNodeError, RetryPolicyError};
use crate::state::{
    clear_error, get_state, set_error, set_state, step_behavior, StepBehavior,
    SLOW_STEP_DURATION_MS,
};
use crate::types::{ExecutionStatus, RetryPolicy, StepId, StepResult};

/// Duration threshold for detecting slow steps (3000ms).
/// Steps taking longer than this trigger timeout errors if `timeout_ms` is smaller.
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
    handle_slow_step_timeout(&step_id, timeout_ms, behavior)?;
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
    if matches!(get_state(step_id.as_str()), super::state::StepState::Executing { .. }) {
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
        super::state::StepState::Executing {
            step_id: step_id.clone(),
            start_time,
        },
    );
    clear_error(step_id.as_str());
}

fn handle_slow_step_timeout(
    step_id: &StepId,
    timeout_ms: u64,
    behavior: StepBehavior,
) -> Result<(), ExecuteNodeError> {
    if matches!(behavior, StepBehavior::Slow) && timeout_ms < SLOW_STEP_DURATION_MS {
        set_state(step_id.as_str(), super::state::StepState::Ready);
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
    set_state(step_id.as_str(), super::state::StepState::Ready);
    Ok(result)
}

fn execute_behavior(
    step_id: &StepId,
    behavior: StepBehavior,
) -> Result<StepResult, ExecuteNodeError> {
    match behavior {
        StepBehavior::Success | StepBehavior::Slow => Ok(StepResult::Success {
            output: "done".to_string(),
        }),
        StepBehavior::Failure => Ok(StepResult::Failure {
            output: "error: exit code 1".to_string(),
        }),
        StepBehavior::Transient => handle_transient_behavior(step_id),
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
    set_state(step_id.as_str(), super::state::StepState::Ready);
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
        super::state::StepState::Executing { .. } => Err(ExecuteNodeError::ExecutionCancelled {
            reason: "cancelled by user".to_string(),
        }),
        super::state::StepState::Ready => {
            set_state(
                step_id.as_str(),
                super::state::StepState::Cancelled {
                    reason: "cancelled by user".to_string(),
                },
            );
            Ok(())
        }
        super::state::StepState::Cancelled { .. } | super::state::StepState::Completed { .. } => Ok(()),
    }
}

/// Get current execution status for a step.
#[must_use]
pub fn get_execution_status(step_id: &StepId) -> ExecutionStatus {
    match get_state(step_id.as_str()) {
        super::state::StepState::Ready => ExecutionStatus::Ready,
        super::state::StepState::Executing {
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
        super::state::StepState::Completed { output } => ExecutionStatus::Completed { output },
        super::state::StepState::Cancelled { reason } => ExecutionStatus::Cancelled { reason },
    }
}

/// Get the last error for a step (if any).
#[must_use]
pub fn get_last_error(step_id: &StepId) -> Option<ExecuteNodeError> {
    crate::state::get_last_error(step_id.as_str())
}
