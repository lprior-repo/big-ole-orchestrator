#![allow(clippy::needless_for_each)]
#![allow(clippy::useless_conversion)]
#![allow(clippy::into_iter_on_ref)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

//! Red Queen adversarial tests for the instance index partition.
//!
//! These tests attempt to break the implementation through:
//! - Contract violation attempts
//! - Edge cases (boundary values, nil UUIDs, extreme timestamps)
//! - Key encoding attacks (prefix scan confusion, status byte boundaries)
//! - Invariant violations under stress (phantom entries, large volumes)
//! - Ordering verification under adversarial conditions
//!
//! bead_id: vel-ngt
//! bead_title: vo-storage: implement instance index partition
//! phase: 5

mod contract_violations;
mod edge_cases;
mod encode_decode;
mod helpers;
mod invariant_violations;
mod key_encoding_attacks;
mod proptests;
mod scan_all;