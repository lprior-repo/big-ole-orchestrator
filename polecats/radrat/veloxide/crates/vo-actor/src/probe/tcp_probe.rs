use std::net::SocketAddr;
use std::time::Duration;

use async_trait::async_trait;

use super::types::{Probe, ProbeError, ProbeId, ProbeResult, ProbeStatus};

pub struct TcpProbe {
    id: ProbeId,
    address: SocketAddr,
    timeout: Duration,
}

impl TcpProbe {
    pub fn new(address: SocketAddr) -> Self {
        Self {
            id: ProbeId::new(),
            address,
            timeout: Duration::from_secs(5),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[async_trait]
impl Probe for TcpProbe {
    async fn check(&self) -> Result<ProbeResult, ProbeError> {
        let start = tokio::time::Instant::now();

        let connection =
            tokio::time::timeout(self.timeout, tokio::net::TcpStream::connect(self.address)).await;

        let latency_ms = start.elapsed().as_millis() as u64;

        let (status, message) = match connection {
            Ok(Ok(_)) => (
                ProbeStatus::Healthy,
                format!("TCP connect to {}", self.address),
            ),
            Ok(Err(e)) => (
                ProbeStatus::Unhealthy,
                format!("TCP failed to {}: {}", self.address, e),
            ),
            Err(_) => (
                ProbeStatus::Unhealthy,
                format!(
                    "TCP connect to {} timed out after {:?}",
                    self.address, self.timeout
                ),
            ),
        };

        Ok(ProbeResult {
            probe_id: self.id,
            status,
            latency_ms,
            consecutive_failures: 0,
            last_check_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            message: Some(message),
        })
    }

    fn probe_id(&self) -> ProbeId {
        self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::types::AggregatedStatus;

    #[tokio::test]
    async fn qa_smoke_tcp_probe_refused_connection() {
        let probe =
            TcpProbe::new("127.0.0.1:1".parse().unwrap()).with_timeout(Duration::from_millis(500));
        let result = probe.check().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().status, ProbeStatus::Unhealthy);
    }

    #[tokio::test]
    async fn test_multiple_probe_types_via_trait_object() {
        let tcp_probe: Box<dyn Probe> = Box::new(TcpProbe::new("127.0.0.1:9999".parse().unwrap()));
        let http_probe: Box<dyn Probe> = Box::new(
            crate::probe::http_probe::HttpProbe::new("http://localhost:9999"),
        );
        let exec_probe: Box<dyn Probe> =
            Box::new(crate::probe::exec_probe::ExecProbe::new("false", vec![]));

        let tcp_result = tcp_probe.check().await;
        let exec_result = exec_probe.check().await;
        let http_result = http_probe.check().await;

        assert!(tcp_result.is_ok());
        let tcp_r = tcp_result.unwrap();
        assert_eq!(tcp_r.status, ProbeStatus::Unhealthy);
        assert!(matches!(tcp_r.probe_id, _));

        assert!(exec_result.is_ok());
        assert_eq!(exec_result.unwrap().status, ProbeStatus::Unhealthy);

        assert!(http_result.is_err());
        assert!(matches!(http_result.unwrap_err(), ProbeError::Http(_)));
    }

    #[tokio::test]
    async fn qa_smoke_probe_trait_dispatch() {
        let probes: Vec<Box<dyn Probe>> = vec![
            Box::new(crate::probe::exec_probe::ExecProbe::new("true", vec![])),
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
}
