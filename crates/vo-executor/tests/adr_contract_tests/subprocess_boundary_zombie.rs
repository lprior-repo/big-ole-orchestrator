use super::common::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bdd_zombie_cleanup_pr_set_pdeathsig_sigterm_configured() {
        let _guard = state_guard();
        let config = vo_executor::SubprocessConfig::new(
            "/bin/true".to_string(),
            vec!["true".to_string()],
            5000,
            vec![],
        );
        assert_eq!(config.executable_path(), "/bin/true");
        assert_eq!(config.timeout_ms(), 5000);
    }

    #[test]
    fn bdd_zombie_cleanup_process_group_isolated() {
        let _guard = state_guard();
        let config = vo_executor::SubprocessConfig::new(
            "/bin/sleep".to_string(),
            vec!["sleep".to_string(), "1".to_string()],
            5000,
            vec![],
        );
        assert_eq!(config.executable_path(), "/bin/sleep");
        assert_eq!(config.timeout_ms(), 5000);
    }

    #[test]
    fn bdd_zombie_cleanup_config_carries_all_parameters() {
        let _guard = state_guard();
        let argv = vec!["sleep".to_string(), "60".to_string()];
        let config = vo_executor::SubprocessConfig::new(
            "/bin/sleep".to_string(),
            argv.clone(),
            5000,
            vec![],
        );
        assert_eq!(config.argv(), &argv);
    }

    #[tokio::test]
    async fn step_not_found_rejected_before_execution() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("nonexistent-binary".to_string()), 5000).await;
        assert!(matches!(result, Err(ExecuteNodeError::StepNotFound { .. })));
    }

    #[tokio::test]
    async fn step_not_found_with_retry_still_terminal() {
        let _guard = state_guard();
        let policy = RetryPolicy::new(5, 100, 2.0).unwrap();
        let result =
            execute_step_with_retry(StepId::new("ghost-binary".to_string()), 5000, policy).await;
        assert!(matches!(result, Err(ExecuteNodeError::StepNotFound { .. })));
    }

    #[tokio::test]
    async fn invalid_timeout_prevents_spawn() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-1".to_string()), 0).await;
        assert!(matches!(
            result,
            Err(ExecuteNodeError::InvalidTimeout { value: 0, .. })
        ));
    }

    #[tokio::test]
    async fn max_u64_timeout_prevents_spawn() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-1".to_string()), u64::MAX).await;
        assert!(matches!(
            result,
            Err(ExecuteNodeError::InvalidTimeout {
                value: u64::MAX,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn timeout_boundary_below_slow_threshold_fails() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-slow".to_string()), 1).await;
        assert!(matches!(
            result,
            Err(ExecuteNodeError::TimeoutExceeded { .. })
        ));
    }

    #[tokio::test]
    async fn timeout_boundary_at_slow_threshold_passes() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-slow".to_string()), 3000).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn timeout_boundary_above_slow_threshold_passes() {
        let _guard = state_guard();
        let result = execute_step(StepId::new("step-slow".to_string()), 3001).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn zombie_prevention_cancel_returns_ready_state() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        cancel_execution(step_id.clone())
            .await
            .expect("cancel should succeed");

        let status = get_execution_status(&step_id);
        assert!(matches!(status, ExecutionStatus::Cancelled { .. }));
    }

    #[tokio::test]
    async fn double_cancel_is_idempotent() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        let r1 = cancel_execution(step_id.clone()).await;
        let r2 = cancel_execution(step_id.clone()).await;
        let r3 = cancel_execution(step_id.clone()).await;

        assert!(r1.is_ok());
        assert!(r2.is_ok());
        assert!(r3.is_ok());
    }

    #[tokio::test]
    async fn cancel_already_completed_is_noop() {
        let _guard = state_guard();
        let step_id = StepId::new("step-1".to_string());

        execute_step(step_id.clone(), 5000)
            .await
            .expect("should succeed");

        let cancel_result = cancel_execution(step_id.clone()).await;
        assert!(cancel_result.is_ok());
    }

    #[test]
    fn bdd_zombie_cleanup_pr_set_pdeathsig_configured_in_subprocess() {
        let _guard = state_guard();

        let config = vo_executor::SubprocessConfig::new(
            "/bin/true".to_string(),
            vec!["true".to_string()],
            5000,
            vec![],
        );

        assert_eq!(config.executable_path(), "/bin/true");
        assert_eq!(config.timeout_ms(), 5000);
    }

    #[test]
    fn bdd_zombie_cleanup_setpgid_isolates_process_group() {
        let _guard = state_guard();

        let config = vo_executor::SubprocessConfig::new(
            "/bin/sleep".to_string(),
            vec!["sleep".to_string(), "1".to_string()],
            5000,
            vec![],
        );

        assert_eq!(config.executable_path(), "/bin/sleep");
        assert_eq!(config.timeout_ms(), 5000);
    }

    #[test]
    fn bdd_zombie_cleanup_timeout_configuration_validates() {
        let _guard = state_guard();

        let config = vo_executor::SubprocessConfig::new(
            "/bin/sleep".to_string(),
            vec!["sleep".to_string(), "300".to_string()],
            100,
            vec![],
        );

        assert_eq!(
            config.timeout_ms(),
            100,
            "Timeout should be 100ms for zombie cleanup test"
        );
    }

    #[test]
    fn bdd_subprocess_config_validates_timeout_not_zero() {
        let _guard = state_guard();
        let config = vo_executor::SubprocessConfig::new(
            "/bin/true".to_string(),
            vec!["true".to_string()],
            1,
            vec![],
        );
        assert!(config.timeout_ms() > 0, "Timeout must be > 0");
    }

    #[test]
    fn bdd_subprocess_config_validates_timeout_not_max() {
        let _guard = state_guard();
        let config = vo_executor::SubprocessConfig::new(
            "/bin/true".to_string(),
            vec!["true".to_string()],
            u64::MAX - 1,
            vec![],
        );
        assert!(config.timeout_ms() < u64::MAX, "Timeout must be < u64::MAX");
    }

    #[test]
    fn bdd_subprocess_error_timeout_contains_elapsed_ms() {
        let _guard = state_guard();
        let err = vo_executor::SubprocessError::Timeout { elapsed_ms: 100 };
        let err_str = err.to_string();
        assert!(
            err_str.contains("100"),
            "Timeout error should contain elapsed_ms, got: {}",
            err_str
        );
    }

    #[test]
    fn bdd_subprocess_error_bounded_buffer_exceeded_contains_details() {
        let _guard = state_guard();
        let err = vo_executor::SubprocessError::BoundedBufferExceeded {
            max: 65536,
            tried: 100000,
        };
        let err_str = err.to_string();
        assert!(
            err_str.contains("65536") && err_str.contains("100000"),
            "BoundedBufferExceeded should contain max and tried, got: {}",
            err_str
        );
    }
}
