//! QA smoke tests: contract verification and probe behavior validation.

use std::time::Duration;

use super::types::*;
use super::probes::*;

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn make_result(id: ProbeId, status: ProbeStatus, failures: u32) -> ProbeResult {
    ProbeResult { probe_id: id, status, latency_ms: 10, consecutive_failures: failures,
        last_check_ms: now_ms(), message: None }
}

// =========================================================================
// QA Smoke Tests: Contract Verification (ve-3edr)
// =========================================================================

#[cfg(test)]
mod qa_smoke_tests {
    use super::*;

    #[test]
    fn qa_inv001_healthy_requires_threshold_successes() {
        let mut agg = AggregatedStatus::new();
        for _ in 0..3 { agg.update(make_result(ProbeId::new(), ProbeStatus::Healthy, 0)); }
        assert_eq!(agg.overall, ProbeStatus::Healthy);
    }

    #[test]
    fn qa_inv002_unhealthy_after_threshold_failures() {
        let mut agg = AggregatedStatus::new();
        for i in 0..3 { agg.update(make_result(ProbeId::new(), ProbeStatus::Unhealthy, i + 1)); }
        assert_eq!(agg.overall, ProbeStatus::Unhealthy);
    }

    #[test]
    fn qa_inv003_probe_config_timeout_bounds() {
        for config in [ProbeConfig::http("http://localhost:8080/health"),
                       ProbeConfig::tcp("localhost", 8080), ProbeConfig::exec("echo", vec![])] {
            let timeout = config.timeout();
            assert!(timeout.as_millis() > 0, "INV-003: timeout must be positive, got {:?}", timeout);
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
        let def = ProbeDefinition { id: ProbeId::new(), name: "qa-inv008".to_string(),
            config: ProbeConfig::http("http://localhost:8080"), interval, backoff: BackoffConfig::default(),
            failure_threshold: 3, success_threshold: 2 };
        assert_eq!(def.interval, interval);
    }

    #[test]
    fn qa_inv009_backoff_not_applied_before_first_probe() {
        let config = BackoffConfig::default();
        assert_eq!(config.calculate_interval(0), config.initial_interval);
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
        assert_eq!(result.unwrap().status, ProbeStatus::Unhealthy);
    }

    #[tokio::test]
    async fn qa_smoke_exec_probe_custom_exit_code() {
        let probe = ExecProbe::new("bash", vec!["-c".to_string(), "exit 42".to_string()]).with_expected_exit_code(42);
        assert!(probe.check().await.is_ok());
        assert_eq!(probe.check().await.unwrap().status, ProbeStatus::Healthy);
    }

    #[tokio::test]
    async fn qa_smoke_exec_probe_nonexistent_command() {
        assert!(ExecProbe::new("/nonexistent/command/xyz", vec![]).check().await.is_err());
    }

    #[tokio::test]
    async fn qa_smoke_tcp_probe_refused_connection() {
        let probe = TcpProbe::new("127.0.0.1:1".parse().unwrap()).with_timeout(Duration::from_millis(500));
        assert!(probe.check().await.is_ok());
        assert_eq!(probe.check().await.unwrap().status, ProbeStatus::Unhealthy);
    }

    #[tokio::test]
    async fn qa_smoke_probe_trait_dispatch() {
        let probes: Vec<Box<dyn Probe>> = vec![
            Box::new(ExecProbe::new("true", vec![])),
            Box::new(TcpProbe::new("127.0.0.1:1".parse().unwrap()).with_timeout(Duration::from_millis(200))),
        ];
        let mut agg = AggregatedStatus::new();
        for probe in &probes { agg.update(probe.check().await.unwrap()); }
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
        for config in [ProbeConfig::http("http://localhost:8080/health"),
                       ProbeConfig::tcp("127.0.0.1", 9090), ProbeConfig::exec("curl", vec!["-s".to_string()])] {
            let json = serde_json::to_string(&config).unwrap();
            assert!(json.contains("\"type\""), "Tagged serde must include type field: {}", json);
            let parsed: ProbeConfig = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed.probe_type(), config.probe_type());
        }
    }

    #[test]
    fn qa_smoke_registry_crud_lifecycle() {
        let mut registry = ProbeRegistry::new();
        assert!(registry.is_empty());
        let defs: Vec<ProbeDefinition> = (0..5).map(|i| ProbeDefinition {
            id: ProbeId::new(), name: format!("probe-{}", i),
            config: ProbeConfig::http(format!("http://localhost:{}", 8080 + i)),
            interval: Duration::from_secs(30), backoff: BackoffConfig::default(),
            failure_threshold: 3, success_threshold: 2,
        }).collect();
        let mut ids = vec![];
        for def in defs { ids.push(registry.register(def)); }
        assert_eq!(registry.len(), 5);
        assert!(registry.unregister(ids[2]).is_some());
        assert!(registry.get(&ids[2]).is_none());
        assert_eq!(registry.len(), 4);
        assert_eq!(registry.list().len(), 4);
    }

    #[test]
    fn qa_smoke_backoff_monotonic_growth() {
        let config = BackoffConfig::default();
        let mut prev = Duration::ZERO;
        for failures in 0..=config.max_failures {
            let interval = config.calculate_interval(failures);
            assert!(interval >= prev, "Backoff monotonic: failure={} {:?} < {:?}", failures, interval, prev);
            assert!(interval <= config.max_interval, "Backoff <= max: {:?} > {:?}", interval, config.max_interval);
            prev = interval;
        }
    }

    #[test]
    fn qa_smoke_aggregation_dominance_rule() {
        let mut agg = AggregatedStatus::new();
        let ids: Vec<ProbeId> = (0..10).map(|_| ProbeId::new()).collect();
        for id in &ids[..9] { agg.update(make_result(*id, ProbeStatus::Healthy, 0)); }
        assert_eq!(agg.overall, ProbeStatus::Healthy);
        agg.update(make_result(ids[9], ProbeStatus::Unhealthy, 1));
        assert_eq!(agg.overall, ProbeStatus::Unhealthy);
        assert_eq!(agg.healthy_count, 9);
        assert_eq!(agg.unhealthy_count, 1);
    }
}
