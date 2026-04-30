//! Current-thread SDK runtime for vo-executor
//!
//! Provides an ultra-lightweight single-threaded async runtime for executing
//! workflow steps without the cold-start latency of a full Tokio multi-threaded runtime.
//! See ADR-011 for details.

use crate::errors::ExecuteNodeError;
use crate::types::{ExecutionStatus, RetryPolicy, StepId, StepResult};
use std::mem;
use std::sync::LazyLock;
use tokio::runtime::Handle;

static BLOCKING_RT_HANDLE: LazyLock<Handle> = LazyLock::new(|| {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to build blocking runtime");
    let handle = rt.handle().clone();
    mem::forget(rt);
    handle
});

fn blocking_handle() -> Handle {
    BLOCKING_RT_HANDLE.clone()
}

#[derive(Debug, Clone)]
pub struct Runtime {
    handle: Handle,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum RuntimeError {
    #[error("failed to build runtime: {0}")]
    BuildFailed(String),
}

impl Runtime {
    pub fn new() -> Result<Self, RuntimeError> {
        Ok(Self {
            handle: blocking_handle(),
        })
    }

    pub fn block_on<F, T>(&self, future: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        self.handle.block_on(future)
    }

    pub fn execute_step_sync(
        &self,
        step_id: StepId,
        timeout_ms: u64,
    ) -> Result<StepResult, ExecuteNodeError> {
        let step_id_clone = step_id.clone();
        self.block_on(
            async move { crate::execution::execute_step(step_id_clone, timeout_ms).await },
        )
    }

    pub fn execute_step_with_retry_sync(
        &self,
        step_id: StepId,
        timeout_ms: u64,
        retry_policy: RetryPolicy,
    ) -> Result<StepResult, ExecuteNodeError> {
        let step_id_clone = step_id.clone();
        let retry_policy_clone = retry_policy.clone();
        self.block_on(async move {
            crate::execution::execute_step_with_retry(step_id_clone, timeout_ms, retry_policy_clone)
                .await
        })
    }

    pub fn get_status(&self, step_id: &StepId) -> ExecutionStatus {
        crate::execution::get_execution_status(step_id)
    }

    pub fn get_last_error(&self, step_id: &StepId) -> Option<ExecuteNodeError> {
        crate::execution::get_last_error(step_id)
    }

    pub fn cancel(&self, step_id: StepId) -> Result<(), ExecuteNodeError> {
        let step_id_clone = step_id.clone();
        self.block_on(async move { crate::execution::cancel_execution(step_id_clone).await })
    }
}

#[derive(Debug, Clone)]
pub struct StepContext {
    step_id: StepId,
    runtime: Runtime,
}

impl StepContext {
    pub fn new(step_id: StepId) -> Result<Self, ContextError> {
        let runtime = Runtime::new()?;
        Ok(Self { step_id, runtime })
    }

    pub fn execute(&self, timeout_ms: u64) -> Result<StepResult, ExecuteNodeError> {
        self.runtime
            .execute_step_sync(self.step_id.clone(), timeout_ms)
    }

    pub fn execute_with_retry(
        &self,
        timeout_ms: u64,
        retry_policy: RetryPolicy,
    ) -> Result<StepResult, ExecuteNodeError> {
        self.runtime
            .execute_step_with_retry_sync(self.step_id.clone(), timeout_ms, retry_policy)
    }

    pub fn status(&self) -> ExecutionStatus {
        self.runtime.get_status(&self.step_id)
    }

    pub fn last_error(&self) -> Option<ExecuteNodeError> {
        self.runtime.get_last_error(&self.step_id)
    }

    pub fn cancel(&self) -> Result<(), ExecuteNodeError> {
        self.runtime.cancel(self.step_id.clone())
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ContextError {
    #[error("failed to initialize context: {0}")]
    RuntimeInitFailed(#[from] RuntimeError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::RetryPolicyError;
    use crate::reset_all_state;
    use std::sync::LazyLock;
    use std::sync::Mutex;
    use std::sync::MutexGuard;

    static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    fn reset_state() -> MutexGuard<'static, ()> {
        let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_all_state();
        guard
    }

    #[test]
    fn runtime_creation() {
        let runtime = Runtime::new();
        assert!(runtime.is_ok());
        let _guard = reset_state();
    }

    #[test]
    fn runtime_execute_step_success() {
        let _guard = reset_state();
        let runtime = Runtime::new().unwrap();
        let result = runtime.execute_step_sync(StepId::new("step-1".to_string()), 5000);
        assert!(result.is_ok());
        assert!(result.unwrap().is_success());
        let _guard = reset_state();
    }

    #[test]
    fn runtime_execute_step_failure() {
        let _guard = reset_state();
        let runtime = Runtime::new().unwrap();
        let result = runtime.execute_step_sync(StepId::new("step-fail".to_string()), 5000);
        assert!(result.is_ok());
        assert!(!result.unwrap().is_success());
        let _guard = reset_state();
    }

    #[test]
    fn runtime_execute_step_not_found() {
        let _guard = reset_state();
        let runtime = Runtime::new().unwrap();
        let result = runtime.execute_step_sync(StepId::new("nonexistent-step".to_string()), 5000);
        assert!(result.is_err());
        let _guard = reset_state();
    }

    #[test]
    fn runtime_execute_with_retry_success() {
        let _guard = reset_state();
        let runtime = Runtime::new().unwrap();
        let retry_policy = RetryPolicy::new(3, 100, 2.0).unwrap();
        let result = runtime.execute_step_with_retry_sync(
            StepId::new("step-retry".to_string()),
            5000,
            retry_policy,
        );
        assert!(result.is_ok());
        let _guard = reset_state();
    }

    #[test]
    fn runtime_get_status() {
        let _guard = reset_state();
        let runtime = Runtime::new().unwrap();
        let status = runtime.get_status(&StepId::new("step-1".to_string()));
        assert_eq!(status, ExecutionStatus::Ready);
        let _guard = reset_state();
    }

    #[test]
    fn runtime_cancel() {
        let _guard = reset_state();
        let runtime = Runtime::new().unwrap();
        let result = runtime.cancel(StepId::new("step-1".to_string()));
        assert!(result.is_ok());
        let _guard = reset_state();
    }

    #[test]
    fn step_context_creation() {
        let _guard = reset_state();
        let context = StepContext::new(StepId::new("step-1".to_string()));
        assert!(context.is_ok());
        let _guard = reset_state();
    }

    #[test]
    fn step_context_execute() {
        let _guard = reset_state();
        let context = StepContext::new(StepId::new("step-1".to_string())).unwrap();
        let result = context.execute(5000);
        assert!(result.is_ok());
        let _guard = reset_state();
    }

    #[test]
    fn step_context_status() {
        let _guard = reset_state();
        let context = StepContext::new(StepId::new("step-1".to_string())).unwrap();
        let status = context.status();
        assert_eq!(status, ExecutionStatus::Ready);
        let _guard = reset_state();
    }

    #[test]
    fn invalid_timeout_rejected() {
        let _guard = reset_state();
        let runtime = Runtime::new().unwrap();
        let result = runtime.execute_step_sync(StepId::new("step-1".to_string()), 0);
        assert!(result.is_err());
        match result.unwrap_err() {
            ExecuteNodeError::InvalidTimeout { .. } => {}
            _ => panic!("Expected InvalidTimeout error"),
        }
        let _guard = reset_state();
    }

    #[test]
    fn retry_policy_validation() {
        let result = RetryPolicy::new(0, 100, 2.0);
        assert!(result.is_err());
        match result.unwrap_err() {
            RetryPolicyError::ZeroAttempts => {}
            _ => panic!("Expected ZeroAttempts error"),
        }
    }

    #[test]
    fn retry_backoff_calculation() {
        let policy = RetryPolicy::new(3, 100, 2.0).unwrap();
        assert_eq!(policy.calculate_backoff_delay(1), 100);
        assert_eq!(policy.calculate_backoff_delay(2), 200);
        assert_eq!(policy.calculate_backoff_delay(3), 400);
    }
}
