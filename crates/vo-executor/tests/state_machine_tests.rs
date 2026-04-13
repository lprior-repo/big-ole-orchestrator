//! State machine tests for vo-executor
//!
//! Tests all valid and invalid state transitions in the workflow executor:
//! - Ready → Executing (via execute_step)
//! - Executing → Ready (via execute_step completion)
//! - Ready → Cancelled (via cancel_execution)
//! - Executing → Cancelled error (via cancel_execution - state unchanged)
//! - Terminal states: Cancelled, Completed

#[cfg(test)]
mod state_machine_transition_tests {
    use std::sync::LazyLock;
    use std::sync::Mutex;
    use std::sync::MutexGuard;
    use vo_executor::{
        cancel_execution, execute_step, get_execution_status, reset_all_state, ExecutionStatus,
        StepId, StepResult,
    };

    static STATE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    fn state_guard() -> MutexGuard<'static, ()> {
        let guard = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_all_state();
        guard
    }

    fn get_status(step_id: &StepId) -> ExecutionStatus {
        get_execution_status(step_id)
    }

    fn assert_ready(step_id: &StepId) {
        assert!(
            matches!(get_status(step_id), ExecutionStatus::Ready),
            "Expected Ready state"
        );
    }

    fn assert_cancelled(step_id: &StepId) {
        assert!(
            matches!(get_status(step_id), ExecutionStatus::Cancelled { .. }),
            "Expected Cancelled state"
        );
    }

    // =========================================================================
    // Ready → Executing Transitions
    // =========================================================================

    #[tokio::test]
    async fn state_ready_to_executing_to_ready_flow() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        assert_ready(&step_id);

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());
        assert_ready(&step_id);
    }

    // =========================================================================
    // Executing → Ready Transitions (successful completion)
    // =========================================================================

    #[tokio::test]
    async fn state_executing_to_ready_after_success() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(matches!(result, Ok(StepResult::Success { .. })));
        assert_ready(&step_id);
    }

    #[tokio::test]
    async fn state_executing_to_ready_after_failure() {
        let _guard = state_guard();
        let step_id = StepId::new("step-fail".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(matches!(result, Ok(StepResult::Failure { .. })));
        assert_ready(&step_id);
    }

    #[tokio::test]
    async fn state_executing_to_ready_after_transient_error() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_err());
        assert_ready(&step_id);
    }

    // =========================================================================
    // Ready → Cancelled Transitions
    // =========================================================================

    #[tokio::test]
    async fn state_ready_to_cancelled_on_cancel() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        assert_ready(&step_id);

        let result = cancel_execution(step_id.clone()).await;
        assert!(result.is_ok());
        assert_cancelled(&step_id);
    }

    #[tokio::test]
    async fn state_cancelled_is_terminal_no_transitions() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        cancel_execution(step_id.clone()).await.expect("cancel should succeed");
        assert_cancelled(&step_id);

        let result = cancel_execution(step_id.clone()).await;
        assert!(result.is_ok());
        assert_cancelled(&step_id);
    }

    #[tokio::test]
    async fn state_cancelled_can_still_execute() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        cancel_execution(step_id.clone()).await.expect("cancel should succeed");
        assert_cancelled(&step_id);

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());
        assert_ready(&step_id);
    }

    // =========================================================================
    // Cancelled State Tests
    // =========================================================================

    #[tokio::test]
    async fn state_cancel_multiple_steps_independently() {
        let _guard = state_guard();

        let step_a = StepId::new("step-1".to_string());
        let step_b = StepId::new("step-good".to_string());
        let step_c = StepId::new("step-retry".to_string());

        cancel_execution(step_a.clone()).await.expect("cancel should succeed");
        cancel_execution(step_b.clone()).await.expect("cancel should succeed");
        cancel_execution(step_c.clone()).await.expect("cancel should succeed");

        assert_cancelled(&step_a);
        assert_cancelled(&step_b);
        assert_cancelled(&step_c);
    }

    // =========================================================================
    // Invalid Transition Tests
    // =========================================================================

    #[tokio::test]
    async fn state_executing_cannot_start_another_execution() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let handle = tokio::spawn(execute_step(step_id.clone(), 5000));
        tokio::task::yield_now().await;

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::InvalidTransition { .. })
        ));

        let _ = handle.await;
    }

    #[tokio::test]
    async fn state_concurrent_same_step_both_get_result() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        let (result_a, result_b) = tokio::join!(
            execute_step(step_id.clone(), 5000),
            execute_step(step_id.clone(), 5000)
        );

        assert!(result_a.is_ok());
        assert!(result_b.is_ok());
        assert!(result_a.is_success());
        assert!(result_b.is_success());
    }

    // =========================================================================
    // Terminal State Tests
    // =========================================================================

    #[tokio::test]
    async fn state_completed_is_defined_but_never_set() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());

        let status = get_status(&step_id);
        assert!(matches!(status, ExecutionStatus::Ready));
    }

    // =========================================================================
    // Complex State Transition Scenarios
    // =========================================================================

    #[tokio::test]
    async fn state_execute_cancel_retry_flow() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        assert_ready(&step_id);

        let result1 = execute_step(step_id.clone(), 5000).await;
        assert!(result1.is_ok());
        assert_ready(&step_id);

        cancel_execution(step_id.clone()).await.expect("cancel should succeed");
        assert_cancelled(&step_id);

        let result2 = execute_step(step_id.clone(), 5000).await;
        assert!(result2.is_ok());
        assert_ready(&step_id);
    }

    #[tokio::test]
    async fn state_interleaved_steps_maintain_independent_states() {
        let _guard = state_guard();

        let step_a = StepId::new("step-good".to_string());
        let step_b = StepId::new("step-fail".to_string());

        let (result_a, result_b) = tokio::join!(
            execute_step(step_a.clone(), 5000),
            execute_step(step_b.clone(), 5000)
        );

        assert!(result_a.expect("a should complete").is_ok());
        assert!(matches!(
            result_b.expect("b should complete"),
            Ok(StepResult::Failure { .. })
        ));

        assert_ready(&step_a);
        assert_ready(&step_b);
    }

    #[tokio::test]
    async fn state_cancel_one_step_does_not_affect_other() {
        let _guard = state_guard();

        let step_a = StepId::new("step-good".to_string());
        let step_b = StepId::new("step-1".to_string());

        cancel_execution(step_a.clone()).await.expect("cancel should succeed");
        assert_cancelled(&step_a);
        assert_ready(&step_b);

        let result = execute_step(step_b.clone(), 5000).await;
        assert!(result.is_ok());
        assert_ready(&step_b);
        assert_cancelled(&step_a);
    }

    #[tokio::test]
    async fn state_concurrent_cancel_on_ready_steps() {
        let _guard = state_guard();

        let step_a = StepId::new("step-1".to_string());
        let step_b = StepId::new("step-good".to_string());
        let step_c = StepId::new("step-retry".to_string());

        let (result_a, result_b, result_c) = tokio::join!(
            cancel_execution(step_a.clone()),
            cancel_execution(step_b.clone()),
            cancel_execution(step_c.clone())
        );

        assert!(result_a.is_ok());
        assert!(result_b.is_ok());
        assert!(result_c.is_ok());

        assert_cancelled(&step_a);
        assert_cancelled(&step_b);
        assert_cancelled(&step_c);
    }

    #[tokio::test]
    async fn state_timeout_during_execution_returns_to_ready() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let result = execute_step(step_id.clone(), 1).await;
        assert!(matches!(
            result,
            Err(vo_executor::ExecuteNodeError::TimeoutExceeded { .. })
        ));
        assert_ready(&step_id);
    }

    #[tokio::test]
    async fn state_slow_step_with_sufficient_timeout_completes() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok());
        assert_ready(&step_id);
    }

    #[tokio::test]
    async fn state_step_not_found_leaves_no_state() {
        let _guard = state_guard();
        let step_id = StepId::new("nonexistent-step".to_string());

        assert_ready(&step_id);

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(matches!(result, Err(vo_executor::ExecuteNodeError::StepNotFound { .. })));

        assert_ready(&step_id);
    }

    #[tokio::test]
    async fn state_all_step_types_transition_correctly() {
        let _guard = state_guard();

        let success = StepId::new("step-1".to_string());
        let failure = StepId::new("step-fail".to_string());
        let transient = StepId::new("step-transient".to_string());
        let slow = StepId::new("step-slow".to_string());

        let (r1, r2, r3, r4) = tokio::join!(
            execute_step(success.clone(), 5000),
            execute_step(failure.clone(), 5000),
            execute_step(transient.clone(), 5000),
            execute_step(slow.clone(), 5000)
        );

        assert!(matches!(r1, Ok(StepResult::Success { .. })));
        assert!(matches!(r2, Ok(StepResult::Failure { .. })));
        assert!(r3.is_err());
        assert!(r4.is_ok());

        assert_ready(&success);
        assert_ready(&failure);
        assert_ready(&transient);
        assert_ready(&slow);
    }
}