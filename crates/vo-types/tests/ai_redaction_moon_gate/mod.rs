//! Moon Gate: AI Redaction Integration Tests (ADR-008, ADR-025)
//!
//! Integration CI gate for AI redaction per ADR-008/025:
//! - PII injection tests verify zero leaks in operator projections
//! - Canonical encryption verification
//! - Access control: AI default path uses operator projection (not canonical)
//!
//! ADR-008: AI agents default to operator projection (redacted view).
//! ADR-025: Dual-representation privacy model with canonical (encrypted) and
//!          operator projection (redacted).

#![allow(clippy::unwrap_used)]

mod fixtures;
mod pii_injection;
mod canonical_encryption;
mod access_control;
mod purge_invariants;
mod redaction_edge_cases;
mod hash_determinism;
mod serialization;

pub use fixtures::*;
