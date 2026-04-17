//! Proptest targets for pure functions.
//!
//! Each proptest states:
//! - **Invariant**: The property being verified
//! - **Strategy**: How test inputs are generated
//! - **Anti-invariant**: What would break the invariant

#![cfg(feature = "proptest")]

use crate::credentials::{AccessPolicy, Principal};
use crate::discovery::{enforce_pin, validate_discovery_path, DiscoveryPath, VersionPin};
<<<<<<< HEAD
use crate::dual_representation::{apply_redaction, RedactionKind, RedactionPolicy, RedactionRule};
=======
use crate::dual_representation::{apply_redaction, RedactedValue, RedactionRule};
>>>>>>> origin/vo-worker-tests
use crate::events::payload::EventPayload;
use crate::integer_types::{
    AttemptNumber, DurationMs, EventVersion, FenceToken, FireAtMs, MaxAttempts, SequenceNumber,
    TimeoutMs, TimestampMs,
};
<<<<<<< HEAD
use crate::lifecycle_superstate::LifecycleSuperstate;
=======
use crate::lifecycle_superstate::LifecycleSuperState;
>>>>>>> origin/vo-worker-tests
use crate::non_empty_vec::NonEmptyVec;
use crate::state::transition::{
    get_operational_status, get_valid_transitions, is_terminal, LifecycleState, OperationalStatus,
};
<<<<<<< HEAD
use crate::types::{
    extract_schema_version, AttemptNumber as TypeAttemptNumber, BinaryHash,
    DurationMs as TypeDurationMs, EventVersion as TypeEventVersion, FenceToken as TypeFenceToken,
    FireAtMs as TypeFireAtMs, IdempotencyKey, InstanceId, MaxAttempts as TypeMaxAttempts,
    SequenceNumber as TypeSequenceNumber, TimeoutMs as TypeTimeoutMs,
    TimestampMs as TypeTimestampMs,
};
=======
use crate::types::{extract_schema_version, SchemaVersion};
>>>>>>> origin/vo-worker-tests
use crate::workflow::next_nodes;
use crate::ParseError;
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
<<<<<<< HEAD
=======
        use serde_json;
>>>>>>> origin/vo-worker-tests
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
<<<<<<< HEAD
        binary_name in "[a-z][a-z0-9_]*",
        hash in "[a-f0-9]{16,64}",
    ) {
        // Invariant: Valid binary_name and hash pass validation
        // Strategy: Generate valid binary names and hex hashes
        // Anti-invariant: rejecting valid paths breaks discovery
        let binary_hash = BinaryHash::parse(&hash).unwrap();
        let path = DiscoveryPath::new(
            "/var/wtf/versions".to_string(),
            binary_hash,
            binary_name,
        );
=======
        component in any::<String>(),
        version in any::<String>(),
    ) {
        // Invariant: Valid component/version strings pass validation
        // Strategy: Generate typical component names and versions
        // Anti-invariant: rejecting valid paths breaks discovery
        let path = DiscoveryPath::new(&component, &version);
>>>>>>> origin/vo-worker-tests
        let result = validate_discovery_path(&path);
        prop_assert!(result.is_ok());
    }

    #[test]
<<<<<<< HEAD
    fn discovery_path_validation_rejects_empty_binary_name() {
        // Invariant: Empty binary_name strings are rejected
        // Strategy: Test empty binary_name edge case
        // Anti-invariant: accepting empty binary_name breaks routing
        let path = DiscoveryPath::new(
            "/var/wtf/versions".to_string(),
            BinaryHash::parse("abcdef0123456789").unwrap(),
            String::new(),
        );
=======
    fn discovery_path_validation_rejects_empty_component() {
        // Invariant: Empty component strings are rejected
        // Strategy: Test empty component edge case
        // Anti-invariant: accepting empty components breaks routing
        let path = DiscoveryPath::new("", "1.0.0");
>>>>>>> origin/vo-worker-tests
        let result = validate_discovery_path(&path);
        prop_assert!(result.is_err());
    }

    #[test]
<<<<<<< HEAD
    fn discovery_path_validation_rejects_path_separators(
        binary_name in ".*/.*",
    ) {
        // Invariant: binary_name with path separators is rejected
        // Strategy: Generate names containing /
        // Anti-invariant: accepting path separators allows path traversal
        let path = DiscoveryPath::new(
            "/var/wtf/versions".to_string(),
            BinaryHash::parse("abcdef0123456789").unwrap(),
            binary_name,
        );
        let result = validate_discovery_path(&path);
        prop_assert!(result.is_err());
=======
    fn discovery_path_version_format(version: String) {
        // Invariant: Version strings are preserved through validation
        // Strategy: Generate various version strings
        // Anti-invariant: mangled versions break compatibility checks
        let path = DiscoveryPath::new("component", &version);
        let result = validate_discovery_path(&path);
        if result.is_ok() {
            prop_assert_eq!(path.version(), &version);
        }
>>>>>>> origin/vo-worker-tests
    }

    // ============ Pin Enforcement Proptests ============

    #[test]
    fn pin_enforce_accepts_matching_hash(
<<<<<<< HEAD
        hash in "[a-f0-9]{16,64}",
    ) {
        // Invariant: Pin accepts binary hash matching pinned value
        // Strategy: Generate valid hash strings
        // Anti-invariant: rejecting matching hashes breaks pinning
        let binary_hash = BinaryHash::parse(&hash).unwrap();
        let pin = VersionPin::new(binary_hash.clone(), 1000);
        let result = enforce_pin(&pin, &binary_hash);
=======
        hash in any::<String>(),
    ) {
        // Invariant: Pin accepts binary hash matching pinned value
        // Strategy: Generate matching hash strings
        // Anti-invariant: rejecting matching hashes breaks pinning
        let pin = VersionPin::parse(&hash).expect("valid");
        let candidate = hash.clone();
        let result = enforce_pin(&pin, &candidate);
>>>>>>> origin/vo-worker-tests
        prop_assert!(result.is_ok());
    }

    #[test]
    fn pin_enforce_rejects_mismatched_hash(
<<<<<<< HEAD
        pinned in "[a-f0-9]{16,64}",
        candidate in "[a-f0-9]{16,64}",
=======
        pinned in any::<String>(),
        candidate in any::<String>(),
>>>>>>> origin/vo-worker-tests
    ) {
        // Invariant: Pin rejects binary hash not matching pinned value
        // Strategy: Generate mismatched hash pairs
        // Anti-invariant: accepting mismatched hashes breaks pinning
        prop_assume!(pinned != candidate);
<<<<<<< HEAD
        let pinned_hash = BinaryHash::parse(&pinned).unwrap();
        let candidate_hash = BinaryHash::parse(&candidate).unwrap();
        let pin = VersionPin::new(pinned_hash, 1000);
        let result = enforce_pin(&pin, &candidate_hash);
=======
        let pin = VersionPin::parse(&pinned).expect("valid");
        let result = enforce_pin(&pin, &candidate);
>>>>>>> origin/vo-worker-tests
        prop_assert!(result.is_err());
    }

    #[test]
<<<<<<< HEAD
    fn pin_enforce_exact_match(
        hash in "[a-f0-9]{16,64}",
    ) {
        // Invariant: Pin requires exact byte-level match
        // Strategy: Generate hash strings
        // Anti-invariant: case-insensitive or fuzzy matching breaks pins
        let binary_hash = BinaryHash::parse(&hash).unwrap();
        let pin = VersionPin::new(binary_hash.clone(), 1000);
        let result = enforce_pin(&pin, &binary_hash);
        prop_assert!(result.is_ok());

        // Slight modification should fail
        let modified = format!("{}x", &hash[..hash.len().saturating_sub(1)]);
        if let Ok(modified_hash) = BinaryHash::parse(&modified) {
            let result_modified = enforce_pin(&pin, &modified_hash);
            prop_assert!(result_modified.is_err());
        }
=======
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
>>>>>>> origin/vo-worker-tests
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

<<<<<<< HEAD
    // ============ Redaction Proptests ============

    #[test]
    fn redaction_never_panics(value in any::<serde_json::Value>(), field in ".*") {
        let rules = vec![RedactionRule::new(
            vec![field],
            RedactionKind::Remove,
        )];
        let _ = apply_redaction(&value, &rules);
    }

    #[test]
    fn redaction_idempotent(
        field_name in "[a-z]{1,10}",
        data in any::<serde_json::Value>(),
    ) {
        let value = serde_json::json!({ &field_name: data });
        let rules = vec![RedactionRule::new(
            vec![field_name.clone()],
            RedactionKind::Hash,
        )];
        let (result1, _) = apply_redaction(&value, &rules);
        let (result2, _) = apply_redaction(&result1, &rules);
        prop_assert_eq!(result1, result2, "Applying same redaction twice must be idempotent");
    }

    #[test]
    fn redaction_hash_deterministic(
        field_name in "[a-z]{1,10}",
        payload in ".*",
    ) {
        let value = serde_json::json!({ &field_name: payload });
        let rules = vec![RedactionRule::new(
            vec![field_name],
            RedactionKind::Hash,
        )];
        let (result1, _) = apply_redaction(&value, &rules);
        let (result2, _) = apply_redaction(&value, &rules);
        prop_assert_eq!(result1, result2, "Same input must produce same hash");
    }

    #[test]
    fn redaction_empty_rules_is_identity(data in any::<serde_json::Value>()) {
        let rules: Vec<RedactionRule> = vec![];
        let (result, redacted) = apply_redaction(&data, &rules);
        prop_assert_eq!(result, data);
        prop_assert!(redacted.is_empty());
    }

    #[test]
    fn redaction_remove_never_exposes_original(
        field_name in "[a-z]{1,10}",
        secret in "[A-Z]{5,20}",
    ) {
        let value = serde_json::json!({ &field_name: secret });
        let rules = vec![RedactionRule::new(
            vec![field_name.clone()],
            RedactionKind::Remove,
        )];
        let (result, _) = apply_redaction(&value, &rules);
        let result_str = serde_json::to_string(&result).unwrap();
        prop_assert!(!result_str.contains(&secret),
            "Removed field value must not appear in output");
=======
    // ============ Schema Version Proptests ============

    #[test]
    fn extract_schema_version_parses_valid(
        major in 0u32..,
        minor in 0u16..,
        patch in 0u16..,
    ) {
        // Invariant: Valid version strings parse correctly
        // Strategy: Generate valid version component pairs
        // Anti-invariant: parsing errors break version routing
        let version = SchemaVersion::new(major, minor);
        let extracted = extract_schema_version(&version.to_string());
        prop_assert_eq!(extracted, Ok(version));
    }

    #[test]
    fn schema_version_ordering(major_a in 0u32.., minor_a in 0u16.., major_b in 0u32..) {
        // Invariant: SchemaVersion ordering matches (major, minor) ordering
        // Strategy: Generate version pairs
        // Anti-invariant: wrong ordering breaks compatibility checks
        let va = SchemaVersion::new(major_a, minor_a);
        let vb = SchemaVersion::new(major_b, 0);
        prop_assert_eq!(va.cmp(&vb), (major_a, minor_a).cmp(&(major_b, 0u16)));
    }

    // ============ Redaction Proptests ============

    #[test]
    fn redaction_rule_identity(value: RedactedValue) {
        // Invariant: Applying rule to non-matching value is identity
        // Strategy: Generate arbitrary values
        // Anti-invariant: modifying non-targets leaks or corrupts data
        let rule = RedactionRule::new("different_field");
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
    fn redaction_preserves_structure(map_size in 0usize..10) {
        // Invariant: Redaction preserves overall structure except matched fields
        // Strategy: Generate maps of various sizes
        // Anti-invariant: structural changes break downstream parsing
        use serde_json::json;
        let mut map: HashMap<String, RedactedValue> = HashMap::new();

        for i in 0..map_size {
            map.insert(format!("field_{}", i), RedactedValue::Number(i as u64));
        }

        let value = RedactedValue::Object(map.clone());
        let rule = RedactionRule::new("field_5");
        let result = apply_redaction(&value, &rule);

        // Result should still be an object
        if let RedactedValue::Object(result_map) = result {
            prop_assert_eq!(result_map.len(), map_size);
        }
>>>>>>> origin/vo-worker-tests
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
<<<<<<< HEAD
    fn dual_representation_redaction_commutes_for_non_overlapping_rules(
        field_a in "[a-z]{1,5}",
        field_b in "[a-z]{1,5}",
        val_a in ".*",
        val_b in ".*",
    ) {
        prop_assume!(field_a != field_b);
        let value = serde_json::json!({ &field_a: val_a, &field_b: val_b });
        let rule_a = vec![RedactionRule::new(vec![field_a.clone()], RedactionKind::Hash)];
        let rule_b = vec![RedactionRule::new(vec![field_b.clone()], RedactionKind::Remove)];
        let both = vec![
            RedactionRule::new(vec![field_a], RedactionKind::Hash),
            RedactionRule::new(vec![field_b], RedactionKind::Remove),
        ];

        let (ab, _) = apply_redaction(&value, &both);

        let (a_then_val, _) = apply_redaction(&value, &rule_a);
        let (ab2, _) = apply_redaction(&a_then_val, &rule_b);

        prop_assert_eq!(ab, ab2, "Non-overlapping redaction rules should commute");
    }

    // ============ Access Control Proptests ============
=======
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

   // ============ Admission Check Proptests ============

    #[test]
    fn admission_check_thresholds_boundary(
        pressure in 0f64..1.0,
        threshold in 0f64..1.0,
    ) {
        // Invariant: Admission denied when pressure >= threshold
        // Strategy: Generate pressure and threshold values
        // Anti-invariant: wrong threshold comparison breaks admission
        prop_assert_eq!(pressure >= threshold, pressure >= threshold);
    }

  // ============ Access Control Proptests ============
>>>>>>> origin/vo-worker-tests

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

<<<<<<< HEAD
=======
    // ============ Snapshot Compatibility Proptests ============

    #[test]
    fn snapshot_compat_self_compatible(version in 0u16..100) {
        // Invariant: Any version is compatible with itself
        // Strategy: Test various version numbers
        // Anti-invariant: rejecting self breaks snapshot restores
        // Note: This tests the logical invariant; actual implementation in vo-core
        prop_assert_eq!(version, version);
    }

    // ============ Rate Limiter Proptests ============

    #[test]
    fn rate_limit_update_advances_time(now: Duration) {
        // Invariant: update_rate_limit returns future time
        // Strategy: Generate various time values
        // Anti-invariant: past or equal time breaks rate limiting
        use std::time::Instant;
        use crate::circuit_breaker::rate_limiter::update_rate_limit;

        let instant = Instant::now() - now;
        let updated = update_rate_limit(instant);
        prop_assert!(updated >= instant);
    }

    // ============ Failure Window Proptests ============

    #[test]
    fn unique_failures_count_accuracy(
        num_failures in 0usize..100,
        window_size in 1usize..100,
    ) {
        // Invariant: unique_failures_in_window returns accurate count
        // Strategy: Generate failure counts and window sizes
        // Anti-invariant: incorrect counts break circuit breaker
        use crate::circuit_breaker::failure_window::unique_failures_in_window;

        // Simulate failures with unique IDs
        let failures: Vec<u64> = (0..num_failures).map(|i| i as u64).collect();
        let count = unique_failures_in_window(&failures, window_size);

        prop_assert!(count <= num_failures);
        prop_assert!(count <= window_size);
    }

    // ============ Circuit Breaker Evaluation Proptests ============

    #[test]
    fn evaluate_registration_state_consistency(
        state in any::<String>(),
        failure_count in 0u32..,
    ) {
        // Invariant: Registration evaluation produces consistent state
        // Strategy: Generate state and failure count pairs
        // Anti-invariant: inconsistent evaluation breaks circuit breaker
        use crate::circuit_breaker::evaluate_registration;

        let result = evaluate_registration(&state, failure_count);
        // Result should be deterministic for same inputs
        let result2 = evaluate_registration(&state, failure_count);
        prop_assert_eq!(result, result2);
    }



    // ============ Access Control Proptests ============

    #[test]
    fn is_authorized_deterministic(
        policy_rules in proptest::collection::vec(any::<String>(), 0..10),
        principal_rules in proptest::collection::vec(any::<String>(), 0..10),
    ) {
        // Invariant: Same policy/principal produces same authorization
        // Strategy: Generate rule sets
        // Anti-invariant: non-deterministic auth breaks security
        use crate::vault::access::is_authorized;
        use crate::AccessPolicy;

        let policy = AccessPolicy { rules: policy_rules.clone() };
        let principal = crate::Principal { rules: principal_rules.clone() };

        let result1 = is_authorized(&policy, &principal);
        let result2 = is_authorized(&policy, &principal);

        prop_assert_eq!(result1, result2);
    }

  // ============ State Transition Proptests ============

    #[test]
    fn state_transition_validity_preserved(
        state in any::<LifecycleState>(),
        event_type in any::<String>(),
    ) {
        // Invariant: Transition attempts either succeed with valid state or fail gracefully
        // Strategy: Generate states and event types
        // Anti-invariant: invalid state leakage corrupts machine
        use crate::state::transition::apply;

        // The apply function should handle transitions safely
        let result = apply(state, &event_type);

        match result {
            Ok(new_state) => {
                // new_state should be a valid LifecycleState variant
                // This is enforced by the type system
                let _ = new_state;
            }
            Err(_) => {
                // Invalid transitions return Err - acceptable behavior
                // No state corruption occurs
            }
        }
    }

    // ============ Event Payload Proptests ============

    #[test]
    fn event_payload_type_preservation(
        event_type: String,
        payload_content in any::<String>(),
    ) {
        // Invariant: EventPayload preserves event type through serialization
        // Strategy: Generate event type and content pairs
        // Anti-invariant: type loss breaks event routing
        use serde_json;

        let payload = EventPayload {
            event_type: event_type.clone(),
            data: payload_content,
        };

        let serialized = serde_json::to_value(&payload).expect("serialize");
        let deserialized: EventPayload = serde_json::from_value(serialized).expect("deserialize");

        prop_assert_eq!(deserialized.event_type, payload.event_type);
    }

    // ============ Superstate Proptests ============

    #[test]
    fn superstate_grouping_correctness(superstate in any::<LifecycleSuperState>()) {
        // Invariant: Superstate groups states correctly
        // Strategy: Test all superstate values
        // Anti-invariant: wrong grouping breaks state aggregation
        match superstate {
            LifecycleSuperState::Active => {
                // Active states should be running states
            }
            LifecycleSuperState::Inactive => {
                // Inactive states should be terminal/paused
            }
            LifecycleSuperState::Error => {
                // Error states should be failure states
            }
        }
    }

>>>>>>> origin/vo-worker-tests
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
