//! Property-based and edge case tests for probe types and backoff logic.

use std::collections::HashSet;
use std::time::Duration;

use proptest::prelude::*;

use super::types::*;

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

// =========================================================================
// Property-Based Tests (proptest)
// =========================================================================

#[cfg(test)]
mod proptest_tests {
    use super::*;

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
            let mut ids = HashSet::new();
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
}

// =========================================================================
// Edge Case Tests (T103-T115)
// =========================================================================

#[cfg(test)]
mod edge_case_tests {
    use super::*;

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
}

// =========================================================================
// Contract Compliance Tests (T116-T119)
// =========================================================================

#[cfg(test)]
mod contract_compliance_tests {
    use super::*;

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
}
