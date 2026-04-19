//! Degraded admission coupled to write pressure.
//!
//! This module implements the admission coupling that binds degraded-mode
//! admission decisions to actual write pressure indicators.
//!
//! # Write Classes
//!
//! - **Critical Control Plane**: events, instances, dedupe, effects, leases, timers, snapshots
//! - **Operator Projections**: dashboard views, redacted history enrichments, UI convenience indexes
//! - **Bulk Blobs**: large canonical payloads, bounded stderr blobs, optional large outputs
//!
//! # Admission Rules
//!
//! Critical control-plane writes are protected when degraded mode is active.
//! Operator projections may lag under pressure (acceptable degradation).
//! Bulk blobs may be deferred under pressure but canonical blobs must not
//! violate control-plane durability boundaries.

pub mod budget;
pub mod budget_ops;
pub mod check;
pub mod control;
pub mod controller;
pub mod metrics; // intentionally pub to expose Gauge, BoolGauge, WritePressureMetrics
pub mod types;
pub mod workload;

#[cfg(test)]
pub mod check_tests;
#[cfg(kani)]
pub mod check_verification;
#[cfg(test)]
pub mod circuit_breaker_tests;
#[cfg(test)]
pub mod control_tests;
#[cfg(test)]
pub mod controller_tests;
#[cfg(test)]
pub mod workload_tests;

pub use budget::{BudgetAllocation, WorkloadBudget};
pub use budget_ops::{
    acquire_slot, check_budget, compute_degraded_mode, is_class_accepted_in_mode, release_slot,
    set_degraded_mode, BudgetCheckResult, BudgetRejectionReason,
};
pub use check::{check_admission, check_admission_with_thresholds};
pub use control::{AdmissionCheck, AdmissionResult, DedupeToken, RejectionReason};
pub use controller::{AdmissionController, AdmissionControllerWithDlq};
pub use metrics::{BoolGauge, Gauge, WritePressureMetrics};
pub use types::{AdmissionDeadLetterQueue, AdmissionError, AdmissionThresholds, PressureIndicator, RejectedRequest, WritePressureState};
