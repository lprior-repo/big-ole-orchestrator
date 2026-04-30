//! Heartbeat watcher for actor health monitoring.
//!
//! Per ADR-012: The heartbeat watcher monitors actor health and handles
//! graceful shutdown when actors become unresponsive.
//!
//! # Architecture
//!
//! The watcher periodically checks actor health via configured probes.
//! When consecutive failures exceed the threshold, it triggers graceful
//! shutdown via the lifecycle module.

mod config;
mod detector;
mod runner;
mod watcher;

#[cfg(test)]
mod tests;

pub use config::{HeartbeatWatcherConfig, InstanceIdOwned, ShutdownCallback, ShutdownError};
pub use detector::ActorHealthState;
pub use runner::{HeartbeatError, HeartbeatWatcher, run_heartbeat_watcher};
