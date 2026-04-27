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

pub mod probes;
pub mod types;

#[cfg(test)]
mod integration_tests;
#[cfg(test)]
mod proptest;
#[cfg(test)]
mod qa_smoke;

pub use probes::*;
pub use types::*;
