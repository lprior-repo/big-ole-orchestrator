//! Inline data size validation vs blob routing rules.
//!
//! Enforces the ADR-040 boundary between inline control-plane payloads
//! and externalized blobs. Any payload exceeding `INLINED_MAX_BYTES`
//! must be routed through the blob pipeline rather than admitted as
//! inline data.
//!
//! Workflow publish validation (ADR-003, ADR-031):
//! - Publish-time rejection of workflows that violate guarantee-class invariants
//!   (e.g., Unsafe nodes in non-BestEffort workflows)
//! - Publish-time rejection of exact-once workflows missing dedupe policy (ADR-028)

pub mod payload;
pub mod unsafe_node;
pub mod workflow;

pub use payload::{validate_inline_size, PayloadTooLarge};
pub use unsafe_node::{validate_no_unsafe_in_exact_workflow, UnsafeNodeInExactWorkflow};
pub use workflow::{
    validate_effect_kinds, validate_exact_workflow_node_kinds, validate_workflow_effects,
    validate_workflow_sinks, KnownSinks, NodeDescriptor, UnsafeNodeError,
    UnsupportedSinkError, WorkflowSinkValidator,
};

#[cfg(test)]
mod workflow_tests;
