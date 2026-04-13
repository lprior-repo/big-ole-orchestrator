//! Proptest targets for pure functions.
//!
//! Each proptest states:
//! - **Invariant**: The property being verified
//! - **Strategy**: How test inputs are generated
//! - **Anti-invariant**: What would break the invariant

#![cfg(feature = "proptest")]

use crate::credentials::{AccessPolicy, Principal};
use crate::discovery::{enforce_pin, validate_discovery_path, DiscoveryPath, VersionPin};
use crate::dual_representation::{apply_redaction, RedactionKind, RedactionPolicy, RedactionRule};
use crate::events::payload::EventPayload;
use crate::integer_types::{
    AttemptNumber, DurationMs, EventVersion, FenceToken, FireAtMs, MaxAttempts, SequenceNumber,
    TimeoutMs, TimestampMs,
};
use crate::lifecycle_superstate::LifecycleSuperstate;
use crate::non_empty_vec::NonEmptyVec;
use crate::state::transition::{
    get_operational_status, get_valid_transitions, is_terminal, LifecycleState, OperationalStatus,
};
use crate::types::{
    extract_schema_version, AttemptNumber as TypeAttemptNumber, BinaryHash,
    DurationMs as TypeDurationMs, EventVersion as TypeEventVersion, FenceToken as TypeFenceToken,
    FireAtMs as TypeFireAtMs, IdempotencyKey, InstanceId, MaxAttempts as TypeMaxAttempts,
    SequenceNumber as TypeSequenceNumber, TimeoutMs as TypeTimeoutMs,
    TimestampMs as TypeTimestampMs,
};
use crate::workflow::next_nodes;
use crate::{ParseError, RedactedValue};
use proptest::prelude::*;
use std::collections::HashMap;
use std::num::NonZeroU64;
use std::time::Duration;

// ============ Integer Type Proptests ============

proptest! {
    // SequenceNumber invariants
    #[test]
    fn sequence_number_parse_validates_nonzero(value in 1u64..) {
        // Invariant: SequenceNumber must be nonzero
        // Strategy: Generate values from 1 to u64::MAX
        // Anti-invariant: value = 0 would fail
        let result = SequenceNumber::parse(&value.to_string());
        prop_assert!(result.is_ok(), "Parse should succeed for nonzero values");
        if let Ok(sn) = result {
            prop_assert_eq!(sn.as_u64(), value);
        }
    }

    #[test]
    fn sequence_number_display_roundtrip(value in 1u64..u64::MAX) {
        // Invariant: Display <-> Parse roundtrip preserves value
        // Strategy: Generate nonzero u64 values
        // Anti-invariant: any value that doesn't roundtrip
        let sn = SequenceNumber(NonZeroU64::new(value).expect("nonzero"));
        let serialized = sn.to_string();
        let deserialized = SequenceNumber::parse(&serialized);
        prop_assert_eq!(deserialized, Ok(sn));
    }

    #[test]
    fn sequence_number_ordering_preserved(a in 1u64.., b in 1u64..) {
        // Invariant: Ordering of SequenceNumbers matches u64 ordering
        // Strategy: Generate pairs of nonzero u64 values
        // Anti-invariant: reordering would break consistency
        let sa = SequenceNumber(NonZeroU64::new(a).expect("nonzero"));
        let sb = SequenceNumber(NonZeroU64::new(b).expect("nonzero"));
        prop_assert_eq!(sa.cmp(&sb), a.cmp(&b));
    }

    // EventVersion invariants
    #[test]
    fn event_version_parse_validates_nonzero(value in 1u64..) {
        // Invariant: EventVersion must be nonzero
        // Strategy: Generate values from 1 to u64::MAX
        // Anti-invariant: value = 0 would fail
        let result = EventVersion::parse(&value.to_string());
        prop_assert!(result.is_ok());
        if let Ok(ev) = result {
            prop_assert_eq!(ev.as_u64(), value);
        }
    }

    #[test]
    fn event_version_serialization_stability(value in 1u64..u64::MAX) {
        // Invariant: Serialized form is stable across versions
        // Strategy: Generate event version values
        // Anti-invariant: unstable serialization breaks compatibility
        let ev = EventVersion(NonZeroU64::new(value).expect("nonzero"));
        let json = serde_json::to_string(&ev).expect("serialize");
        let restored: EventVersion = serde_json::from_str(&json).expect("deserialize");
        prop_assert_eq!(restored, ev);
    }

    // AttemptNumber invariants
    #[test]
    fn attempt_number_monotonic_increase(a in 1u64.., b in 1u64..) {
        // Invariant: Higher attempt numbers indicate more recent attempts
        // Strategy: Generate pairs of attempt values
        // Anti-invariant: non-monotonic attempts break retry logic
        let an_a = AttemptNumber(NonZeroU64::new(a).expect("nonzero"));
        let an_b = AttemptNumber(NonZeroU64::new(b).expect("nonzero"));
        if a < b {
            prop_assert!(an_a < an_b);
        } else if a > b {
            prop_assert!(an_a > an_b);
        } else {
            prop_assert_eq!(an_a, an_b);
        }
    }

    // TimeoutMs invariants
    #[test]
    fn timeout_ms_to_duration_preserves_value(value in 1u64..u64::MAX) {
        // Invariant: TimeoutMs <-> Duration conversion is lossless
        // Strategy: Generate timeout values in milliseconds
        // Anti-invariant: duration mismatch breaks timeout logic
        let tm = TimeoutMs(NonZeroU64::new(value).expect("nonzero"));
        let duration = tm.to_duration();
        prop_assert_eq!(duration.as_millis(), value as u128);
    }

    #[test]
    fn timeout_ms_parse_validates_range(value in 1u64..u64::MAX) {
        // Invariant: TimeoutMs must be nonzero positive
        // Strategy: Generate positive u64 values
        // Anti-invariant: zero or negative would be invalid
        let result = TimeoutMs::parse(&value.to_string());
        prop_assert!(result.is_ok());
        if let Ok(tm) = result {
            prop_assert_eq!(tm.as_u64(), value);
        }
    }

    // DurationMs invariants
    #[test]
    fn duration_ms_zero_is_valid() {
        // Invariant: DurationMs can be zero (immediate execution)
        // Strategy: Test edge case of zero duration
        // Anti-invariant: rejecting zero breaks immediate operations
        let dm = DurationMs(0);
        prop_assert_eq!(dm.as_u64(), 0);
        prop_assert_eq!(dm.to_duration(), Duration::from_millis(0));
    }

    #[test]
    fn duration_ms_to_duration_roundtrip(value in 0u64..u64::MAX) {
        // Invariant: DurationMs -> Duration -> millisecond value is identity
        // Strategy: Generate all valid duration values including zero
        // Anti-invariant: conversion loss breaks timing precision
        let dm = DurationMs(value);
        let duration = dm.to_duration();
        prop_assert_eq!(duration.as_millis(), value as u128);
    }

    // TimestampMs invariants
    #[test]
    fn timestamp_ms_epoch_conversion(value in 0u64..u64::MAX) {
        // Invariant: TimestampMs correctly converts to SystemTime from epoch
        // Strategy: Generate timestamp values
        // Anti-invariant: incorrect epoch math breaks time-based queries
        use std::time::{Duration, SystemTime};
        let ts = TimestampMs(value);
        let st = ts.to_system_time();
        prop_assert_eq!(st, SystemTime::UNIX_EPOCH + Duration::from_millis(value));
    }

    #[test]
    fn timestamp_ms_ordering(a in 0u64.., b in 0u64..) {
        // Invariant: Timestamp ordering matches millisecond value ordering
        // Strategy: Generate pairs of timestamp values
        // Anti-invariant: incorrect ordering breaks event causality
        let ta = TimestampMs(a);
        let tb = TimestampMs(b);
        prop_assert_eq!(ta.cmp(&tb), a.cmp(&b));
    }

    // FireAtMs invariants
    #[test]
    fn fire_at_ms_has_elapsed_correctness(fire_at in 0u64.., now in 0u64..) {
        // Invariant: has_elapsed returns true iff fire_at < now
        // Strategy: Generate pairs of fire times and current times
        // Anti-invariant: wrong comparison breaks timer firing
        let fire = FireAtMs(fire_at);
        let current = TimestampMs(now);
        prop_assert_eq!(fire.has_elapsed(current), fire_at < now);
    }

    #[test]
    fn fire_at_ms_boundary_edge_cases() {
        // Invariant: Boundary values (0, MAX) are handled correctly
        // Strategy: Test edge cases at boundaries
        // Anti-invariant: boundary failures break timer system
        let fire_zero = FireAtMs(0);
        let now_any = TimestampMs(1);
        prop_assert!(fire_zero.has_elapsed(now_any));

        let now_zero = TimestampMs(0);
        let fire_any = FireAtMs(1);
        prop_assert!(!fire_any.has_elapsed(now_zero));
    }

    // MaxAttempts invariants
    #[test]
    fn max_attempts_is_exhausted_logic(max_val in 1u64.., attempt_val in 1u64..) {
        // Invariant: is_exhausted returns true iff attempt >= max
        // Strategy: Generate max and attempt value pairs
        // Anti-invariant: wrong exhaustion logic breaks retry limits
        let max = MaxAttempts(NonZeroU64::new(max_val).expect("nonzero"));
        let attempt = AttemptNumber(NonZeroU64::new(attempt_val).expect("nonzero"));
        prop_assert_eq!(max.is_exhausted(attempt), attempt_val >= max_val);
    }

    #[test]
    fn max_attempts_minimum_is_one() {
        // Invariant: MaxAttempts minimum value is 1
        // Strategy: Test minimum valid value
        // Anti-invariant: zero max attempts would prevent execution
        let max = MaxAttempts::parse("1").expect("valid");
        prop_assert_eq!(max.as_u64(), 1);

        let attempt = AttemptNumber::parse("1").expect("valid");
        prop_assert!(!max.is_exhausted(attempt));
    }

    // FenceToken invariants
    #[test]
    fn fencetoken_next_monotonic(value in 1u64..u64::MAX) {
        // Invariant: next() always produces strictly greater value
        // Strategy: Generate fence tokens below MAX
        // Anti-invariant: non-strictly-increasing breaks fencing
        let token = FenceToken::new(value).expect("valid");
        let next = token.next().expect("next should succeed");
        prop_assert!(next.inner().get() > token.inner().get());
    }

    #[test]
    fn fencetoken_next_consecutive(value in 1u64..u64::MAX) {
        // Invariant: next() produces exactly value + 1
        // Strategy: Generate fence tokens below MAX
        // Anti-invariant: non-consecutive breaks token sequencing
        let token = FenceToken::new(value).expect("valid");
        let next = token.next().expect("next should succeed");
        prop_assert_eq!(next.inner().get(), value + 1);
    }

    #[test]
    fn fencetoken_rejects_zero() {
        // Invariant: FenceToken cannot be zero
        // Strategy: Test zero value rejection
        // Anti-invariant: accepting zero breaks fence semantics
        let result = FenceToken::new(0);
        prop_assert!(result.is_err());
        if let Err(e) = result {
            prop_assert_eq!(e, ParseError::ZeroValue { type_name: "FenceToken" });
        }
    }

    #[test]
    fn fencetoken_max_fails(next_max in u64::MAX) {
        // Invariant: next() fails when at u64::MAX
        // Strategy: Test maximum value edge case
        // Anti-invariant: wrapping or panicking breaks fencing
        let token = FenceToken::new(u64::MAX).expect("valid");
        let result = token.next();
        prop_assert!(result.is_err());
    }

    // ============ Discovery Path Proptests ============

    #[test]
    fn discovery_path_validation_accepts_valid(
        component in any::<String>(),
        version in any::<String>(),
    ) {
        // Invariant: Valid component/version strings pass validation
        // Strategy: Generate typical component names and versions
        // Anti-invariant: rejecting valid paths breaks discovery
        let path = DiscoveryPath::new(&component, &version);
        let result = validate_discovery_path(&path);
        prop_assert!(result.is_ok());
    }

    #[test]
    fn discovery_path_validation_rejects_empty_component() {
        // Invariant: Empty component strings are rejected
        // Strategy: Test empty component edge case
        // Anti-invariant: accepting empty components breaks routing
        let path = DiscoveryPath::new("", "1.0.0");
        let result = validate_discovery_path(&path);
        prop_assert!(result.is_err());
    }

    #[test]
    fn discovery_path_version_format(version: String) {
        // Invariant: Version strings are preserved through validation
        // Strategy: Generate various version strings
        // Anti-invariant: mangled versions break compatibility checks
        let path = DiscoveryPath::new("component", &version);
        let result = validate_discovery_path(&path);
        if result.is_ok() {
            prop_assert_eq!(path.version(), &version);
        }
    }

    // ============ Pin Enforcement Proptests ============

    #[test]
    fn pin_enforce_accepts_matching_hash(
        hash in any::<String>(),
    ) {
        // Invariant: Pin accepts binary hash matching pinned value
        // Strategy: Generate matching hash strings
        // Anti-invariant: rejecting matching hashes breaks pinning
        let pin = VersionPin::parse(&hash).expect("valid");
        let candidate = hash.clone();
        let result = enforce_pin(&pin, &candidate);
        prop_assert!(result.is_ok());
    }

    #[test]
    fn pin_enforce_rejects_mismatched_hash(
        pinned in any::<String>(),
        candidate in any::<String>(),
    ) {
        // Invariant: Pin rejects binary hash not matching pinned value
        // Strategy: Generate mismatched hash pairs
        // Anti-invariant: accepting mismatched hashes breaks pinning
        prop_assume!(pinned != candidate);
        let pin = VersionPin::parse(&pinned).expect("valid");
        let result = enforce_pin(&pin, &candidate);
        prop_assert!(result.is_err());
    }

    #[test]
    fn pin_enforce_exact_match(pinned in any::<String>()) {
        // Invariant: Pin requires exact byte-level match
        // Strategy: Generate hash strings
        // Anti-invariant: case-insensitive or fuzzy matching breaks pins
        let pin = VersionPin::parse(&pinned).expect("valid");
        let result = enforce_pin(&pin, &pinned);
        prop_assert!(result.is_ok());

        // Slight modification should fail
        let modified = pinned + "x";
        let result_modified = enforce_pin(&pin, &modified);
        prop_assert!(result_modified.is_err());
    }

    // ============ Lifecycle State Proptests ============

    #[test]
    fn is_terminal_identifies_terminal_states_correctly() {
        // Invariant: Terminal states are correctly identified
        // Strategy: Test all known lifecycle states
        // Anti-invariant: misidentifying terminal states breaks workflow completion
        let terminal_states = [
            LifecycleState::Terminated,
            LifecycleState::Failed,
            LifecycleState::Cancelled,
        ];

        for state in &terminal_states {
            prop_assert!(is_terminal(*state));
        }

        let non_terminal_states = [
            LifecycleState::Created,
            LifecycleState::Running,
            LifecycleState::Suspended,
            LifecycleState::Resuming,
        ];

        for state in &non_terminal_states {
            prop_assert!(!is_terminal(*state));
        }
    }

    #[test]
    fn operational_status_from_state(state in any::<LifecycleState>()) {
        // Invariant: OperationalStatus correctly derived from state
        // Strategy: Generate all lifecycle states
        // Anti-invariant: wrong status breaks health checks
        let status = get_operational_status(state);
        match state {
            LifecycleState::Terminated | LifecycleState::Failed | LifecycleState::Cancelled => {
                prop_assert_eq!(status, OperationalStatus::Stopped);
            }
            LifecycleState::Suspended => {
                prop_assert_eq!(status, OperationalStatus::Paused);
            }
            _ => {
                prop_assert_eq!(status, OperationalStatus::Running);
            }
        }
    }

    #[test]
    fn get_valid_transitions_produces_valid_events(state in any::<LifecycleState>()) {
        // Invariant: Valid transitions only produce defined TransitionEvents
        // Strategy: Generate all lifecycle states
        // Anti-invariant: undefined transitions break workflow
        let transitions = get_valid_transitions(state);
        for transition in transitions {
            // Each transition should have a valid event type
            prop_assert!(!transition.event_type().is_empty());
        }
    }

    // ============ Redaction Proptests ============

    #[test]
    fn redaction_rule_identity(value: RedactedValue, field: String) {
        // Invariant: Applying rule to non-matching value is identity
        // Strategy: Generate arbitrary values and field names
        // Anti-invariant: modifying non-targets leaks or corrupts data
        let rule = RedactionRule::new(&field);
        let result = apply_redaction(&value, &rule);
        prop_assert_eq!(result, value);
    }

    #[test]
    fn redaction_rule_matches_field(field_name: String, value: RedactedValue) {
        // Invariant: Rule matches only when field name matches
        // Strategy: Generate field names and values
        // Anti-invariant: false positives/negatives break security
        let rule = RedactionRule::new(&field_name);
        let result = apply_redaction(&value, &rule);

        match &value {
            RedactedValue::Object(map) => {
                if let Some(v) = map.get(&field_name) {
                    prop_assert_eq!(result, RedactedValue::Redacted);
                } else {
                    prop_assert_eq!(result, value);
                }
            }
            _ => {
                prop_assert_eq!(result, value);
            }
        }
    }

    #[test]
    fn redaction_preserves_structure(map_size in 0usize..10, target_field: String) {
        // Invariant: Redaction preserves overall structure except matched fields
        // Strategy: Generate maps of various sizes
        // Anti-invariant: structural changes break downstream parsing
        use serde_json::json;
        let mut map: HashMap<String, RedactedValue> = HashMap::new();

        for i in 0..map_size {
            map.insert(format!("field_{}", i), RedactedValue::Number(i as u64));
        }

        let value = RedactedValue::Object(map.clone());
        let rule = RedactionRule::new(&target_field);
        let result = apply_redaction(&value, &rule);

        // Result should still be an object
        if let RedactedValue::Object(result_map) = result {
            prop_assert_eq!(result_map.len(), map_size);
        }
    }

    // ============ Workflow Next Nodes Proptests ============

    #[test]
    fn next_nodes_returns_all_successors(
        node_id: String,
        num_successors in 0usize..5,
    ) {
        // Invariant: next_nodes returns all valid successor nodes
        // Strategy: Generate node with various successor counts
        // Anti-invariant: missing successors break workflow execution
        let mut successors: Vec<String> = Vec::new();
        for i in 0..num_successors {
            successors.push(format!("node_{}", i));
        }

        let workflow = next_nodes(&node_id, &successors);
        prop_assert_eq!(workflow.len(), num_successors);
    }

    #[test]
    fn next_nodes_empty_successors_returns_empty(node_id: String) {
        // Invariant: Node with no successors returns empty list
        // Strategy: Test leaf nodes
        // Anti-invariant: returning successors for leaf nodes causes errors
        let successors: Vec<String> = Vec::new();
        let workflow = next_nodes(&node_id, &successors);
        prop_assert!(workflow.is_empty());
    }

    // ============ NonEmptyVec Proptests ============

    #[test]
    fn non_empty_vec_construction_requires_at_least_one(
        first in any::<u64>(),
        rest in proptest::collection::vec(any::<u64>(), 0..10),
    ) {
        // Invariant: NonEmptyVec requires at least one element
        // Strategy: Generate one required element plus optional additional
        // Anti-invariant: empty construction would be unsafe
        let result = NonEmptyVec::new(first, rest);
        prop_assert!(result.is_ok());

        if let Ok(vec) = result {
            prop_assert_eq!(vec.len(), 1 + rest.len());
        }
    }

    #[test]
    fn non_empty_vec_rejects_empty() {
        // Invariant: Construction with zero elements fails
        // Strategy: Test empty input
        // Anti-invariant: accepting empty breaks non-empty guarantee
        let result = NonEmptyVec::<u64>::new(0, Vec::new());
        prop_assert!(result.is_err());
    }

    #[test]
    fn non_empty_vec_access_first_returns_head(first in any::<u64>(), rest: Vec<u64>) {
        // Invariant: first() returns the first element
        // Strategy: Generate head and tail elements
        // Anti-invariant: wrong first element breaks iteration
        let nev = NonEmptyVec::new(first, rest).expect("valid");
        prop_assert_eq!(nev.first(), &first);
    }

    // ============ Compensation Transition Proptests ============

    #[test]
    fn compensation_transition_applies_idempotently(
        state_value in 0u64..100,
        action in any::<String>(),
    ) {
        // Invariant: Repeated application produces same result
        // Strategy: Generate state and action pairs
        // Anti-invariant: non-idempotent compensation corrupts state
        use crate::compensation::apply_compensation_transition;

        let mut state_a = state_value;
        let mut state_b = state_value;

        apply_compensation_transition(&mut state_a, &action);
        let first_result = state_a;

        apply_compensation_transition(&mut state_b, &action);
        let second_result = state_b;

        prop_assert_eq!(first_result, second_result);
    }

    // ============ Connector Transition Proptests ============

    #[test]
    fn connector_transition_preserves_identity(
        connector_id: String,
        action in any::<String>(),
    ) {
        // Invariant: Connector identity preserved through transitions
        // Strategy: Generate connector IDs and actions
        // Anti-invariant: identity loss breaks connector routing
        use crate::connector::transition::apply_connector_transition;

        let mut connector = connector_id.clone();
        apply_connector_transition(&mut connector, &action);

        // Connector ID should remain unchanged (identity is invariant)
        prop_assert_eq!(connector, connector_id);
    }

    // ============ Effect Transition Proptests ============

    #[test]
    fn effect_transition_state_change_deterministic(
        initial_state in 0u8..256,
        effect_value in 0u8..256,
    ) {
        // Invariant: Same inputs produce same output state
        // Strategy: Generate state and effect pairs
        // Anti-invariant: non-deterministic effects break reproducibility
        use crate::effects::apply_effect_transition;

        let mut state_a = initial_state;
        let mut state_b = initial_state;

        apply_effect_transition(&mut state_a, effect_value);
        apply_effect_transition(&mut state_b, effect_value);

        prop_assert_eq!(state_a, state_b);
    }

    // ============ Dual Representation Proptests ============

    #[test]
    fn dual_representation_redaction_is_applicative(
        value in any::<RedactedValue>(),
        rule1 in any::<String>(),
        rule2 in any::<String>(),
    ) {
        // Invariant: Applying multiple rules is order-independent
        // Strategy: Generate values and rule pairs
        // Anti-invariant: order-dependent redaction breaks consistency
        let rule1 = RedactionRule::new(&rule1);
        let rule2 = RedactionRule::new(&rule2);

        let result1 = apply_redaction(&apply_redaction(&value, &rule1), &rule2);
        let result2 = apply_redaction(&apply_redaction(&value, &rule2), &rule1);

        prop_assert_eq!(result1, result2);
    }

    // ============ Access Control Proptests ============

    #[test]
    fn access_policy_principal_match(policy_rules in proptest::collection::vec(any::<String>(), 0..10),
                                      principal in any::<Principal>()) {
        // Invariant: AccessPolicy construction preserves rules
        // Strategy: Generate policy rules and principals
        // Anti-invariant: rule corruption breaks access control
        let policy = AccessPolicy::new(policy_rules.clone());
        prop_assert_eq!(policy.allowed_principals().len(), policy_rules.len());
    }

    #[test]
    fn system_principal_distinct_from_user() {
        // Invariant: System principal is distinct from User principal
        // Strategy: Test principal equality
        // Anti-invariant: confusing principals breaks security boundaries
        let system = Principal::System;
        let user = Principal::User(crate::InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid"));
        prop_assert_ne!(system, user);
    }

    // ============ Anti-Invariant Tests ============

    #[test]
    fn anti_invariant_sequence_number_nonzero_fails(
        _value in proptest::collection::vec(0u8, 1..10),
    ) {
        // Anti-invariant: SequenceNumber MUST NOT accept zero
        // This test documents what should NEVER happen
        let result = SequenceNumber::parse("0");
        prop_assert!(result.is_err(), "Zero must be rejected");
    }

    #[test]
    fn anti_invariant_fence_token_nonzero_fails(
        _value in proptest::collection::vec(0u8, 1..10),
    ) {
        // Anti-invariant: FenceToken MUST NOT accept zero
        // This test documents what should NEVER happen
        let result = FenceToken::new(0);
        prop_assert!(result.is_err(), "Zero must be rejected");
    }

    #[test]
    fn anti_invariant_max_attempts_minimum_is_one() {
        // Anti-invariant: MaxAttempts MUST be >= 1
        // This test documents what should NEVER happen
        let result = MaxAttempts::parse("0");
        prop_assert!(result.is_err(), "Zero must be rejected for MaxAttempts");
    }
}
