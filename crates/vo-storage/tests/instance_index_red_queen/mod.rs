// Instance Index Red Queen adversarial tests — decomposed into submodules.
//!
//! These tests attempt to break the instance index through:
//! - Contract violation attempts (contract_violations)
//! - Edge cases (edge_cases)
//! - Key encoding attacks (key_encoding_attacks)
//! - Invariant violations under stress (invariant_violations, rapid_transitions)
//! - Encode/decode edge cases (encode_decode)
//! - Value slot verification & scan correctness (value_and_scan)
//! - Property-based tests (proptests)

mod helpers;

mod contract_violations;
mod encode_decode;
mod edge_cases;
mod invariant_violations;
mod key_encoding_attacks;
mod proptests;
mod rapid_transitions;
mod value_and_scan;
