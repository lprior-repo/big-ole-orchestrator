//! Workflow lineage and epoch types for continue-as-new (ADR-038).
//!
//! These types track workflow identity across epoch rollover boundaries:
//! - [`Epoch`] identifies one execution epoch within a lineage
//! - [`WorkflowLineage`] binds a stable lineage_id to an epoch with optional parent
//! - [`LineageError`] enumerates construction failures
//!
//! Split into:
//! - [`parent`] — `Epoch`, `WorkflowLineage`, `LineageError`
//! - [`trace`] — `LineageStatus`, `LineageState`

mod parent;
mod trace;

pub use parent::{Epoch, LineageError, WorkflowLineage};
pub use trace::{LineageState, LineageStatus};
