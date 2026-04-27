//! Integration tests for probe trait dispatch, thread safety, and legacy behavior.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use super::probes::*;
use super::types::*;

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

// =========================================================================
// Integration Tests (T86-T91)
// =========================================================================

#[cfg(test)]
mod integration_tests {
    use super::*;

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

        assert!(tcp_result.is_ok(), "TCP probe check should succeed");
        assert_eq!(tcp_result.unwrap().status, ProbeStatus::Unhealthy);
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
}

// =========================================================================
// Legacy Tests (from original implementation)
// =========================================================================

#[cfg(test)]
mod legacy_tests {
    use super::*;

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
            last_check_ms: now_ms(),
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
            last_check_ms: now_ms(),
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
}
