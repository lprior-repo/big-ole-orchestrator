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

pub mod check;
pub mod control;
pub mod types;

pub use check::{check_admission, check_admission_with_thresholds};
pub use control::{
    AdmissionCheck, AdmissionResult, DedupeToken, DedupeTokenError, RejectionReason,
};
pub use types::{AdmissionError, AdmissionThresholds, PressureIndicator, WritePressureState};
