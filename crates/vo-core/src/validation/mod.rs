//! Inline data size validation vs blob routing rules.
//!
//! Enforces the ADR-040 boundary between inline control-plane payloads
//! and externalized blobs. Any payload exceeding `INLINED_MAX_BYTES`
//! must be routed through the blob pipeline rather than admitted as
//! inline data.

pub mod payload;
pub mod workflow;

pub use payload::{validate_inline_size, PayloadTooLarge};
pub use workflow::{
    validate_effect_kinds, validate_workflow_effects, validate_workflow_sinks,
    UnsupportedSinkError, WorkflowSinkValidator,
};
