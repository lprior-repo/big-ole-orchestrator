use std::collections::HashMap;

use super::metrics::{ProbeId, ProbeResult, ProbeStatus};

#[derive(Debug, Clone)]
pub struct AggregatedStatus {
    pub overall: ProbeStatus,
    pub healthy_count: u32,
    pub unhealthy_count: u32,
    pub unknown_count: u32,
    pub results: HashMap<ProbeId, ProbeResult>,
}

impl AggregatedStatus {
    pub fn new() -> Self {
        Self {
            overall: ProbeStatus::Unknown,
            healthy_count: 0,
            unhealthy_count: 0,
            unknown_count: 0,
            results: HashMap::new(),
        }
    }

    pub fn update(&mut self, result: ProbeResult) {
        if let Some(old_result) = self.results.get(&result.probe_id) {
            match old_result.status {
                ProbeStatus::Healthy => self.healthy_count -= 1,
                ProbeStatus::Unhealthy => self.unhealthy_count -= 1,
                ProbeStatus::Unknown => self.unknown_count -= 1,
            }
        }
        match result.status {
            ProbeStatus::Healthy => self.healthy_count += 1,
            ProbeStatus::Unhealthy => self.unhealthy_count += 1,
            ProbeStatus::Unknown => self.unknown_count += 1,
        }
        self.results.insert(result.probe_id, result);

        self.overall = if self.unhealthy_count > 0 {
            ProbeStatus::Unhealthy
        } else if self.healthy_count > 0 && self.unknown_count == 0 {
            ProbeStatus::Healthy
        } else {
            ProbeStatus::Unknown
        };
    }

    pub fn is_healthy(&self) -> bool {
        self.overall == ProbeStatus::Healthy
    }
}

impl Default for AggregatedStatus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    #[test]
    fn test_aggregated_status_new_initializes_unknown() {
        let status = AggregatedStatus::new();
        assert_eq!(status.overall, ProbeStatus::Unknown);
        assert_eq!(status.healthy_count, 0);
        assert_eq!(status.unhealthy_count, 0);
        assert_eq!(status.unknown_count, 0);
    }

    #[test]
    fn test_aggregated_status_update_healthy() {
        let mut status = AggregatedStatus::new();
        let result = ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Healthy,
            latency_ms: 10,
            consecutive_failures: 0,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(result);
        assert_eq!(status.healthy_count, 1);
        assert_eq!(status.unhealthy_count, 0);
        assert_eq!(status.unknown_count, 0);
    }

    #[test]
    fn test_aggregated_status_update_unhealthy() {
        let mut status = AggregatedStatus::new();
        let result = ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Unhealthy,
            latency_ms: 10,
            consecutive_failures: 1,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(result);
        assert_eq!(status.unhealthy_count, 1);
    }

    #[test]
    fn test_aggregated_status_update_unknown() {
        let mut status = AggregatedStatus::new();
        let result = ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Unknown,
            latency_ms: 0,
            consecutive_failures: 0,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(result);
        assert_eq!(status.unknown_count, 1);
    }

    #[test]
    fn test_aggregated_status_overall_unhealthy_when_any_probe_unhealthy() {
        let mut status = AggregatedStatus::new();

        let healthy = ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Healthy,
            latency_ms: 10,
            consecutive_failures: 0,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(healthy);

        let unhealthy = ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Unhealthy,
            latency_ms: 10,
            consecutive_failures: 1,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(unhealthy);

        assert_eq!(status.overall, ProbeStatus::Unhealthy);
    }

    #[test]
    fn test_aggregated_status_overall_healthy_when_all_healthy_and_none_unknown() {
        let mut status = AggregatedStatus::new();

        let h1 = ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Healthy,
            latency_ms: 10,
            consecutive_failures: 0,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(h1);

        let h2 = ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Healthy,
            latency_ms: 10,
            consecutive_failures: 0,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(h2);

        assert_eq!(status.overall, ProbeStatus::Healthy);
    }

    #[test]
    fn test_aggregated_status_overall_unknown_when_mix_of_healthy_and_unknown() {
        let mut status = AggregatedStatus::new();

        let healthy = ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Healthy,
            latency_ms: 10,
            consecutive_failures: 0,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(healthy);

        let unknown = ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Unknown,
            latency_ms: 0,
            consecutive_failures: 0,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(unknown);

        assert_eq!(status.overall, ProbeStatus::Unknown);
    }

    #[test]
    fn test_aggregated_status_is_healthy() {
        let mut status = AggregatedStatus::new();
        assert!(!status.is_healthy());

        let healthy = ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Healthy,
            latency_ms: 10,
            consecutive_failures: 0,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(healthy);
        assert!(status.is_healthy());

        let unhealthy = ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Unhealthy,
            latency_ms: 10,
            consecutive_failures: 1,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(unhealthy);
        assert!(!status.is_healthy());
    }

    #[test]
    fn test_aggregated_status_update_replaces_previous_for_same_probe() {
        let mut status = AggregatedStatus::new();
        let probe_id = ProbeId::new();

        let r1 = ProbeResult {
            probe_id,
            status: ProbeStatus::Healthy,
            latency_ms: 10,
            consecutive_failures: 0,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(r1);
        assert_eq!(status.healthy_count, 1);

        let r2 = ProbeResult {
            probe_id,
            status: ProbeStatus::Unhealthy,
            latency_ms: 20,
            consecutive_failures: 1,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(r2);
        assert_eq!(status.healthy_count, 0);
        assert_eq!(status.unhealthy_count, 1);
        assert_eq!(status.results.len(), 1);
    }

    #[test]
    fn test_aggregated_status_multiple_probes_tracked() {
        let mut status = AggregatedStatus::new();

        for i in 0..5 {
            let result = ProbeResult {
                probe_id: ProbeId::new(),
                status: ProbeStatus::Healthy,
                latency_ms: 10 + i as u64,
                consecutive_failures: 0,
                last_check_ms: now_ms(),
                message: None,
            };
            status.update(result);
        }

        assert_eq!(status.results.len(), 5);
        assert_eq!(status.healthy_count, 5);
    }
}
