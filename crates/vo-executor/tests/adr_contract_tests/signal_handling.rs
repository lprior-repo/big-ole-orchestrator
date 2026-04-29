use super::common::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sigterm_cancel_during_ready() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        let result = cancel_execution(step_id.clone()).await;
        assert!(result.is_ok());

        let status = get_execution_status(&step_id);
        assert!(
            matches!(status, ExecutionStatus::Cancelled { reason } if reason.contains("cancelled"))
        );
    }

    #[tokio::test]
    async fn sigterm_then_reexecute_succeeds() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        cancel_execution(step_id.clone())
            .await
            .expect("cancel should succeed");

        let status_after_cancel = get_execution_status(&step_id);
        assert!(matches!(
            status_after_cancel,
            ExecutionStatus::Cancelled { .. }
        ));

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok(), "Should succeed after cancel + reexecute");
    }

    #[tokio::test]
    async fn sigkill_escalation_timeout_for_slow_step() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let result = execute_step(step_id.clone(), 1).await;
        assert!(
            matches!(result, Err(ExecuteNodeError::TimeoutExceeded { .. })),
            "Slow step with 1ms timeout should timeout (SIGKILL escalation)"
        );
    }

    #[tokio::test]
    async fn grace_period_timeout_boundary() {
        let _guard = state_guard();
        let step_id = StepId::new("step-slow".to_string());

        let below = execute_step(step_id.clone(), 2999).await;
        assert!(below.is_err(), "2999ms < 3000ms threshold should timeout");

        let at = execute_step(step_id.clone(), 3000).await;
        assert!(at.is_ok(), "3000ms == threshold should succeed");

        let above = execute_step(step_id.clone(), 3001).await;
        assert!(above.is_ok(), "3001ms > threshold should succeed");
    }

    #[tokio::test]
    async fn signal_during_transient_failure_handled() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());

        let exec_result = execute_step(step_id.clone(), 5000).await;
        assert!(exec_result.is_err());

        let cancel_result = cancel_execution(step_id.clone()).await;
        assert!(cancel_result.is_ok());

        let status = get_execution_status(&step_id);
        assert!(matches!(status, ExecutionStatus::Cancelled { .. }));
    }

    #[tokio::test]
    async fn multiple_cancel_calls_are_safe() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        for _ in 0..10 {
            let result = cancel_execution(step_id.clone()).await;
            assert!(result.is_ok());
        }
    }
}
