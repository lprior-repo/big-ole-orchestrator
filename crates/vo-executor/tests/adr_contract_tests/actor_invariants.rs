use super::common::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn single_writer_invariant_no_concurrent_executing_state() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        let (r1, r2) = tokio::join!(
            execute_step(step_id.clone(), 5000),
            execute_step(step_id.clone(), 5000)
        );

        assert!(r1.is_ok());
        assert!(r2.is_ok());

        let status = get_execution_status(&step_id);
        assert!(status.is_ready());
    }

    #[tokio::test]
    async fn bounded_mailbox_step_error_does_not_cascade() {
        let _guard = state_guard();
        let step_err = StepId::new("step-transient".to_string());
        let step_ok = StepId::new("step-good".to_string());

        let (r_err, r_ok) = tokio::join!(
            execute_step(step_err.clone(), 5000),
            execute_step(step_ok.clone(), 5000)
        );

        assert!(r_err.is_err());
        assert!(r_ok.is_ok());

        let error_ok = get_last_error(&step_ok);
        assert!(
            error_ok.is_none(),
            "Error should not leak to unrelated step"
        );
    }

    #[tokio::test]
    async fn stale_actor_resurrection_prevented() {
        let _guard = state_guard();
        let step_id = StepId::new("step-good".to_string());

        execute_step(step_id.clone(), 5000).await.unwrap();
        reset_all_state();

        let result = execute_step(step_id.clone(), 5000).await;
        assert!(result.is_ok(), "Should work cleanly after state reset");
    }

    #[tokio::test]
    async fn concurrent_different_steps_independent_errors() {
        let _guard = state_guard();
        let steps = vec![
            StepId::new("step-transient".to_string()),
            StepId::new("step-good".to_string()),
            StepId::new("step-fail".to_string()),
            StepId::new("step-1".to_string()),
        ];

        let handles: Vec<_> = steps
            .into_iter()
            .map(|sid| tokio::spawn(async move { (sid.clone(), execute_step(sid, 5000).await) }))
            .collect();

        for handle in handles {
            let (sid, result) = handle.await.unwrap();
            match &result {
                Ok(_) => {}
                Err(ExecuteNodeError::TransientError { .. }) => {
                    assert_eq!(sid.as_str(), "step-transient");
                }
                Err(other) => panic!("Unexpected error for {}: {:?}", sid, other),
            }
        }
    }

    #[tokio::test]
    async fn error_per_step_isolation() {
        let _guard = state_guard();
        let step_a = StepId::new("step-transient".to_string());
        let step_b = StepId::new("step-transient-clone".to_string());

        let r_a = execute_step(step_a.clone(), 5000).await;
        assert!(r_a.is_err());

        let error_a = get_last_error(&step_a);
        let error_b = get_last_error(&step_b);

        assert!(error_a.is_some());
        assert!(error_b.is_none(), "Different step should not inherit error");
    }
}
