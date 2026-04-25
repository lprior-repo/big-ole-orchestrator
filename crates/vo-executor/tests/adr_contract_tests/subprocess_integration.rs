use super::common::*;

fn helper_path() -> String {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let target_dir = std::path::Path::new(&manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target")
        .join("debug")
        .join("test_subprocess_helper");
    target_dir.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bdd_zombie_cleanup_exit_code_propagated() {
        let _guard = state_guard();
        let helper = helper_path();
        let config = SubprocessConfig::new(
            helper,
            vec!["sleep-exit".to_string(), "0".to_string(), "42".to_string()],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(result.is_ok(), "Subprocess should complete: {:?}", result);
        assert_eq!(
            result.unwrap().exit_code,
            Some(42),
            "Exit code 42 should be propagated"
        );
    }

    #[tokio::test]
    async fn bdd_zombie_cleanup_zero_exit_succeeds() {
        let _guard = state_guard();
        let helper = helper_path();
        let config = SubprocessConfig::new(
            helper,
            vec!["sleep-exit".to_string(), "0".to_string(), "0".to_string()],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(result.is_ok(), "Quick exit subprocess should be reaped");
        assert_eq!(result.unwrap().exit_code, Some(0));
    }

    #[tokio::test]
    async fn bdd_zombie_cleanup_short_sleep_reaped() {
        let _guard = state_guard();
        let helper = helper_path();
        let config = SubprocessConfig::new(
            helper,
            vec!["sleep-exit".to_string(), "50".to_string(), "0".to_string()],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(result.is_ok(), "Short sleep subprocess should be reaped");
        assert_eq!(result.unwrap().exit_code, Some(0));
    }

    #[tokio::test]
    async fn bdd_fd_budget_subprocess_completes_without_fd_leak() {
        let _guard = state_guard();
        let helper = helper_path();
        let config = SubprocessConfig::new(
            helper,
            vec!["sleep-exit".to_string(), "100".to_string(), "0".to_string()],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(result.is_ok(), "Subprocess should complete without FD leak");
        assert_eq!(result.unwrap().exit_code, Some(0));
    }
}
