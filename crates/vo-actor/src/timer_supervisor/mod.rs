//! Timer Supervisor Module
//!
//! This module implements the timer scanning and dispatch logic with delete-before-dispatch
//! ordering per ADR-005.
//!
//! # Migration Note
//! This module now uses the unified TimerStorage trait from vo_common::ports.
//! The local TimerRecord (with u64 timestamps) has been replaced with the unified
//! TimerRecord (with TimestampMs).
//!
//! # Structure
//!
//! - [`types`] - Type definitions (TimerSupervisorError, etc.) - TimerRecord now from vo_common::ports
//! - [`traits`] - Trait definitions (WorkQueue only - TimerStorage is now from vo_common::ports)
//! - [`calc`] - Pure calculation functions
//! - [`supervisor`] - Main supervisor actor implementation
//!
//! WorkQueue is shared in [crate::work_queue].

pub mod calc;
pub mod supervisor;
pub mod traits;
pub mod types;

// Re-export commonly used types
pub use calc::{is_overdue, verify_dual_clock};
pub use supervisor::{TimerSupervisor, TimerSupervisorHandle};
pub use traits::WorkQueue;
pub use types::{
    Counter, CycleResult, TimerRecord, TimerSupervisorError, TimerSupervisorMetrics,
    TimerSupervisorState,
};
// Re-export TimerStorage from vo_common::ports for compatibility
pub use vo_common::ports::TimerStorage;
