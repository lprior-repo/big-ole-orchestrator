//! Inline data size validation vs blob routing rules.
//!
//! Enforces the ADR-040 boundary between inline control-plane payloads
//! and externalized blobs. Any payload exceeding `INLINED_MAX_BYTES`
//! must be routed through the blob pipeline rather than admitted as
//! inline data.

pub mod payload;

pub use payload::{validate_inline_size, PayloadTooLarge};
