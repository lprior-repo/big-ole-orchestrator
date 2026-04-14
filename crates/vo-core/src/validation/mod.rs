//! Validation module for workflow publishing and related checks.

pub mod workflow;

pub use workflow::{UnsupportedSinkError, WorkflowSinkValidator, validate_workflow_sinks};