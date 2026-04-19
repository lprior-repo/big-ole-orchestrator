//! API type definitions for request/response handling.
//!
//! # Modules
//!
//! - [`v3`] - Current API version 3 types
//! - [`v1`] - Legacy API version 1 types (for backwards compatibility)
//! - [`errors`] - API error types
//! - [`names`] - Common name types (InstanceId, WorkflowName, etc.)
//! - [`helpers`] - Helper functions for type conversions

pub mod errors;
pub mod events;
pub mod helpers;
pub mod names;
pub mod v1;
pub mod v3;

#[cfg(test)]
mod security_validation_tests;
#[cfg(test)]
mod v3_test;

pub use errors::*;
pub use ingress::*;
pub use names::*;
pub use v1::*;
pub use v3::*;
