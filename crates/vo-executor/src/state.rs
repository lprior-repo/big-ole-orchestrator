//! Internal state management for vo-executor

use dashmap::DashMap;
use std::sync::LazyLock;
use std::time::Instant;

use crate::errors::ExecuteNodeError;
use crate::types::StepId;

/// Execution state for a step.
#[derive(Debug, Clone)]
pub(crate) enum StepState {
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
/// Steps taking longer than this trigger timeout errors if `timeout_ms` is smaller.
pub(crate) const SLOW_STEP_DURATION_MS: u64 = 3000;

/// Get current state for a step.
pub fn get_state(step_id: &str) -> StepState {
    STATE.get(step_id).map_or(StepState::Ready, |v| v.clone())
}

/// Set state for a step.
pub fn set_state(step_id: &str, state: StepState) {
    STATE.insert(step_id.to_string(), state);
}

/// Clear any stored error for a step.
///pub for testing
pub fn clear_error(step_id: &str) {
    LAST_ERROR.remove(step_id);
}

/// Store an error for a step.
///pub for testing
pub fn set_error(step_id: &str, err: ExecuteNodeError) {
    LAST_ERROR.insert(step_id.to_string(), err);
}

/// **NOTE:** This is test infrastructure that simulates workflow step behavior.
pub fn step_behavior(step_id: &str) -> StepBehavior {
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

#[derive(Debug, Clone, Copy)]
pub enum StepBehavior {
    Success,
    Failure,
    Transient,
    Slow,
    NotFound,
}

/// Get the last error for a step (if any).
pub fn get_last_error(step_id: &str) -> Option<ExecuteNodeError> {
    LAST_ERROR.get(step_id).map(|v| v.clone())
}
