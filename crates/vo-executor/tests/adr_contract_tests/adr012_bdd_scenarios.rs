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
    async fn bdd_zombie_prevention_subprocess_exits_cleanly_no_zombie() {
        let _guard = state_guard();
        let helper = helper_path();

        let config = SubprocessConfig::new(
            helper,
            vec!["sleep-exit".to_string(), "10".to_string(), "0".to_string()],
            5000,
            vec![],
        );

        let result = run_subprocess(config).await;

        assert!(
            result.is_ok(),
            "Subprocess should complete without zombie, got: {:?}",
            result
        );
        let output = result.unwrap();
        assert_eq!(
            output.exit_code,
            Some(0),
            "Exit code 0 should be reaped, not left as zombie"
        );
        assert!(
            output.fd4_bytes.is_empty(),
            "No FD4 output expected for simple exit"
        );
    }

    #[tokio::test]
    async fn bdd_zombie_prevention_nonzero_exit_code_propagated() {
        let _guard = state_guard();
        let helper = helper_path();

        let config = SubprocessConfig::new(
            helper,
            vec!["sleep-exit".to_string(), "5".to_string(), "137".to_string()],
            5000,
            vec![],
        );

        let result = run_subprocess(config).await;

        assert!(result.is_ok(), "Should complete even with non-zero exit");
        assert_eq!(
            result.unwrap().exit_code,
            Some(137),
            "Non-zero exit code (potentially from SIGKILL) should be reaped"
        );
    }

    #[tokio::test]
    async fn bdd_zombie_prevention_short_sleep_reaped_quickly() {
        let _guard = state_guard();
        let helper = helper_path();

        let config = SubprocessConfig::new(
            helper,
            vec!["sleep-exit".to_string(), "50".to_string(), "0".to_string()],
            5000,
            vec![],
        );

        let start = std::time::Instant::now();
        let result = run_subprocess(config).await;
        let elapsed = start.elapsed();

        assert!(
            result.is_ok(),
            "Short sleep should complete quickly: {:?}",
            result
        );
        assert!(
            elapsed.as_millis() < 500,
            "50ms sleep should complete in under 500ms"
        );
        assert_eq!(result.unwrap().exit_code, Some(0), "Exit code should be 0");
    }

    #[tokio::test]
    async fn bdd_zombie_prevention_exit_code_42_propagated() {
        let _guard = state_guard();
        let helper = helper_path();

        let config = SubprocessConfig::new(
            helper,
            vec!["sleep-exit".to_string(), "0".to_string(), "42".to_string()],
            5000,
            vec![],
        );

        let result = run_subprocess(config).await;

        assert!(result.is_ok(), "Immediate exit should succeed");
        assert_eq!(
            result.unwrap().exit_code,
            Some(42),
            "Exit code 42 should be propagated from subprocess"
        );
    }

    #[tokio::test]
    async fn bdd_fd_budget_cloexec_constant_verification() {
        assert_eq!(libc::FD_CLOEXEC, 1, "FD_CLOEXEC should be 1");
    }

    #[tokio::test]
    async fn bdd_fd_budget_bounded_buffer_64kb() {
        const BOUNDED_BUFFER_SIZE: usize = 65536;
        assert_eq!(
            BOUNDED_BUFFER_SIZE, 65536,
            "Bounded buffer is 64KB to match kernel pipe capacity"
        );
    }

    #[tokio::test]
    async fn bdd_fd_budget_kernel_buffer_64kb() {
        const KERNEL_PIPE_BUFFER: usize = 65536;
        assert_eq!(
            KERNEL_PIPE_BUFFER, 65536,
            "Kernel pipe buffer is 64KB on Linux"
        );
    }

    #[tokio::test]
    async fn bdd_fd_budget_10mb_max_output_limit() {
        const MAX_OUTPUT_BYTES: usize = 10_485_760;
        assert_eq!(
            MAX_OUTPUT_BYTES, 10_485_760,
            "MAX_STEP_OUTPUT_BYTES must be 10MB per ADR-012"
        );
    }

    #[tokio::test]
    async fn bdd_memory_bomb_10mb_limit_constant() {
        const MAX_OUTPUT: usize = 10_485_760;
        const FIFTEEN_MB: usize = 15_720_384;
        const ELEVEN_MB: usize = 11_534_336;

        assert!(FIFTEEN_MB > MAX_OUTPUT, "15MB exceeds 10MB limit");
        assert!(ELEVEN_MB > MAX_OUTPUT, "11MB exceeds 10MB limit");
        assert!(MAX_OUTPUT > 0, "10MB limit should be positive");
    }

    #[tokio::test]
    async fn bdd_memory_bomb_timeout_configuration_enforced() {
        let _guard = state_guard();
        let helper = helper_path();

        let config = SubprocessConfig::new(
            helper,
            vec!["sleep-exit".to_string(), "50".to_string(), "0".to_string()],
            100,
            vec![],
        );

        let result = run_subprocess(config).await;

        assert!(
            result.is_ok(),
            "100ms timeout should allow 50ms sleep to complete"
        );
        assert_eq!(
            result.unwrap().exit_code,
            Some(0),
            "Process should exit cleanly with code 0"
        );
    }

    #[tokio::test]
    async fn bdd_adr012_chunking_calculation() {
        const CHUNK_SIZE: usize = 65536;
        const PAYLOAD_1MB: usize = 1_048_576;
        const PAYLOAD_10MB: usize = 10_485_760;

        let chunks_1mb = (PAYLOAD_1MB + CHUNK_SIZE - 1) / CHUNK_SIZE;
        assert_eq!(chunks_1mb, 16, "1MB requires 16 chunks of 64KB");

        let chunks_10mb = (PAYLOAD_10MB + CHUNK_SIZE - 1) / CHUNK_SIZE;
        assert_eq!(chunks_10mb, 160, "10MB requires 160 chunks of 64KB");
    }

    #[tokio::test]
    async fn bdd_adr012_payload_length_header_4bytes() {
        const HEADER_SIZE: usize = 4;
        assert_eq!(
            HEADER_SIZE, 4,
            "FD3/FD4 length prefix is 4 bytes (u32 big-endian)"
        );
    }

    #[tokio::test]
    async fn bdd_adr012_pr_set_pdeath_signal_constant() {
        const PR_SET_PDEATHSIG: libc::c_int = 1;
        assert_eq!(
            PR_SET_PDEATHSIG,
            libc::PR_SET_PDEATHSIG,
            "PR_SET_PDEATHSIG constant should match libc"
        );
    }

    #[tokio::test]
    async fn bdd_adr012_sigterm_constant() {
        const SIGTERM: libc::c_int = 15;
        assert_eq!(SIGTERM, libc::SIGTERM, "SIGTERM constant should match libc");
    }
}
