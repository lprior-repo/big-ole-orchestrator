//! wtf-engine: Binary Registry and Discovery
//!
//! This crate implements the `BinaryRegistry`, a live, concurrent registry that maps
//! `WorkflowName` to versioned binary paths. Supports a three-state lifecycle
//! (Active -> Deactivated -> Deleted) with a background Reaper GC loop.

mod error;
mod helpers;
mod registry;
mod types;

#[cfg(test)]
mod tests;

pub use error::BinaryRegistryError;
pub use registry::BinaryRegistry;
pub use types::{BinaryPath, ReaperReport, RegistrationStatus, WorkflowRegistration};
