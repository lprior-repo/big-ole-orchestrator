#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

//! Red Queen adversarial tests for canonical key encoding (ADR-020).
//!
//! These tests probe:
//! - Lexicographic ordering invariants across all key types
//! - Prefix collision between different key types
//! - Key composition soundness across all entity types
//! - Edge cases in encoding/decoding boundary conditions

pub mod lexicographic_ordering;
pub mod prefix_collision;

use vo_types::{InstanceId, SequenceNumber, StepId};

// ========================================================================
// HELPERS
// ========================================================================

fn min_instance_id() -> InstanceId {
    InstanceId::parse("00000000000000000000000001").unwrap()
}

fn max_instance_id() -> InstanceId {
    InstanceId::parse("7ZZZZZZZZZZZZZZZZZZZZZZZZZ").unwrap()
}

fn mid_instance_id() -> InstanceId {
    InstanceId::parse("40000000000000000000000000").unwrap()
}
