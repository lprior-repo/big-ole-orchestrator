pub(crate) use std::sync::LazyLock;
pub(crate) use std::sync::Mutex;
pub(crate) use std::sync::MutexGuard;
pub(crate) use std::time::{Duration, Instant};

pub(crate) use vo_executor::{
    cancel_execution, clear_error, execute_step, execute_step_with_retry, get_execution_status,
    get_last_error, reset_all_state, run_subprocess, set_error, ExecuteNodeError, ExecutionStatus,
    RetryPolicy, StepId, StepResult, SubprocessConfig,
};

static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

pub(crate) fn state_guard() -> MutexGuard<'static, ()> {
    let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset_all_state();
    guard
}
