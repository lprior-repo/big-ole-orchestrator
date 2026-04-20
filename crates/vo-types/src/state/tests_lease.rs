//! Tests for LeaseRecord construction, accessors, and fence-token matching.

use super::*;

fn make_instance_id(s: &str) -> crate::string_types::InstanceId {
    // Just return a dummy InstanceId for tests by directly wrapping String (as it's pub(crate))
    crate::string_types::InstanceId(s.to_string())
}

fn make_step_id(s: &str) -> crate::string_types::StepId {
    crate::string_types::StepId(s.to_string())
}

fn make_fence_token(v: u64) -> crate::integer_types::FenceToken {
    crate::integer_types::FenceToken(std::num::NonZeroU64::new(v).unwrap())
}

#[test]
fn leaserecord_returns_success_when_instantiated_with_typical_components() {
    let rec = LeaseRecord::new(
        make_instance_id("inst-1"),
        make_step_id("step-1"),
        make_fence_token(42),
    );
    assert_eq!(rec.instance_id(), &make_instance_id("inst-1"));
    assert_eq!(rec.step_id(), &make_step_id("step-1"));
    assert_eq!(rec.token(), &make_fence_token(42));
}

#[test]
fn leaserecord_returns_success_when_instantiated_with_minimum_boundary_components() {
    let rec = LeaseRecord::new(
        make_instance_id("01H8X"),
        make_step_id("A"),
        make_fence_token(1),
    );
    assert_eq!(rec.instance_id(), &make_instance_id("01H8X"));
    assert_eq!(rec.step_id(), &make_step_id("A"));
    assert_eq!(rec.token(), &make_fence_token(1));
}

#[test]
fn leaserecord_returns_success_when_instantiated_with_maximum_boundary_components() {
    let rec = LeaseRecord::new(
        make_instance_id("long"),
        make_step_id("long"),
        make_fence_token(u64::MAX),
    );
    assert_eq!(rec.token(), &make_fence_token(u64::MAX));
}

#[test]
fn leaserecord_returns_exact_instance_id_when_instance_id_called_on_typical_record() {
    let rec = LeaseRecord::new(
        make_instance_id("inst-1"),
        make_step_id("step"),
        make_fence_token(1),
    );
    assert_eq!(rec.instance_id(), &make_instance_id("inst-1"));
}

#[test]
fn leaserecord_returns_exact_instance_id_when_instance_id_called_on_minimum_boundary_record() {
    let rec = LeaseRecord::new(
        make_instance_id("a"),
        make_step_id("step"),
        make_fence_token(1),
    );
    assert_eq!(rec.instance_id(), &make_instance_id("a"));
}

#[test]
fn leaserecord_returns_exact_instance_id_when_instance_id_called_on_maximum_boundary_record() {
    let rec = LeaseRecord::new(
        make_instance_id("max"),
        make_step_id("step"),
        make_fence_token(1),
    );
    assert_eq!(rec.instance_id(), &make_instance_id("max"));
}

#[test]
fn leaserecord_returns_exact_step_id_when_step_id_called_on_typical_record() {
    let rec = LeaseRecord::new(
        make_instance_id("inst"),
        make_step_id("step-1"),
        make_fence_token(1),
    );
    assert_eq!(rec.step_id(), &make_step_id("step-1"));
}

#[test]
fn leaserecord_returns_exact_step_id_when_step_id_called_on_single_char_boundary_record() {
    let rec = LeaseRecord::new(
        make_instance_id("inst"),
        make_step_id("A"),
        make_fence_token(1),
    );
    assert_eq!(rec.step_id(), &make_step_id("A"));
}

#[test]
fn leaserecord_returns_exact_step_id_when_step_id_called_on_numeric_boundary_record() {
    let rec = LeaseRecord::new(
        make_instance_id("inst"),
        make_step_id("1"),
        make_fence_token(1),
    );
    assert_eq!(rec.step_id(), &make_step_id("1"));
}

#[test]
fn leaserecord_returns_exact_token_when_token_called_on_typical_record() {
    let rec = LeaseRecord::new(
        make_instance_id("inst"),
        make_step_id("step"),
        make_fence_token(42),
    );
    assert_eq!(rec.token(), &make_fence_token(42));
}

#[test]
fn leaserecord_returns_exact_token_when_token_called_on_minimum_limit_record() {
    let rec = LeaseRecord::new(
        make_instance_id("inst"),
        make_step_id("step"),
        make_fence_token(1),
    );
    assert_eq!(rec.token(), &make_fence_token(1));
}

#[test]
fn leaserecord_returns_exact_token_when_token_called_on_maximum_limit_record() {
    let rec = LeaseRecord::new(
        make_instance_id("inst"),
        make_step_id("step"),
        make_fence_token(u64::MAX),
    );
    assert_eq!(rec.token(), &make_fence_token(u64::MAX));
}

#[test]
fn leaserecord_returns_true_when_matches_token_called_with_exact_typical_match() {
    let rec = LeaseRecord::new(
        make_instance_id("inst"),
        make_step_id("step"),
        make_fence_token(42),
    );
    assert!(rec.matches_token(&make_fence_token(42)));
}

#[test]
fn leaserecord_returns_true_when_matches_token_called_with_exact_minimum_match() {
    let rec = LeaseRecord::new(
        make_instance_id("inst"),
        make_step_id("step"),
        make_fence_token(1),
    );
    assert!(rec.matches_token(&make_fence_token(1)));
}

#[test]
fn leaserecord_returns_true_when_matches_token_called_with_exact_maximum_match() {
    let rec = LeaseRecord::new(
        make_instance_id("inst"),
        make_step_id("step"),
        make_fence_token(u64::MAX),
    );
    assert!(rec.matches_token(&make_fence_token(u64::MAX)));
}

#[test]
fn leaserecord_returns_false_when_matches_token_called_with_stale_token_by_one() {
    let rec = LeaseRecord::new(
        make_instance_id("inst"),
        make_step_id("step"),
        make_fence_token(5),
    );
    assert!(!rec.matches_token(&make_fence_token(4)));
}

#[test]
fn leaserecord_returns_false_when_matches_token_called_with_stale_token_by_large_margin() {
    let rec = LeaseRecord::new(
        make_instance_id("inst"),
        make_step_id("step"),
        make_fence_token(100),
    );
    assert!(!rec.matches_token(&make_fence_token(10)));
}

#[test]
fn leaserecord_returns_false_when_matches_token_called_with_future_token_by_one() {
    let rec = LeaseRecord::new(
        make_instance_id("inst"),
        make_step_id("step"),
        make_fence_token(5),
    );
    assert!(!rec.matches_token(&make_fence_token(6)));
}

#[test]
fn leaserecord_returns_false_when_matches_token_called_with_future_token_by_large_margin() {
    let rec = LeaseRecord::new(
        make_instance_id("inst"),
        make_step_id("step"),
        make_fence_token(5),
    );
    assert!(!rec.matches_token(&make_fence_token(100)));
}

#[test]
fn leaserecord_returns_false_when_matches_token_called_with_maximum_token_against_minimum_record() {
    let rec = LeaseRecord::new(
        make_instance_id("inst"),
        make_step_id("step"),
        make_fence_token(1),
    );
    assert!(!rec.matches_token(&make_fence_token(u64::MAX)));
}

#[test]
fn leaserecord_returns_false_when_matches_token_called_with_minimum_token_against_maximum_record() {
    let rec = LeaseRecord::new(
        make_instance_id("inst"),
        make_step_id("step"),
        make_fence_token(u64::MAX),
    );
    assert!(!rec.matches_token(&make_fence_token(1)));
}
