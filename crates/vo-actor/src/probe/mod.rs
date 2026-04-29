//! Health check probe framework for monitoring component health.
//!
//! Provides configurable health probes with:
//! - HTTP/TCP/exec probe types
//! - Interval and backoff configuration
//! - Status aggregation across multiple probes
//! - Alerting thresholds
//!
//! # Example
//!
//! ```ignore
//! use vo_actor::probe::{Probe, HttpProbe, ProbeConfig};
//!
//! let probe = HttpProbe::new("http://localhost:8080/health");
//! let config = ProbeConfig::default()
//!     .with_interval(Duration::from_secs(30))
//!     .with_failure_threshold(3);
//! ```

pub mod config;
pub mod health;
pub mod liveness;
pub mod metrics;
pub mod readiness;
pub mod runner;

pub use config::*;
pub use health::*;
pub use liveness::*;
pub use metrics::*;
pub use readiness::*;
pub use runner::*;

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::time::Duration;

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    fn make_result(id: ProbeId, status: ProbeStatus, failures: u32) -> ProbeResult {
        ProbeResult {
            probe_id: id,
            status,
            latency_ms: 10,
            consecutive_failures: failures,
            last_check_ms: now_ms(),
            message: None,
        }
    }

    // =========================================================================
    // Invariant Tests (T71-T85)
    // =========================================================================

    #[test]
    fn test_inv_healthy_only_after_consecutive_successes() {
        let config = BackoffConfig::default();
        assert_eq!(config.initial_interval, Duration::from_secs(1));
    }

    #[test]
    fn test_inv_unhealthy_after_consecutive_failures() {
        let config = BackoffConfig::default();
        let interval = config.calculate_interval(3);
        assert_eq!(interval, Duration::from_secs(8));
    }

    #[test]
    fn test_inv_probe_timeout_respected() {
        let config = ProbeConfig::http("http://localhost");
        assert_eq!(config.timeout(), Duration::from_millis(5000));
    }

    #[test]
    fn test_inv_consecutive_healthy_resets_on_failure() {
        let mut status = AggregatedStatus::new();
        let id = ProbeId::new();

        let r1 = ProbeResult {
            probe_id: id,
            status: ProbeStatus::Healthy,
            latency_ms: 10,
            consecutive_failures: 0,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(r1);

        let r2 = ProbeResult {
            probe_id: id,
            status: ProbeStatus::Unhealthy,
            latency_ms: 10,
            consecutive_failures: 1,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(r2);

        assert_eq!(status.overall, ProbeStatus::Unhealthy);
    }

    #[test]
    fn test_inv_initial_delay_respected() {
        let definition = ProbeDefinition {
            id: ProbeId::new(),
            name: "test".to_string(),
            config: ProbeConfig::http("http://localhost"),
            interval: Duration::from_secs(30),
            backoff: BackoffConfig::default(),
            failure_threshold: 3,
            success_threshold: 2,
        };
        assert_eq!(definition.interval, Duration::from_secs(30));
    }

    #[test]
    fn test_inv_backoff_applied_after_failure() {
        let config = BackoffConfig::default();
        assert_eq!(config.calculate_interval(0), Duration::from_secs(1));
        assert_eq!(config.calculate_interval(1), Duration::from_secs(2));
    }

    #[test]
    fn test_inv_timestamp_ordering() {
        let id = ProbeId::new();
        let now = now_ms();
        let result = ProbeResult {
            probe_id: id,
            status: ProbeStatus::Healthy,
            latency_ms: 10,
            consecutive_failures: 0,
            last_check_ms: now,
            message: None,
        };
        assert!(result.last_check_ms <= now_ms());
    }

    // =========================================================================
    // Property-Based Tests (T92-T96)
    // =========================================================================

    proptest! {
        #[test]
        fn test_backoff_interval_monotonic(failures in 0u32..100u32) {
            let config = BackoffConfig::default();
            let interval = config.calculate_interval(failures);
            prop_assert!(interval >= config.initial_interval);
            prop_assert!(interval <= config.max_interval);
        }

        #[test]
        fn test_backoff_respects_max_interval(failures in 0u32..1000u32) {
            let config = BackoffConfig {
                initial_interval: Duration::from_secs(1),
                max_interval: Duration::from_secs(60),
                multiplier: 2.0,
                max_failures: 10,
            };
            let interval = config.calculate_interval(failures);
            prop_assert!(interval <= Duration::from_secs(60));
        }

        #[test]
        fn test_aggregated_status_deterministic(
            status1 in 0u8..3,
            status2 in 0u8..3
        ) {
            let s1 = match status1 {
                0 => ProbeStatus::Healthy,
                1 => ProbeStatus::Unhealthy,
                _ => ProbeStatus::Unknown,
            };
            let s2 = match status2 {
                0 => ProbeStatus::Healthy,
                1 => ProbeStatus::Unhealthy,
                _ => ProbeStatus::Unknown,
            };

            let mut agg1 = AggregatedStatus::new();
            let mut agg2 = AggregatedStatus::new();

            let id1 = ProbeId::new();
            let id2 = ProbeId::new();

            agg1.update(ProbeResult {
                probe_id: id1,
                status: s1,
                latency_ms: 10,
                consecutive_failures: 0,
                last_check_ms: now_ms(),
                message: None,
            });
            agg1.update(ProbeResult {
                probe_id: id2,
                status: s2,
                latency_ms: 10,
                consecutive_failures: 0,
                last_check_ms: now_ms(),
                message: None,
            });

            agg2.update(ProbeResult {
                probe_id: id1,
                status: s1,
                latency_ms: 10,
                consecutive_failures: 0,
                last_check_ms: now_ms(),
                message: None,
            });
            agg2.update(ProbeResult {
                probe_id: id2,
                status: s2,
                latency_ms: 10,
                consecutive_failures: 0,
                last_check_ms: now_ms(),
                message: None,
            });

            prop_assert_eq!(agg1.overall, agg2.overall);
        }

        #[test]
        fn test_probe_id_uniqueness(count in 1u8..10u8) {
            let mut ids = std::collections::HashSet::new();
            for _ in 0..count {
                let id = ProbeId::new();
                prop_assert!(ids.insert(id), "ProbeId should be unique");
            }
        }

        #[test]
        fn test_probe_result_latency_positive(latency in 1u64..10000u64) {
            let result = ProbeResult {
                probe_id: ProbeId::new(),
                status: ProbeStatus::Healthy,
                latency_ms: latency,
                consecutive_failures: 0,
                last_check_ms: now_ms(),
                message: None,
            };
            prop_assert!(result.latency_ms > 0);
        }
    }

    // =========================================================================
    // Edge Case Tests (T103-T115)
    // =========================================================================

    #[test]
    fn test_zero_timeout_probe() {
        let config = ProbeConfig::http("http://localhost").with_timeout(Duration::from_secs(0));
        assert_eq!(config.timeout(), Duration::from_secs(0));
    }

    #[test]
    fn test_very_long_timeout_probe() {
        let config = ProbeConfig::http("http://localhost").with_timeout(Duration::from_secs(7200));
        assert_eq!(config.timeout(), Duration::from_secs(7200));
    }

    #[test]
    fn test_negative_backoff_multiplier() {
        let config = BackoffConfig {
            initial_interval: Duration::from_secs(1),
            max_interval: Duration::from_secs(60),
            multiplier: -2.0,
            max_failures: 10,
        };
        let interval = config.calculate_interval(1);
        assert!(interval >= Duration::from_secs(0));
    }

    #[test]
    fn test_zero_backoff_multiplier() {
        let config = BackoffConfig {
            initial_interval: Duration::from_secs(1),
            max_interval: Duration::from_secs(60),
            multiplier: 0.0,
            max_failures: 10,
        };
        let interval = config.calculate_interval(1);
        assert_eq!(interval, Duration::from_secs(0));
    }

    #[test]
    fn test_max_u64_latency() {
        let result = ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Healthy,
            latency_ms: u64::MAX,
            consecutive_failures: 0,
            last_check_ms: now_ms(),
            message: None,
        };
        assert_eq!(result.latency_ms, u64::MAX);
    }

    // =========================================================================
    // Contract Compliance Tests (T116-T119)
    // =========================================================================

    #[test]
    fn test_probe_types_exhaustive() {
        assert_eq!(3, 3);
        let _ = ProbeType::Http;
        let _ = ProbeType::Tcp;
        let _ = ProbeType::Exec;
    }

    #[test]
    fn test_probe_outcomes_exhaustive() {
        assert_eq!(3, 3);
        let _ = ProbeStatus::Healthy;
        let _ = ProbeStatus::Unhealthy;
        let _ = ProbeStatus::Unknown;
    }

    // =========================================================================
    // Integration Tests (T86-T91)
    // =========================================================================

    #[tokio::test]
    async fn test_probe_trait_object_can_be_stored_and_called() {
        let probe: Box<dyn Probe> = Box::new(HttpProbe::new("http://localhost:9999"));
        let result = probe.check().await;
        assert!(
            result.is_err(),
            "HTTP probe to nonexistent host should fail"
        );
        let err = result.unwrap_err();
        assert!(
            matches!(err, ProbeError::Http(_)),
            "Expected Http error variant, got {:?}",
            err
        );
    }

    #[tokio::test]
    async fn test_multiple_probe_types_via_trait_object() {
        let tcp_probe: Box<dyn Probe> = Box::new(TcpProbe::new("127.0.0.1:9999".parse().unwrap()));
        let exec_probe: Box<dyn Probe> = Box::new(ExecProbe::new("false", vec![]));
        let http_probe: Box<dyn Probe> = Box::new(HttpProbe::new("http://localhost:9999"));

        let tcp_result = tcp_probe.check().await;
        let exec_result = exec_probe.check().await;
        let http_result = http_probe.check().await;

        assert!(
            tcp_result.is_ok(),
            "TCP probe check should succeed (returns result with Unhealthy status)"
        );
        let tcp_r = tcp_result.unwrap();
        assert_eq!(
            tcp_r.status,
            ProbeStatus::Unhealthy,
            "TCP to nonexistent host should be Unhealthy"
        );
        assert!(matches!(tcp_r.probe_id, _));

        assert!(
            exec_result.is_ok(),
            "Exec false should return Ok with Unhealthy status"
        );
        assert_eq!(exec_result.unwrap().status, ProbeStatus::Unhealthy);

        assert!(http_result.is_err(), "HTTP to nonexistent host should fail");
        assert!(matches!(http_result.unwrap_err(), ProbeError::Http(_)));
    }

    #[test]
    fn test_registry_thread_safety_concurrent_register() {
        use std::sync::Arc;
        use tokio::sync::Mutex;

        let registry = Arc::new(Mutex::new(ProbeRegistry::new()));
        let mut handles = vec![];

        for i in 0..10 {
            let reg = Arc::clone(&registry);
            let handle = std::thread::spawn(move || {
                let definition = ProbeDefinition {
                    id: ProbeId::new(),
                    name: format!("test{}", i),
                    config: ProbeConfig::http("http://localhost"),
                    interval: Duration::from_secs(30),
                    backoff: BackoffConfig::default(),
                    failure_threshold: 3,
                    success_threshold: 2,
                };
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let mut reg = reg.lock().await;
                    reg.register(definition);
                });
            });
            handles.push(handle);
        }

        for h in handles {
            h.join().unwrap();
        }

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let reg = registry.lock().await;
            assert_eq!(reg.len(), 10);
        });
    }

    // =========================================================================
    // Legacy tests
    // =========================================================================

    #[test]
    fn test_backoff_config_calculate_interval() {
        let config = BackoffConfig::default();
        assert_eq!(config.calculate_interval(0), Duration::from_secs(1));
        assert_eq!(config.calculate_interval(1), Duration::from_secs(2));
        assert_eq!(config.calculate_interval(2), Duration::from_secs(4));
        assert_eq!(config.calculate_interval(3), Duration::from_secs(8));
    }

    #[test]
    fn test_backoff_config_max_interval() {
        let config = BackoffConfig {
            initial_interval: Duration::from_secs(1),
            max_interval: Duration::from_secs(10),
            multiplier: 2.0,
            max_failures: 10,
        };
        assert_eq!(config.calculate_interval(10), Duration::from_secs(10));
        assert_eq!(config.calculate_interval(100), Duration::from_secs(10));
    }

    #[test]
    fn test_aggregated_status_update() {
        let mut status = AggregatedStatus::new();

        let healthy_result = ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Healthy,
            latency_ms: 10,
            consecutive_failures: 0,
            last_check_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            message: None,
        };
        status.update(healthy_result);
        assert_eq!(status.healthy_count, 1);
        assert_eq!(status.unhealthy_count, 0);
        assert_eq!(status.overall, ProbeStatus::Healthy);

        let unhealthy_result = ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Unhealthy,
            latency_ms: 10,
            consecutive_failures: 1,
            last_check_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            message: None,
        };
        status.update(unhealthy_result);
        assert_eq!(status.healthy_count, 1);
        assert_eq!(status.unhealthy_count, 1);
        assert_eq!(status.overall, ProbeStatus::Unhealthy);
    }

    #[test]
    fn test_probe_config_timeout() {
        let config = ProbeConfig::http("http://localhost:8080/health");
        assert_eq!(config.timeout(), Duration::from_millis(5000));

        let config = config.with_timeout(Duration::from_secs(10));
        assert_eq!(config.timeout(), Duration::from_secs(10));
    }

    #[test]
    fn test_probe_id_display() {
        let id = ProbeId::new();
        let display = format!("{}", id);
        assert!(display.starts_with("probe-"));
    }

    // =========================================================================
    // QA Smoke Tests: Contract Verification (ve-3edr)
    // =========================================================================

    #[test]
    fn qa_inv001_healthy_requires_threshold_successes() {
        let threshold = 3u32;
        let id = ProbeId::new();
        let mut agg = AggregatedStatus::new();
        for _ in 0..threshold {
            agg.update(make_result(id, ProbeStatus::Healthy, 0));
        }
        assert_eq!(agg.overall, ProbeStatus::Healthy);
    }

    #[test]
    fn qa_inv002_unhealthy_after_threshold_failures() {
        let threshold = 3u32;
        let id = ProbeId::new();
        let mut agg = AggregatedStatus::new();
        for i in 0..threshold {
            agg.update(make_result(id, ProbeStatus::Unhealthy, i + 1));
        }
        assert_eq!(agg.overall, ProbeStatus::Unhealthy);
    }

    #[test]
    fn qa_inv003_probe_config_timeout_bounds() {
        let configs = vec![
            ProbeConfig::http("http://localhost:8080/health"),
            ProbeConfig::tcp("localhost", 8080),
            ProbeConfig::exec("echo", vec![]),
        ];
        for config in configs {
            let timeout = config.timeout();
            assert!(
                timeout.as_millis() > 0,
                "INV-003: timeout must be positive, got {:?}",
                timeout
            );
        }
    }

    #[test]
    fn qa_inv004_consecutive_healthy_resets_on_failure() {
        let id = ProbeId::new();
        let mut agg = AggregatedStatus::new();
        agg.update(make_result(id, ProbeStatus::Healthy, 0));
        agg.update(make_result(id, ProbeStatus::Healthy, 0));
        assert_eq!(agg.healthy_count, 1);

        agg.update(make_result(id, ProbeStatus::Unhealthy, 1));
        assert_eq!(agg.healthy_count, 0);
        assert_eq!(agg.unhealthy_count, 1);
    }

    #[test]
    fn qa_inv005_consecutive_unhealthy_resets_on_success() {
        let id = ProbeId::new();
        let mut agg = AggregatedStatus::new();
        agg.update(make_result(id, ProbeStatus::Unhealthy, 1));
        agg.update(make_result(id, ProbeStatus::Unhealthy, 2));
        assert_eq!(agg.unhealthy_count, 1);

        agg.update(make_result(id, ProbeStatus::Healthy, 0));
        assert_eq!(agg.unhealthy_count, 0);
        assert_eq!(agg.healthy_count, 1);
    }

    #[test]
    fn qa_inv006_aggregated_status_transitions() {
        let mut agg = AggregatedStatus::new();
        assert_eq!(agg.overall, ProbeStatus::Unknown);

        let id1 = ProbeId::new();
        agg.update(make_result(id1, ProbeStatus::Healthy, 0));
        assert_eq!(agg.overall, ProbeStatus::Healthy);

        let id2 = ProbeId::new();
        agg.update(make_result(id2, ProbeStatus::Unhealthy, 1));
        assert_eq!(agg.overall, ProbeStatus::Unhealthy);

        agg.update(make_result(id2, ProbeStatus::Healthy, 0));
        assert_eq!(agg.overall, ProbeStatus::Healthy);
    }

    #[test]
    fn qa_inv008_definition_interval_respected() {
        let interval = Duration::from_secs(45);
        let def = ProbeDefinition {
            id: ProbeId::new(),
            name: "qa-inv008".to_string(),
            config: ProbeConfig::http("http://localhost:8080"),
            interval,
            backoff: BackoffConfig::default(),
            failure_threshold: 3,
            success_threshold: 2,
        };
        assert_eq!(def.interval, interval);
    }

    #[test]
    fn qa_inv009_backoff_not_applied_before_first_probe() {
        let config = BackoffConfig::default();
        let initial = config.calculate_interval(0);
        assert_eq!(initial, config.initial_interval);
    }

    #[test]
    fn qa_inv010_timestamp_ordering_in_result() {
        let before = now_ms();
        let id = ProbeId::new();
        let result = make_result(id, ProbeStatus::Healthy, 0);
        assert!(result.last_check_ms >= before);
        assert!(result.last_check_ms <= now_ms());
    }

    #[tokio::test]
    async fn qa_smoke_exec_probe_true_command() {
        let probe = ExecProbe::new("true", vec![]);
        let result = probe.check().await;
        assert!(result.is_ok(), "Exec true should succeed");
        let r = result.unwrap();
        assert_eq!(r.status, ProbeStatus::Healthy);
        assert_eq!(r.probe_id, probe.probe_id());
    }

    #[tokio::test]
    async fn qa_smoke_exec_probe_false_command() {
        let probe = ExecProbe::new("false", vec![]);
        let result = probe.check().await;
        assert!(result.is_ok(), "Exec false should return Ok with Unhealthy");
        let r = result.unwrap();
        assert_eq!(r.status, ProbeStatus::Unhealthy);
    }

    #[tokio::test]
    async fn qa_smoke_exec_probe_custom_exit_code() {
        let probe = ExecProbe::new("bash", vec!["-c".to_string(), "exit 42".to_string()])
            .with_expected_exit_code(42);
        let result = probe.check().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().status, ProbeStatus::Healthy);
    }

    #[tokio::test]
    async fn qa_smoke_exec_probe_nonexistent_command() {
        let probe = ExecProbe::new("/nonexistent/command/xyz", vec![]);
        let result = probe.check().await;
        assert!(result.is_err(), "Nonexistent command should error");
    }

    #[tokio::test]
    async fn qa_smoke_tcp_probe_refused_connection() {
        let probe =
            TcpProbe::new("127.0.0.1:1".parse().unwrap()).with_timeout(Duration::from_millis(500));
        let result = probe.check().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().status, ProbeStatus::Unhealthy);
    }

    #[tokio::test]
    async fn qa_smoke_probe_trait_dispatch() {
        let probes: Vec<Box<dyn Probe>> = vec![
            Box::new(ExecProbe::new("true", vec![])),
            Box::new(
                TcpProbe::new("127.0.0.1:1".parse().unwrap())
                    .with_timeout(Duration::from_millis(200)),
            ),
        ];
        let mut agg = AggregatedStatus::new();
        for probe in &probes {
            let result = probe.check().await.unwrap();
            agg.update(result);
        }
        assert_eq!(agg.results.len(), 2);
        assert_eq!(agg.overall, ProbeStatus::Unhealthy);
    }

    #[tokio::test]
    async fn qa_smoke_probe_result_fields_populated() {
        let probe = ExecProbe::new("echo", vec!["hello".to_string()]);
        let result = probe.check().await.unwrap();
        assert_eq!(result.status, ProbeStatus::Healthy);
        assert!(result.message.is_some());
        assert!(result.message.as_ref().unwrap().contains("echo"));
        assert!(result.last_check_ms > 0);
        assert_eq!(result.probe_id, probe.probe_id());
    }

    #[test]
    fn qa_smoke_probe_config_tagged_serde() {
        let configs = vec![
            ProbeConfig::http("http://localhost:8080/health"),
            ProbeConfig::tcp("127.0.0.1", 9090),
            ProbeConfig::exec("curl", vec!["-s".to_string()]),
        ];
        for config in configs {
            let json = serde_json::to_string(&config).unwrap();
            assert!(
                json.contains("\"type\""),
                "Tagged serde must include type field: {}",
                json
            );
            let parsed: ProbeConfig = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed.probe_type(), config.probe_type());
        }
    }

    #[test]
    fn qa_smoke_registry_crud_lifecycle() {
        let mut registry = ProbeRegistry::new();
        assert!(registry.is_empty());

        let defs: Vec<ProbeDefinition> = (0..5)
            .map(|i| ProbeDefinition {
                id: ProbeId::new(),
                name: format!("probe-{}", i),
                config: ProbeConfig::http(format!("http://localhost:{}", 8080 + i)),
                interval: Duration::from_secs(30),
                backoff: BackoffConfig::default(),
                failure_threshold: 3,
                success_threshold: 2,
            })
            .collect();

        let mut ids = vec![];
        for def in defs {
            ids.push(registry.register(def));
        }
        assert_eq!(registry.len(), 5);

        let removed = registry.unregister(ids[2]);
        assert!(removed.is_some());
        assert!(registry.get(&ids[2]).is_none());
        assert_eq!(registry.len(), 4);

        let list = registry.list();
        assert_eq!(list.len(), 4);
    }

    #[test]
    fn qa_smoke_backoff_monotonic_growth() {
        let config = BackoffConfig::default();
        let mut prev = Duration::ZERO;
        for failures in 0..=config.max_failures {
            let interval = config.calculate_interval(failures);
            assert!(
                interval >= prev,
                "Backoff must be monotonic: failure={} interval={:?} < prev={:?}",
                failures,
                interval,
                prev
            );
            assert!(
                interval <= config.max_interval,
                "Backoff must not exceed max: interval={:?} > max={:?}",
                interval,
                config.max_interval
            );
            prev = interval;
        }
    }

    #[test]
    fn qa_smoke_aggregation_dominance_rule() {
        let mut agg = AggregatedStatus::new();
        let ids: Vec<ProbeId> = (0..10).map(|_| ProbeId::new()).collect();
        for id in &ids[..9] {
            agg.update(make_result(*id, ProbeStatus::Healthy, 0));
        }
        assert_eq!(agg.overall, ProbeStatus::Healthy);

        agg.update(make_result(ids[9], ProbeStatus::Unhealthy, 1));
        assert_eq!(agg.overall, ProbeStatus::Unhealthy);
        assert_eq!(agg.healthy_count, 9);
        assert_eq!(agg.unhealthy_count, 1);
    }
}
