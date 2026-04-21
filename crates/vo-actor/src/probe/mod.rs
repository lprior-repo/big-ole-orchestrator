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

pub mod types;
pub mod http_probe;
pub mod tcp_probe;
pub mod exec_probe;
pub mod registry;
pub mod error;

// Re-export main items
pub use error::{Probe, ProbeError};
pub use http_probe::HttpProbe;
pub use tcp_probe::TcpProbe;
pub use exec_probe::ExecProbe;
pub use registry::ProbeRegistry;
pub use types::{
    ProbeType, ProbeStatus, ProbeResult, ProbeId, ProbeDefinition, BackoffConfig, AggregatedStatus,
};


