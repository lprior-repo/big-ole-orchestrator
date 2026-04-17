//! Timer Supervisor Module
//!
//! This module implements the timer scanning and dispatch logic with dual-clock
//! verification and delete-before-dispatch ordering per ADR-005 and ADR-013.
//!
//! # Structure
//!
//! - [`types`] - Type definitions (TimerRecord, TimerSupervisorError, etc.)
//! - [`traits`] - Trait definitions (TimerStorage, WorkQueue)
//! - [`calc`] - Pure calculation functions
//! - [`supervisor`] - Main supervisor actor implementation

pub mod calc;
pub mod supervisor;
pub mod traits;
pub mod types;

// Re-export commonly used types
pub use calc::{is_overdue, timer_delete_before_dispatch, verify_dual_clock};
pub use supervisor::{TimerSupervisor, TimerSupervisorHandle};
pub use traits::{TimerStorage, WorkQueue};
pub use types::{
    Counter, CycleResult, TimerRecord, TimerSupervisorError, TimerSupervisorMetrics,
    TimerSupervisorState,
};
