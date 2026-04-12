//! Async Process Supervisor Module
//!
//! This module provides the async process supervisor implementation for
//! subprocess lifecycle management in the veloxide distributed worker system.

mod port;

pub use port::{
    calculate_backoff_delay, is_zombie_state, should_respawn, Counter, ProcessHandle, ProcessManager,
    ProcessSupervisorError, ProcessSupervisorMetrics, SpawnPhase, SpawnRecord, SpawnStorage,
    WorkQueue,
};