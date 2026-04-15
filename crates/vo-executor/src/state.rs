//! Internal state management for vo-executor

use dashmap::DashMap;
use std::sync::LazyLock;
use std::time::Instant;

use crate::errors::ExecuteNodeError;
use crate::types::StepId;

/// Execution state for a step.
#[derive(Debug, Clone)]
pub enum StepState {
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

#[allow(clippy::unwrap_used)]
pub fn set_executing_state_for_test(step_id: &str) {
    set_state(
        step_id,
        StepState::Executing {
            step_id: StepId::new(step_id.to_string()),
            start_time: Instant::now(),
        },
    );
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
    if step_id.starts_with("step-") && step_id[5..].chars().all(|c| c.is_ascii_digit()) {
        return StepBehavior::Success;
    }
    if step_id.starts_with("workflow-step-") && step_id[14..].chars().all(|c| c.is_ascii_digit()) {
        return StepBehavior::Success;
    }
    if step_id.starts_with("leak-step-") && step_id[10..].chars().all(|c| c.is_ascii_digit()) {
        return StepBehavior::Success;
    }
    if step_id.starts_with("sustained-") && step_id[10..].contains('-') {
        let suffix = &step_id[step_id.rfind('-').map_or(10, |p| p + 1)..];
        if suffix.chars().all(|c| c.is_ascii_digit()) {
            return StepBehavior::Success;
        }
    }
    if step_id.starts_with("concurrent-leak-") && step_id[16..].chars().all(|c| c.is_ascii_digit())
    {
        return StepBehavior::Success;
    }
    if step_id.starts_with("warm-") && step_id[5..].chars().all(|c| c.is_ascii_digit() || c == '-')
    {
        return StepBehavior::Success;
    }
    if step_id.starts_with("bench-state-read-") && step_id[17..].chars().all(|c| c.is_ascii_digit())
    {
        return StepBehavior::Success;
    }
    if step_id.starts_with("growth-")
        && step_id[7..].chars().all(|c| c.is_ascii_digit() || c == '-')
    {
        return StepBehavior::Success;
    }
    if step_id.starts_with("cold-start-")
        && step_id[11..]
            .chars()
            .all(|c| c.is_ascii_digit() || c == '-')
    {
        return StepBehavior::Success;
    }
    if step_id.starts_with("error-") && step_id[6..].chars().all(|c| c.is_ascii_digit() || c == '-')
    {
        return StepBehavior::Success;
    }
    if step_id.starts_with("batch-") && step_id[6..].chars().all(|c| c.is_ascii_digit() || c == '-')
    {
        return StepBehavior::Success;
    }
    if step_id.starts_with("mixed-") && step_id[6..].chars().all(|c| c.is_ascii_digit() || c == '-')
    {
        return StepBehavior::Success;
    }
    if step_id.starts_with("retry-") && step_id[6..].chars().all(|c| c.is_ascii_digit() || c == '-')
    {
        return StepBehavior::Success;
    }
    if step_id.starts_with("write-") && step_id[6..].chars().all(|c| c.is_ascii_digit() || c == '-')
    {
        return StepBehavior::Success;
    }
    if step_id.starts_with("read-") && step_id[5..].chars().all(|c| c.is_ascii_digit()) {
        return StepBehavior::Success;
    }
    if step_id.starts_with("transient-step-") && step_id[15..].chars().all(|c| c.is_ascii_digit()) {
        return StepBehavior::Transient;
    }
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

/// Reset all global state (STATE and LAST_ERROR DashMaps).
pub fn reset_all_state() {
    STATE.clear();
    LAST_ERROR.clear();
}



/// Get the current count of entries in the STATE map.
/// Useful for detecting memory leaks under sustained load.
pub fn get_state_count() -> usize {
    STATE.len()
}

/// Get the current count of entries in the LAST_ERROR map.
pub fn get_error_count() -> usize {
    LAST_ERROR.len()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::errors::ExecuteNodeError;
    use std::sync::LazyLock;
    use std::sync::Mutex;
    use std::sync::MutexGuard;

    static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    fn setup() -> MutexGuard<'static, ()> {
        let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_all_state();
        guard
    }

    #[test]
    fn reset_all_state_clears_everything() {
        let _guard = setup();
        set_state(
            "test-reset-a",
            StepState::Completed {
                output: "x".to_string(),
            },
        );
        set_error(
            "test-reset-a",
            ExecuteNodeError::ExecutionCancelled {
                reason: "r".to_string(),
            },
        );

        reset_all_state();

        assert!(matches!(get_state("test-reset-a"), StepState::Ready));
        assert!(get_last_error("test-reset-a").is_none());
    }

    #[test]
    fn set_and_get_state() {
        let _guard = setup();
        set_state(
            "step-a",
            StepState::Completed {
                output: "result".to_string(),
            },
        );
        let state = get_state("step-a");
        assert!(matches!(state, StepState::Completed { output } if output == "result"));
    }

    #[test]
    fn set_state_overwrites() {
        let _guard = setup();
        set_state("step-a", StepState::Ready);
        set_state(
            "step-a",
            StepState::Cancelled {
                reason: "test".to_string(),
            },
        );
        let state = get_state("step-a");
        assert!(matches!(state, StepState::Cancelled { .. }));
    }

    #[test]
    fn executing_state() {
        let _guard = setup();
        let key = "test-exec-state-unique";
        let start = Instant::now();
        set_state(
            key,
            StepState::Executing {
                step_id: StepId::new(key.to_string()),
                start_time: start,
            },
        );
        let state = get_state(key);
        assert!(matches!(state, StepState::Executing { .. }));
    }

    #[test]
    fn clear_error_no_error() {
        let _guard = setup();
        clear_error("step-a");
        assert!(get_last_error("step-a").is_none());
    }

    #[test]
    fn set_and_get_error() {
        let _guard = setup();
        let key = "test-err-unique";
        let err = ExecuteNodeError::TimeoutExceeded {
            elapsed_ms: 5000,
            limit_ms: 3000,
        };
        set_error(key, err.clone());
        let retrieved = get_last_error(key);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), err);
    }

    #[test]
    fn clear_error_removes_existing() {
        let _guard = setup();
        let key = "test-clear-err-unique";
        let err = ExecuteNodeError::ExecutionCancelled {
            reason: "test".to_string(),
        };
        set_error(key, err);
        assert!(get_last_error(key).is_some());
        clear_error(key);
        assert!(get_last_error(key).is_none());
    }

    #[test]
    fn step_behavior_success_variants() {
        let success_steps = [
            "step-1",
            "step-good",
            "step-valid",
            "step-retry",
            "workflow-step-1",
        ];
        for step in success_steps {
            assert!(
                matches!(step_behavior(step), StepBehavior::Success),
                "failed for {}",
                step
            );
        }
    }

    #[test]
    fn step_behavior_failure() {
        assert!(matches!(step_behavior("step-fail"), StepBehavior::Failure));
    }

    #[test]
    fn step_behavior_transient() {
        assert!(matches!(
            step_behavior("step-transient"),
            StepBehavior::Transient
        ));
        assert!(matches!(
            step_behavior("step-flaky"),
            StepBehavior::Transient
        ));
    }

    #[test]
    fn step_behavior_slow() {
        assert!(matches!(step_behavior("step-slow"), StepBehavior::Slow));
    }

    #[test]
    fn step_behavior_not_found() {
        assert!(matches!(step_behavior("unknown"), StepBehavior::NotFound));
        assert!(matches!(step_behavior(""), StepBehavior::NotFound));
        assert!(matches!(step_behavior("STEP-1"), StepBehavior::NotFound));
    }

    #[test]
    fn step_behavior_is_copy() {
        let b = step_behavior("step-1");
        let _b2 = b;
    }

    #[test]
    fn step_state_clone() {
        let state = StepState::Completed {
            output: "data".to_string(),
        };
        let cloned = state.clone();
        assert!(matches!(cloned, StepState::Completed { .. }));
    }
}
