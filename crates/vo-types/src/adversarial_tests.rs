use crate::*;
use std::num::NonZeroU64;

// --- Unicode edge cases ---

#[test]
fn rq_workflow_name_rejects_emoji() {
    let result = WorkflowName::parse("deploy-rocket-\u{1F680}");
    assert!(result.is_err(), "WorkflowName should reject emoji");
}

#[test]
fn rq_node_name_rejects_emoji() {
    let result = NodeName::parse("compile-\u{1F525}");
    assert!(result.is_err(), "NodeName should reject emoji");
}

#[test]
fn rq_workflow_name_rejects_zero_width_space() {
    let result = WorkflowName::parse("deploy\u{200B}prod");
    assert!(
        result.is_err(),
        "WorkflowName should reject zero-width space"
    );
}

#[test]
fn rq_workflow_name_rejects_zero_width_joiner() {
    let result = WorkflowName::parse("deploy\u{200D}prod");
    assert!(
        result.is_err(),
        "WorkflowName should reject zero-width joiner"
    );
}

#[test]
fn rq_workflow_name_rejects_right_to_left_mark() {
    let result = WorkflowName::parse("deploy\u{200F}prod");
    assert!(
        result.is_err(),
        "WorkflowName should reject right-to-left mark"
    );
}

#[test]
fn rq_workflow_name_rejects_fullwidth_digit() {
    let result = WorkflowName::parse("deploy-\u{FF12}");
    assert!(
        result.is_err(),
        "WorkflowName should reject fullwidth digit"
    );
}

#[test]
fn rq_node_name_rejects_null_byte() {
    let result = NodeName::parse("compile\x00artifact");
    assert!(result.is_err(), "NodeName should reject null byte");
}

#[test]
fn rq_workflow_name_rejects_tab() {
    let result = WorkflowName::parse("deploy\tprod");
    assert!(result.is_err(), "WorkflowName should reject tab");
}

#[test]
fn rq_workflow_name_rejects_newline() {
    let result = WorkflowName::parse("deploy\nprod");
    assert!(result.is_err(), "WorkflowName should reject newline");
}

#[test]
fn rq_workflow_name_rejects_carriage_return() {
    let result = WorkflowName::parse("deploy\rprod");
    assert!(
        result.is_err(),
        "WorkflowName should reject carriage return"
    );
}

// --- InstanceId Crockford Base32 edge cases ---

#[test]
fn rq_instance_id_accepts_lowercase_ulid() {
    let result = InstanceId::parse("01h5jyv4xhgsr2f8kz9bwnrfma");
    let val = result.expect("lowercase ULID should be accepted");
    assert_eq!(val.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
}

#[test]
fn rq_instance_id_accepts_mixed_case_ulid() {
    let result = InstanceId::parse("01H5jyv4XHGSR2F8KZ9BWNRFMA");
    let val = result.expect("mixed-case ULID should be accepted");
    assert_eq!(val.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
}

#[test]
fn rq_instance_id_normalizes_case_to_uppercase() {
    let lower = InstanceId::parse("01h5jyv4xhgsr2f8kz9bwnrfma").expect("valid");
    let upper = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid");
    assert_eq!(lower, upper, "different case = same normalized InstanceId");
}

#[test]
fn rq_instance_id_rejects_25_chars() {
    let result = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFM");
    assert!(result.is_err(), "InstanceId should reject 25-char string");
}

#[test]
fn rq_instance_id_rejects_27_chars() {
    let result = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMAA");
    assert!(result.is_err(), "InstanceId should reject 27-char string");
}

// --- BinaryHash edge cases ---

#[test]
fn rq_binary_hash_rejects_single_char() {
    let result = BinaryHash::parse("a");
    assert!(result.is_err(), "BinaryHash should reject single char");
}

#[test]
fn rq_binary_hash_rejects_7_chars_odd() {
    let result = BinaryHash::parse("abcdef0");
    assert!(
        result.is_err(),
        "BinaryHash should reject 7-char (odd length) string"
    );
}

#[test]
fn rq_binary_hash_rejects_2_chars_below_min() {
    let result = BinaryHash::parse("ab");
    assert!(
        result.is_err(),
        "BinaryHash should reject 2-char string (below minimum 8)"
    );
}

#[test]
fn rq_binary_hash_rejects_4_chars_below_min() {
    let result = BinaryHash::parse("abcd");
    assert!(
        result.is_err(),
        "BinaryHash should reject 4-char string (below minimum 8)"
    );
}

#[test]
fn rq_binary_hash_rejects_6_chars_even_below_min() {
    let result = BinaryHash::parse("abcdef");
    assert!(
        result.is_err(),
        "BinaryHash should reject 6-char string (below minimum 8)"
    );
}

// --- Integer edge cases ---

#[test]
fn rq_empty_string_for_integer_types() {
    macro_rules! assert_empty {
        ($type:ty, $tn:expr) => {
            let result = <$type>::parse("");
            assert!(matches!(
                result,
                Err(ParseError::NotAnInteger { type_name, input }) if type_name == $tn && input.is_empty()
            ));
        };
    }
    assert_empty!(SequenceNumber, "SequenceNumber");
    assert_empty!(EventVersion, "EventVersion");
    assert_empty!(AttemptNumber, "AttemptNumber");
    assert_empty!(TimeoutMs, "TimeoutMs");
    assert_empty!(DurationMs, "DurationMs");
    assert_empty!(TimestampMs, "TimestampMs");
    assert_empty!(FireAtMs, "FireAtMs");
    assert_empty!(MaxAttempts, "MaxAttempts");
}

#[test]
fn rq_negative_zero_rejected() {
    macro_rules! assert_neg_zero {
        ($type:ty, $tn:expr) => {
            assert!(matches!(
                <$type>::parse("-0"),
                Err(ParseError::NotAnInteger { .. })
            ));
        };
    }
    assert_neg_zero!(SequenceNumber, "SequenceNumber");
    assert_neg_zero!(EventVersion, "EventVersion");
    assert_neg_zero!(AttemptNumber, "AttemptNumber");
    assert_neg_zero!(TimeoutMs, "TimeoutMs");
    assert_neg_zero!(MaxAttempts, "MaxAttempts");
}

#[test]
fn rq_very_long_integer_parses() {
    let result = SequenceNumber::parse("00000000000000000001");
    assert_eq!(result.map(|v| v.as_u64()), Ok(1));
}

#[test]
fn rq_plus_prefix_accepted_by_u64_from_str() {
    let result = SequenceNumber::parse("+42");
    let val = result.expect("+42 accepted by u64::from_str");
    assert_eq!(val.as_u64(), 42);
}

#[test]
fn rq_whitespace_only_rejected() {
    assert!(
        SequenceNumber::parse(" ").is_err(),
        "SequenceNumber should reject whitespace-only"
    );
    assert!(
        DurationMs::parse(" ").is_err(),
        "DurationMs should reject whitespace-only"
    );
}

#[test]
fn rq_scientific_notation_rejected() {
    assert!(
        SequenceNumber::parse("1e5").is_err(),
        "SequenceNumber should reject scientific notation"
    );
    assert!(
        DurationMs::parse("1e5").is_err(),
        "DurationMs should reject scientific notation"
    );
}

// --- TimerId/IdempotencyKey: opaque type behavior ---

#[test]
fn rq_timer_id_accepts_null_byte() {
    let result = TimerId::parse("timer\x00id");
    let val = result.expect("TimerId is opaque and should accept null byte");
    assert!(val.as_str().contains('\x00'));
}

#[test]
fn rq_timer_id_accepts_newlines() {
    let result = TimerId::parse("timer\nid");
    let val = result.expect("TimerId is opaque and should accept newlines");
    assert!(val.as_str().contains('\n'));
}

#[test]
fn rq_idempotency_key_accepts_null_byte() {
    let result = IdempotencyKey::parse("key\x00val");
    let val = result.expect("IdempotencyKey is opaque and should accept null byte");
    assert!(val.as_str().contains('\x00'));
}

#[test]
fn rq_timer_id_null_byte_serde_round_trip() {
    let original = TimerId::parse("timer\x00id").expect("parse");
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: TimerId = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn rq_timer_id_rejects_257_ascii() {
    let input = "a".repeat(257);
    assert!(
        TimerId::parse(&input).is_err(),
        "TimerId should reject 257 ASCII chars"
    );
}

#[test]
fn rq_timer_id_accepts_256_multi_byte() {
    let input = "\u{1F600}".repeat(256);
    assert_eq!(input.chars().count(), 256);
    let result = TimerId::parse(&input);
    let val = result.expect("TimerId should accept 256 multi-byte chars");
    assert_eq!(val.as_str().chars().count(), 256);
}

#[test]
fn rq_idempotency_key_rejects_1025_ascii() {
    let input = "b".repeat(1025);
    assert!(
        IdempotencyKey::parse(&input).is_err(),
        "IdempotencyKey should reject 1025 ASCII chars"
    );
}

// --- Trait checks ---

#[test]
fn rq_string_types_are_not_copy() {
    fn assert_not_copy<T: Clone>(_v: T) {}
    assert_not_copy(InstanceId("test".to_string()));
    assert_not_copy(WorkflowName("test".to_string()));
    assert_not_copy(NodeName("test".to_string()));
    assert_not_copy(BinaryHash("test".to_string()));
    assert_not_copy(TimerId("test".to_string()));
    assert_not_copy(IdempotencyKey("test".to_string()));
}

#[test]
fn rq_integer_types_are_copy() {
    fn require_copy<T: Copy>(_v: T) {}
    require_copy(SequenceNumber::new_unchecked(1));
    require_copy(EventVersion::new_unchecked(1));
    require_copy(AttemptNumber::new_unchecked(1));
    require_copy(TimeoutMs::new_unchecked(1));
    require_copy(MaxAttempts::new_unchecked(1));
    require_copy(DurationMs(1));
    require_copy(TimestampMs(1));
    require_copy(FireAtMs(1));
}

#[test]
fn rq_debug_output_contains_type_name() {
    let id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid");
    let debug = format!("{:?}", id);
    assert!(debug.contains("InstanceId"));
}

#[test]
fn rq_debug_output_contains_value_for_string() {
    let wn = WorkflowName::parse("deploy-prod").expect("valid");
    let debug = format!("{:?}", wn);
    assert!(debug.contains("deploy-prod"));
}

#[test]
fn rq_debug_output_contains_value_for_integer() {
    let sn = SequenceNumber::new_unchecked(42);
    let debug = format!("{:?}", sn);
    assert!(debug.contains("42"));
}

// --- Error type_name correctness ---

#[test]
fn rq_error_type_name_matches_for_all_types() {
    assert!(matches!(
        InstanceId::parse(""),
        Err(ParseError::Empty { type_name }) if type_name == "InstanceId"
    ));
    assert!(matches!(
        WorkflowName::parse(""),
        Err(ParseError::Empty { type_name }) if type_name == "WorkflowName"
    ));
    assert!(matches!(
        NodeName::parse(""),
        Err(ParseError::Empty { type_name }) if type_name == "NodeName"
    ));
    assert!(matches!(
        BinaryHash::parse(""),
        Err(ParseError::Empty { type_name }) if type_name == "BinaryHash"
    ));
    assert!(matches!(
        TimerId::parse(""),
        Err(ParseError::Empty { type_name }) if type_name == "TimerId"
    ));
    assert!(matches!(
        IdempotencyKey::parse(""),
        Err(ParseError::Empty { type_name }) if type_name == "IdempotencyKey"
    ));
    assert!(matches!(
        SequenceNumber::parse("abc"),
        Err(ParseError::NotAnInteger { type_name, .. }) if type_name == "SequenceNumber"
    ));
    assert!(matches!(
        DurationMs::parse("abc"),
        Err(ParseError::NotAnInteger { type_name, .. }) if type_name == "DurationMs"
    ));
}

// --- Boundary combos ---

#[test]
fn rq_workflow_name_hyphen_underscore_combo() {
    assert!(
        WorkflowName::parse("-_").is_err(),
        "WorkflowName should reject \"-_\""
    );
}

#[test]
fn rq_workflow_name_underscore_hyphen_combo() {
    assert!(
        WorkflowName::parse("_-").is_err(),
        "WorkflowName should reject \"_-\""
    );
}

#[test]
fn rq_node_name_double_hyphen_middle_valid() {
    let result = NodeName::parse("compile--artifact");
    let val = result.expect("NodeName should accept double hyphen in middle");
    assert_eq!(val.as_str(), "compile--artifact");
}

#[test]
fn rq_node_name_double_underscore_middle_valid() {
    let result = NodeName::parse("compile__artifact");
    let val = result.expect("NodeName should accept double underscore in middle");
    assert_eq!(val.as_str(), "compile__artifact");
}

// --- as_str lifetime ---

#[test]
fn rq_as_str_borrowed_from_struct() {
    let wn = WorkflowName::parse("test").expect("valid");
    let s = wn.as_str();
    assert!(std::ptr::eq(s.as_ptr(), wn.0.as_ptr()));
}

// --- TryFrom<u64> for NonZero types ---

#[test]
fn rq_try_from_u64_rejects_zero() {
    assert!(
        SequenceNumber::try_from(0u64).is_err(),
        "SequenceNumber should reject zero"
    );
    assert!(
        EventVersion::try_from(0u64).is_err(),
        "EventVersion should reject zero"
    );
    assert!(
        AttemptNumber::try_from(0u64).is_err(),
        "AttemptNumber should reject zero"
    );
    assert!(
        TimeoutMs::try_from(0u64).is_err(),
        "TimeoutMs should reject zero"
    );
    assert!(
        MaxAttempts::try_from(0u64).is_err(),
        "MaxAttempts should reject zero"
    );
}

#[test]
fn rq_try_from_u64_accepts_one() {
    SequenceNumber::try_from(1u64).expect("SequenceNumber should accept 1");
    EventVersion::try_from(1u64).expect("EventVersion should accept 1");
    AttemptNumber::try_from(1u64).expect("AttemptNumber should accept 1");
    TimeoutMs::try_from(1u64).expect("TimeoutMs should accept 1");
    MaxAttempts::try_from(1u64).expect("MaxAttempts should accept 1");
}

#[test]
fn rq_try_from_u64_accepts_max() {
    SequenceNumber::try_from(u64::MAX).expect("SequenceNumber should accept u64::MAX");
    EventVersion::try_from(u64::MAX).expect("EventVersion should accept u64::MAX");
    AttemptNumber::try_from(u64::MAX).expect("AttemptNumber should accept u64::MAX");
    TimeoutMs::try_from(u64::MAX).expect("TimeoutMs should accept u64::MAX");
    MaxAttempts::try_from(u64::MAX).expect("MaxAttempts should accept u64::MAX");
}

// --- From<T> no bypass ---

#[test]
fn rq_no_from_string_for_string_types() {
    let s: String = String::from(WorkflowName("test".to_string()));
    assert_eq!(s, "test");
}

// --- Ord consistency ---

#[test]
fn rq_ord_consistent_with_as_u64() {
    let a = SequenceNumber::new_unchecked(1);
    let b = SequenceNumber::new_unchecked(100);
    assert!(a < b);
    assert!(b > a);

    let a = DurationMs(1);
    let b = DurationMs(100);
    assert!(a < b);
    assert!(b > a);

    let a = TimestampMs(0);
    let b = TimestampMs(u64::MAX);
    assert!(a < b);
}

// --- InstanceId display round-trip ---

#[test]
fn instance_id_display_round_trip() {
    let id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid");
    let s = format!("{}", id);
    let result = InstanceId::parse(&s);
    assert_eq!(result, Ok(id));
}

// --- Proptest invariants ---

mod proptests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{Duration, SystemTime};

    proptest! {
        #[test]
        fn instance_id_round_trip(s in "[0-7][0-9A-HJKMNP-TV-Z]{25}") {
            let v = InstanceId(s);
            let result = InstanceId::parse(&v.to_string());
            prop_assert_eq!(result, Ok(v));
        }

        #[test]
        fn workflow_name_round_trip(s in "[a-zA-Z0-9][a-zA-Z0-9_-]{0,126}[a-zA-Z0-9]") {
            let v = WorkflowName(s);
            let result = WorkflowName::parse(&v.to_string());
            prop_assert_eq!(result, Ok(v));
        }

        #[test]
        fn node_name_round_trip(s in "[a-zA-Z0-9][a-zA-Z0-9_-]{0,126}[a-zA-Z0-9]") {
            let v = NodeName(s);
            let result = NodeName::parse(&v.to_string());
            prop_assert_eq!(result, Ok(v));
        }

        #[test]
        fn binary_hash_round_trip(s in "[0-9a-f]{8,256}") {
            let s = if s.len() % 2 != 0 { format!("0{}", s) } else { s };
            let v = BinaryHash(s);
            let result = BinaryHash::parse(&v.to_string());
            prop_assert_eq!(result, Ok(v));
        }

        #[test]
        fn sequence_number_round_trip(value in 1u64..) {
            let v = SequenceNumber(NonZeroU64::new(value).expect("nonzero value from proptest strategy"));
            let result = SequenceNumber::parse(&v.to_string());
            prop_assert_eq!(result, Ok(v));
        }

        #[test]
        fn event_version_round_trip(value in 1u64..) {
            let v = EventVersion(NonZeroU64::new(value).expect("nonzero value from proptest strategy"));
            let result = EventVersion::parse(&v.to_string());
            prop_assert_eq!(result, Ok(v));
        }

        #[test]
        fn attempt_number_round_trip(value in 1u64..) {
            let v = AttemptNumber(NonZeroU64::new(value).expect("nonzero value from proptest strategy"));
            let result = AttemptNumber::parse(&v.to_string());
            prop_assert_eq!(result, Ok(v));
        }

        #[test]
        fn timer_id_round_trip(s in ".{1,256}") {
            let v = TimerId(s);
            let result = TimerId::parse(&v.to_string());
            prop_assert_eq!(result, Ok(v));
        }

        #[test]
        fn idempotency_key_round_trip(s in ".{1,1024}") {
            let v = IdempotencyKey(s);
            let result = IdempotencyKey::parse(&v.to_string());
            prop_assert_eq!(result, Ok(v));
        }

        #[test]
        fn timeout_ms_round_trip(value in 1u64..) {
            let v = TimeoutMs(NonZeroU64::new(value).expect("nonzero value from proptest strategy"));
            let result = TimeoutMs::parse(&v.to_string());
            prop_assert_eq!(result, Ok(v));
        }

        #[test]
        fn duration_ms_round_trip(value in 0u64..) {
            let v = DurationMs(value);
            let result = DurationMs::parse(&v.to_string());
            prop_assert_eq!(result, Ok(v));
        }

        #[test]
        fn timestamp_ms_round_trip(value in 0u64..) {
            let v = TimestampMs(value);
            let result = TimestampMs::parse(&v.to_string());
            prop_assert_eq!(result, Ok(v));
        }

        #[test]
        fn fire_at_ms_round_trip(value in 0u64..) {
            let v = FireAtMs(value);
            let result = FireAtMs::parse(&v.to_string());
            prop_assert_eq!(result, Ok(v));
        }

        #[test]
        fn max_attempts_round_trip(value in 1u64..) {
            let v = MaxAttempts(NonZeroU64::new(value).expect("nonzero value from proptest strategy"));
            let result = MaxAttempts::parse(&v.to_string());
            prop_assert_eq!(result, Ok(v));
        }

        #[test]
        fn string_newtype_display_is_identity(s in "[a-zA-Z0-9][a-zA-Z0-9_-]{0,126}[a-zA-Z0-9]") {
            let v = WorkflowName(s.clone());
            prop_assert_eq!(v.to_string(), v.as_str());
        }

        #[test]
        fn integer_newtype_display_is_decimal(value in 1u64..) {
            let v = SequenceNumber(NonZeroU64::new(value).expect("nonzero value from proptest strategy"));
            prop_assert_eq!(v.to_string(), v.as_u64().to_string());
        }

        #[test]
        fn hash_consistency(a in 1u64.., b in 1u64..) {
            let va = SequenceNumber(NonZeroU64::new(a).expect("nonzero value from proptest strategy"));
            let vb = SequenceNumber(NonZeroU64::new(b).expect("nonzero value from proptest strategy"));
            let mut h1 = DefaultHasher::new();
            va.hash(&mut h1);
            let ha = h1.finish();
            let mut h2 = DefaultHasher::new();
            vb.hash(&mut h2);
            let hb = h2.finish();
            if va == vb { prop_assert_eq!(ha, hb); }
        }

        #[test]
        fn clone_equality(s in "[a-zA-Z0-9][a-zA-Z0-9_-]{0,126}[a-zA-Z0-9]") {
            let v = WorkflowName(s);
            prop_assert_eq!(v.clone(), v);
        }

        #[test]
        fn copy_equal(value in 1u64..) {
            let v = SequenceNumber(NonZeroU64::new(value).expect("nonzero value from proptest strategy"));
            let copy = v;
            prop_assert_eq!(copy, v);
        }

        #[test]
        fn ord_consistent(a in 1u64.., b in 1u64..) {
            let va = SequenceNumber(NonZeroU64::new(a).expect("nonzero value from proptest strategy"));
            let vb = SequenceNumber(NonZeroU64::new(b).expect("nonzero value from proptest strategy"));
            prop_assert_eq!(va.cmp(&vb), a.cmp(&b));
        }

        #[test]
        fn serde_round_trip_duration_ms(value in 0u64..) {
            let v = DurationMs(value);
            let json = serde_json::to_value(v).expect("serialize DurationMs in proptest");
            let restored: DurationMs = serde_json::from_value(json).expect("deserialize DurationMs in proptest");
            prop_assert_eq!(restored, v);
        }

        #[test]
        fn serde_round_trip_sequence_number(value in 1u64..) {
            let v = SequenceNumber(NonZeroU64::new(value).expect("nonzero value from proptest strategy"));
            let json = serde_json::to_value(v).expect("serialize SequenceNumber in proptest");
            let restored: SequenceNumber = serde_json::from_value(json).expect("deserialize SequenceNumber in proptest");
            prop_assert_eq!(restored, v);
        }

        #[test]
        fn timeout_ms_to_duration(value in 1u64..) {
            let v = TimeoutMs(NonZeroU64::new(value).expect("nonzero value from proptest strategy"));
            prop_assert_eq!(v.to_duration(), Duration::from_millis(value));
        }

        #[test]
        fn duration_ms_to_duration(value in 0u64..) {
            let v = DurationMs(value);
            prop_assert_eq!(v.to_duration(), Duration::from_millis(value));
        }

        #[test]
        fn timestamp_ms_to_system_time(value in 0u64..) {
            let v = TimestampMs(value);
            prop_assert_eq!(v.to_system_time(), SystemTime::UNIX_EPOCH + Duration::from_millis(value));
        }

        #[test]
        fn fire_at_ms_has_elapsed(fire_at in 0u64.., now in 0u64..) {
            let f = FireAtMs(fire_at);
            let n = TimestampMs(now);
            prop_assert_eq!(f.has_elapsed(n), fire_at < now);
        }

        #[test]
        fn max_attempts_is_exhausted(max_val in 1u64.., attempt_val in 1u64..) {
            let m = MaxAttempts(NonZeroU64::new(max_val).expect("nonzero value from proptest strategy"));
            let a = AttemptNumber(NonZeroU64::new(attempt_val).expect("nonzero value from proptest strategy"));
            prop_assert_eq!(m.is_exhausted(a), attempt_val >= max_val);
        }
    }
}
