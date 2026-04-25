//! Workflow lineage and epoch types for continue-as-new (ADR-038).
//!
//! These types track workflow identity across epoch rollover boundaries:
//! - [`Epoch`] identifies one execution epoch within a lineage
//! - [`WorkflowLineage`] binds a stable lineage_id to an epoch with optional parent
//! - [`LineageError`] enumerates construction failures
//!
//! Split into:
//! - [`epoch`] — `Epoch` newtype wrapper
//! - [`error`] — `LineageError` enum and validation
//! - [`workflow`] — `WorkflowLineage` struct
//! - [`trace`] — `LineageStatus`, `LineageState`

mod epoch;
mod error;
mod trace;
mod workflow;

pub use epoch::Epoch;
pub use error::LineageError;
pub use workflow::WorkflowLineage;
pub use trace::{LineageState, LineageStatus};
