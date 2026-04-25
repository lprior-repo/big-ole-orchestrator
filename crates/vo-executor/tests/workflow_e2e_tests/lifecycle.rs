use super::common::prelude::*;

mod lifecycle_success {
    use super::*;

    #[tokio::test]
    async fn complete_lifecycle_success_step_ingestion_to_ready_state() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        let initial_status = get_execution_status(&step_id);
        assert!(initial_status.is_ready(), "Initial status should be Ready");

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok(), "Execute should succeed");
        assert!(
            matches!(result.unwrap(), StepResult::Success { output } if output == "done"),
            "Should return Success with 'done' output"
        );

        let final_status = get_execution_status(&step_id);
        assert!(
            final_status.is_ready(),
            "Final status should be Ready after completion"
        );
    }

    #[tokio::test]
    async fn complete_lifecycle_failure_step_ingestion_to_persisted_failure() {
        let _guard = state_guard();
        let step_id = StepId::new("step-fail".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(
            matches!(result, Ok(StepResult::Failure { .. })),
            "Failure step should return Failure result"
        );

        let status = get_execution_status(&step_id);
        assert!(
            status.is_ready(),
            "Status should be Ready after Failure step"
        );
    }

    #[tokio::test]
    async fn complete_lifecycle_transient_error_persisted_in_last_error() {
        let _guard = state_guard();
        let step_id = StepId::new("step-transient".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_err(), "Transient step should return error");

        let stored_error = get_last_error(&step_id);
        assert!(
            stored_error.is_some(),
            "Last error should be persisted after transient failure"
        );

        let final_status = get_execution_status(&step_id);
        assert!(
            final_status.is_ready(),
            "Status should be Ready after transient error"
        );
    }

    #[tokio::test]
    async fn complete_lifecycle_not_found_step_raises_error() {
        let _guard = state_guard();
        let step_id = StepId::new("nonexistent-workflow-step".to_string());

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(
            matches!(
                result,
                Err(ExecuteNodeError::StepNotFound { .. })
            ),
            "Unknown step should return StepNotFound error"
        );

        let status = get_execution_status(&step_id);
        assert!(
            status.is_ready(),
            "Status should be Ready for unknown step (not in STATE map)"
        );
    }

    #[tokio::test]
    async fn complete_lifecycle_cancelled_execution_shows_cancelled_state() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        cancel_execution(step_id.clone())
            .await
            .expect("Cancel on Ready should succeed");

        let status = get_execution_status(&step_id);
        match status {
            ExecutionStatus::Cancelled { reason } => {
                assert!(reason.contains("cancelled"));
            }
            other => panic!("Expected Cancelled status, got {:?}", other),
        }
    }
}

mod state_persistence {
    use super::*;

    #[tokio::test]
    async fn state_persistence_verification_error_survives_across_calls() {
        let _guard = state_guard();
        let step_id = StepId::new("transient-step-999".to_string());

        execute_step(step_id.clone(), 5000)
            .await
            .expect_err("First call should fail");
        assert!(
            get_last_error(&step_id).is_some(),
            "Error should be persisted after first call"
        );

        execute_step(step_id.clone(), 5000)
            .await
            .expect_err("Second call should fail");
        assert!(
            get_last_error(&step_id).is_some(),
            "Error should persist after second call"
        );
    }

    #[tokio::test]
    async fn state_persistence_verification_success_clears_prior_error() {
        let _guard = state_guard();
        let step_id = StepId::new("step-999".to_string());

        let prior_error = ExecuteNodeError::TransientError {
            reason: "prior error".to_string(),
            recoverable: true,
        };
        vo_executor::set_error(step_id.as_str(), prior_error);
        assert!(
            get_last_error(&step_id).is_some(),
            "Error should be set before execution"
        );

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok(), "Good step should succeed");

        let stored_error = get_last_error(&step_id);
        assert!(
            stored_error.is_none(),
            "Error should be cleared after successful execution"
        );
    }

    #[tokio::test]
    async fn state_persistence_verification_different_steps_independent_errors() {
        let _guard = state_guard();
        let step_a = StepId::new("step-transient".to_string());
        let step_b = StepId::new("step-1".to_string());

        execute_step(step_a.clone(), 5000)
            .await
            .expect_err("Step A should fail");
        assert!(
            get_last_error(&step_a).is_some(),
            "Step A should have error"
        );

        execute_step(step_b.clone(), 5000)
            .await
            .expect("Step B should succeed");
        assert!(
            get_last_error(&step_b).is_none(),
            "Step B should not have error"
        );

        assert!(
            get_last_error(&step_a).is_some(),
            "Step A error should still be present"
        );
    }
}
