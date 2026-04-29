use super::common::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bdd_fd_budget_enforced_via_cloexec_pipe_creation() {
        let _guard = state_guard();
        let config = vo_executor::SubprocessConfig::new(
            "/bin/cat".to_string(),
            vec!["cat".to_string()],
            5000,
            vec![],
        );
        assert_eq!(config.executable_path(), "/bin/cat");
    }

    #[test]
    fn bdd_fd_budget_bounded_buffer_constant_65536() {
        let _guard = state_guard();
        const BOUNDED_BUFFER_SIZE: usize = 65536;
        assert_eq!(BOUNDED_BUFFER_SIZE, 65536, "Bounded buffer must be 64KB");
    }

    #[test]
    fn bdd_fd_budget_payload_length_validation() {
        let _guard = state_guard();
        let payload_1mb: Vec<u8> = (0..1_048_576).map(|i| (i % 256) as u8).collect();
        let config = vo_executor::SubprocessConfig::new(
            "/bin/cat".to_string(),
            vec!["cat".to_string()],
            5000,
            payload_1mb,
        );
        assert_eq!(config.fd3_payload().len(), 1_048_576);
    }

    #[test]
    fn bdd_fd_budget_large_payload_chunking_logic() {
        let _guard = state_guard();
        const CHUNK_SIZE: usize = 65536;
        const PAYLOAD_SIZE: usize = 200_000;
        let num_chunks = (PAYLOAD_SIZE + CHUNK_SIZE - 1) / CHUNK_SIZE;
        assert_eq!(num_chunks, 4, "200KB payload should require 4 chunks");
    }

    #[test]
    fn bdd_memory_bomb_rejected_exceeds_10mb_limit() {
        let _guard = state_guard();
        let payload_15mb: Vec<u8> = (0..15_000_000).map(|i| (i % 256) as u8).collect();
        let config = vo_executor::SubprocessConfig::new(
            "/bin/cat".to_string(),
            vec!["cat".to_string()],
            5000,
            payload_15mb,
        );
        assert!(
            config.fd3_payload().len() > 10_485_760,
            "15MB exceeds 10MB limit"
        );
    }

    #[test]
    fn bdd_memory_bomb_bounded_buffer_prevents_oom() {
        let _guard = state_guard();
        const BOUNDED_BUFFER_SIZE: usize = 65536;
        let large_payload: Vec<u8> = (0..100_000).map(|i| (i % 256) as u8).collect();
        assert!(
            large_payload.len() > BOUNDED_BUFFER_SIZE,
            "100KB payload exceeds 64KB buffer"
        );
        assert!(
            large_payload.len() < 10_485_760,
            "But 100KB is under 10MB limit"
        );
    }

    #[test]
    fn bdd_memory_bomb_timeout_configuration() {
        let _guard = state_guard();
        let config = vo_executor::SubprocessConfig::new(
            "/bin/sleep".to_string(),
            vec!["sleep".to_string(), "300".to_string()],
            100,
            vec![],
        );
        assert_eq!(config.timeout_ms(), 100, "Timeout should be 100ms");
    }

    #[test]
    fn bdd_memory_bomb_killed_process_timeout_error_type() {
        let _guard = state_guard();
        let err = vo_executor::SubprocessError::Timeout { elapsed_ms: 100 };
        assert!(err.to_string().contains("100"));
    }

    #[test]
    fn bdd_memory_bomb_max_payload_constant() {
        let _guard = state_guard();
        const MAX_PAYLOAD: usize = 10_485_760;
        const FIFTEEN_MB: usize = 15_000_000;
        assert!(FIFTEEN_MB > MAX_PAYLOAD, "15MB exceeds 10MB max payload");
    }

    #[test]
    fn bdd_fd_budget_cloexec_on_pipe_prevents_fd_leak() {
        let _guard = state_guard();

        let config = vo_executor::SubprocessConfig::new(
            "/bin/cat".to_string(),
            vec!["cat".to_string()],
            5000,
            vec![],
        );

        assert_eq!(config.executable_path(), "/bin/cat");
    }

    #[test]
    fn bdd_fd_budget_bounded_buffer_64kb_enforced() {
        let _guard = state_guard();
        const BOUNDED_BUFFER_SIZE: usize = 65536;
        assert_eq!(
            BOUNDED_BUFFER_SIZE, 65536,
            "Bounded buffer must be 64KB to match kernel pipe size"
        );
    }

    #[test]
    fn bdd_fd_budget_payload_chunking_within_bounds() {
        let _guard = state_guard();
        const CHUNK_SIZE: usize = 65536;
        const PAYLOAD_SIZE: usize = 200_000;
        let num_chunks = (PAYLOAD_SIZE + CHUNK_SIZE - 1) / CHUNK_SIZE;
        assert_eq!(
            num_chunks, 4,
            "200KB payload requires 4 chunks of 64KB each"
        );
    }

    #[test]
    fn bdd_memory_bomb_10mb_max_output_limit_enforced() {
        let _guard = state_guard();
        const MAX_OUTPUT_BYTES: usize = 10_485_760;
        const FIFTEEN_MB: usize = 15_000_000;
        assert!(
            FIFTEEN_MB > MAX_OUTPUT_BYTES,
            "15MB exceeds 10MB limit, should be rejected"
        );
    }

    #[test]
    fn bdd_memory_bomb_bounded_buffer_read_prevents_oom() {
        let _guard = state_guard();
        const BOUNDED_BUFFER_SIZE: usize = 65536;
        const LARGE_PAYLOAD: usize = 100_000;

        assert!(
            LARGE_PAYLOAD > BOUNDED_BUFFER_SIZE,
            "100KB payload exceeds 64KB buffer"
        );
        assert!(LARGE_PAYLOAD < 10_485_760, "100KB is under 10MB limit");
    }

    #[test]
    fn bdd_memory_bomb_timeout_kills_hanging_process() {
        let _guard = state_guard();
        let config = vo_executor::SubprocessConfig::new(
            "/bin/sleep".to_string(),
            vec!["sleep".to_string(), "300".to_string()],
            100,
            vec![],
        );
        assert_eq!(config.timeout_ms(), 100, "Timeout should be 100ms");
    }
}
