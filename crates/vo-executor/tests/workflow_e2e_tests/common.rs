pub mod prelude {
    pub use std::sync::LazyLock;
    pub use std::sync::Mutex;
    pub use std::sync::MutexGuard;
    pub use vo_executor::{
        cancel_execution, execute_step, execute_step_with_retry, get_execution_status,
        get_last_error, reset_all_state, ExecuteNodeError, ExecutionStatus, Job, JobId, JobPriority,
        RetryPolicy, Schedule, Scheduler, SchedulerConfig, StepContext, StepId, StepResult,
    };

    static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    pub fn state_guard() -> MutexGuard<'static, ()> {
        let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_all_state();
        guard
    }
}
