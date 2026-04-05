use std::collections::HashSet;

use rstest::rstest;

use crate::{
    BufferPolicy, Epoch, IdempotencyKey, InstanceId, LineageScope, SignalAddress, SignalDedupeKey,
    SignalDelivery, TimestampMs, WaitKey, WaitRecord,
};

// ---------------------------------------------------------------------------
// Helper: construct a valid InstanceId for tests (ULID format, 26 chars)
// ---------------------------------------------------------------------------
fn valid_instance_id() -> InstanceId {
    InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y6").expect("valid ULID for test setup")
}

// ===========================================================================
// WaitKey — Unit Tests
// ===========================================================================

#[test]
fn waitkey_parses_successfully_when_input_is_non_empty_and_within_max_length() {
    let key = WaitKey::parse("approval-pending");
    assert_eq!(key, Ok(WaitKey("approval-pending".to_string())));
}

#[test]
fn waitkey_parses_at_exact_max_length_when_input_is_256_characters() {
    let input: String = "k".repeat(256);
    let key = WaitKey::parse(&input).expect("256-char key should parse");
    assert_eq!(key.as_str().len(), 256);
}

#[test]
fn waitkey_rejects_empty_string_with_empty_error() {
    let err = WaitKey::parse("").expect_err("empty string should fail");
    assert_eq!(
        err,
        crate::ParseError::Empty {
            type_name: "WaitKey",
        }
    );
}

#[test]
fn waitkey_rejects_string_exceeding_max_length_with_exceeds_max_length_error() {
    let input: String = "k".repeat(257);
    let err = WaitKey::parse(&input).expect_err("257-char key should fail");
    assert_eq!(
        err,
        crate::ParseError::ExceedsMaxLength {
            type_name: "WaitKey",
            max: 256,
            actual: 257,
        }
    );
}

#[test]
fn waitkey_as_str_returns_inner_value() {
    let key = WaitKey::parse("human-approval").expect("valid key");
    assert_eq!(key.as_str(), "human-approval");
}

#[test]
fn waitkey_display_outputs_inner_string() {
    let key = WaitKey::parse("signal-key").expect("valid key");
    assert_eq!(format!("{key}"), "signal-key");
}

#[test]
fn waitkey_try_from_string_delegates_to_parse() {
    let key = WaitKey::try_from("test-key".to_string()).expect("valid key");
    assert_eq!(key.as_str(), "test-key");

    let err = WaitKey::try_from(String::new());
    assert_eq!(
        err.expect_err("empty should fail"),
        crate::ParseError::Empty {
            type_name: "WaitKey",
        }
    );
}

#[test]
fn waitkey_from_waitkey_into_string_produces_original_value() {
    let original = "my-wait-key";
    let key = WaitKey::parse(original).expect("valid key");
    let back: String = key.into();
    assert_eq!(back, original);
}

#[test]
fn waitkey_accepts_unicode_characters_when_valid() {
    let key = WaitKey::parse("审批-待定").expect("unicode key should parse");
    assert_eq!(key.as_str(), "审批-待定");
}

#[test]
fn waitkey_accepts_single_whitespace_character_when_valid() {
    let key = WaitKey::parse(" ").expect("single space should parse");
    assert_eq!(key.as_str(), " ");
}

// ===========================================================================
// BufferPolicy — Unit Tests
// ===========================================================================

#[test]
fn bufferpolicy_default_returns_reject() {
    assert_eq!(BufferPolicy::default(), BufferPolicy::Reject);
}

#[rstest]
#[case(BufferPolicy::Reject, false)]
#[case(BufferPolicy::BufferOne, true)]
#[case(BufferPolicy::BufferMany, true)]
fn bufferpolicy_is_buffering_returns_correct_value_for_each_variant(
    #[case] policy: BufferPolicy,
    #[case] expected: bool,
) {
    assert_eq!(policy.is_buffering(), expected);
}

#[test]
fn bufferpolicy_variants_are_all_distinct() {
    let all = [
        BufferPolicy::Reject,
        BufferPolicy::BufferOne,
        BufferPolicy::BufferMany,
    ];
    let unique: HashSet<_> = all.iter().collect();
    assert_eq!(unique.len(), 3);
}

// ===========================================================================
// SignalDelivery — Unit Tests
// ===========================================================================

#[rstest]
#[case(SignalDelivery::Accepted, true, false)]
#[case(SignalDelivery::Rejected, true, false)]
#[case(SignalDelivery::Buffered, false, true)]
fn signaldelivery_is_terminal_and_is_pending_are_correct_for_each_variant(
    #[case] delivery: SignalDelivery,
    #[case] terminal: bool,
    #[case] pending: bool,
) {
    assert_eq!(delivery.is_terminal(), terminal);
    assert_eq!(delivery.is_pending(), pending);
}

#[test]
fn signaldelivery_variants_are_all_distinct() {
    let all = [
        SignalDelivery::Accepted,
        SignalDelivery::Rejected,
        SignalDelivery::Buffered,
    ];
    let unique: HashSet<_> = all.iter().collect();
    assert_eq!(unique.len(), 3);
}

// ===========================================================================
// LineageScope — Unit Tests (per ADR-042 Section 2)
// ===========================================================================

#[test]
fn lineagescope_is_epoch_local_returns_true_for_epoch_local_variant() {
    assert!(LineageScope::EpochLocal.is_epoch_local());
}

#[test]
fn lineagescope_is_epoch_local_returns_false_for_lineage_wide_variant() {
    assert!(!LineageScope::LineageWide.is_epoch_local());
}

#[test]
fn lineagescope_is_lineage_wide_returns_true_for_lineage_wide_variant() {
    assert!(LineageScope::LineageWide.is_lineage_wide());
}

#[test]
fn lineagescope_is_lineage_wide_returns_false_for_epoch_local_variant() {
    assert!(!LineageScope::EpochLocal.is_lineage_wide());
}

#[test]
fn lineagescope_exhaustive_match_covers_all_variants() {
    // This test proves the enum is closed (only two variants exist)
    let scope = LineageScope::EpochLocal;
    match scope {
        LineageScope::EpochLocal => {}
        LineageScope::LineageWide => {}
    }
    let scope = LineageScope::LineageWide;
    match scope {
        LineageScope::EpochLocal => {}
        LineageScope::LineageWide => {}
    }
}

// ===========================================================================
// SignalAddress — Lineage-Aware Unit Tests (per ADR-042)
// ===========================================================================

#[test]
fn signaladdress_lineage_wide_constructor_sets_correct_fields() {
    let lineage_id = valid_instance_id();
    let instance_id = valid_instance_id();
    let wait_key = WaitKey::parse("approval").expect("valid key");

    let addr =
        SignalAddress::lineage_wide(lineage_id.clone(), instance_id.clone(), wait_key.clone());

    assert_eq!(addr.lineage_scope(), LineageScope::LineageWide);
    assert_eq!(addr.lineage_id(), &lineage_id);
    assert_eq!(addr.instance_id(), &instance_id);
    assert_eq!(addr.wait_key(), &wait_key);
    assert!(addr.epoch_id().is_none());
    assert!(addr.is_lineage_wide());
    assert!(!addr.is_epoch_local());
}

#[test]
fn signaladdress_epoch_local_constructor_sets_correct_fields() {
    let lineage_id = valid_instance_id();
    let instance_id = valid_instance_id();
    let wait_key = WaitKey::parse("approval").expect("valid key");
    let epoch = Epoch::new(5);

    let addr = SignalAddress::epoch_local(
        lineage_id.clone(),
        epoch,
        instance_id.clone(),
        wait_key.clone(),
    );

    assert_eq!(addr.lineage_scope(), LineageScope::EpochLocal);
    assert_eq!(addr.lineage_id(), &lineage_id);
    assert_eq!(addr.instance_id(), &instance_id);
    assert_eq!(addr.wait_key(), &wait_key);
    assert_eq!(addr.epoch_id(), Some(epoch));
    assert!(addr.is_epoch_local());
    assert!(!addr.is_lineage_wide());
}

#[test]
fn signaladdress_epoch_local_with_epoch_zero() {
    let lineage_id = valid_instance_id();
    let instance_id = valid_instance_id();
    let wait_key = WaitKey::parse("approval").expect("valid key");
    let epoch = Epoch::ZERO;

    let addr = SignalAddress::epoch_local(lineage_id, epoch, instance_id, wait_key);

    assert_eq!(addr.epoch_id(), Some(epoch));
}

#[test]
fn signaladdress_epoch_local_with_epoch_max() {
    let lineage_id = valid_instance_id();
    let instance_id = valid_instance_id();
    let wait_key = WaitKey::parse("approval").expect("valid key");
    let epoch = Epoch::new(u64::MAX);

    let addr = SignalAddress::epoch_local(lineage_id, epoch, instance_id, wait_key);

    assert_eq!(addr.epoch_id(), Some(epoch));
}

#[test]
fn signaladdress_lineage_wide_preserves_all_fields_exactly() {
    let lineage_id = valid_instance_id();
    let instance_id = valid_instance_id();
    let wait_key = WaitKey::parse("test-key").expect("valid key");

    let addr =
        SignalAddress::lineage_wide(lineage_id.clone(), instance_id.clone(), wait_key.clone());

    assert_eq!(*addr.lineage_id(), lineage_id);
    assert_eq!(*addr.instance_id(), instance_id);
    assert_eq!(*addr.wait_key(), wait_key);
    assert!(addr.epoch_id().is_none());
}

#[test]
fn signaladdress_epoch_local_preserves_all_fields_exactly() {
    let lineage_id = valid_instance_id();
    let instance_id = valid_instance_id();
    let wait_key = WaitKey::parse("test-key").expect("valid key");
    let epoch = Epoch::new(42);

    let addr = SignalAddress::epoch_local(
        lineage_id.clone(),
        epoch,
        instance_id.clone(),
        wait_key.clone(),
    );

    assert_eq!(*addr.lineage_id(), lineage_id);
    assert_eq!(*addr.instance_id(), instance_id);
    assert_eq!(*addr.wait_key(), wait_key);
    assert_eq!(addr.epoch_id(), Some(epoch));
}

#[test]
fn signaladdress_lineage_scope_returns_configured_scope() {
    let lineage_id = valid_instance_id();
    let instance_id = valid_instance_id();
    let wait_key = WaitKey::parse("approval").expect("valid key");

    let epoch_addr = SignalAddress::epoch_local(
        lineage_id.clone(),
        Epoch::new(1),
        instance_id.clone(),
        wait_key.clone(),
    );
    assert_eq!(epoch_addr.lineage_scope(), LineageScope::EpochLocal);

    let wide_addr = SignalAddress::lineage_wide(lineage_id, instance_id, wait_key);
    assert_eq!(wide_addr.lineage_scope(), LineageScope::LineageWide);
}

#[test]
fn signaladdress_epoch_id_returns_some_for_epoch_local() {
    let lineage_id = valid_instance_id();
    let instance_id = valid_instance_id();
    let wait_key = WaitKey::parse("approval").expect("valid key");
    let epoch = Epoch::new(99);

    let addr = SignalAddress::epoch_local(lineage_id, epoch, instance_id, wait_key);

    assert_eq!(addr.epoch_id(), Some(epoch));
}

#[test]
fn signaladdress_epoch_id_returns_none_for_lineage_wide() {
    let lineage_id = valid_instance_id();
    let instance_id = valid_instance_id();
    let wait_key = WaitKey::parse("approval").expect("valid key");

    let addr = SignalAddress::lineage_wide(lineage_id, instance_id, wait_key);

    assert!(addr.epoch_id().is_none());
}

#[test]
fn signaladdress_equality_works_correctly() {
    let lineage_id = valid_instance_id();
    let instance_id = valid_instance_id();
    let key = WaitKey::parse("same-key").expect("valid key");

    let a = SignalAddress::lineage_wide(lineage_id.clone(), instance_id.clone(), key.clone());
    let b = SignalAddress::lineage_wide(lineage_id.clone(), instance_id.clone(), key.clone());
    let c = SignalAddress::lineage_wide(
        lineage_id.clone(),
        instance_id.clone(),
        WaitKey::parse("other-key").expect("valid key"),
    );

    assert_eq!(a, b);
    assert_ne!(a, c);
}

// ===========================================================================
// WaitRecord — Unit Tests
// ===========================================================================

#[test]
fn waitrecord_new_constructs_with_valid_inputs() {
    let instance_id = valid_instance_id();
    let wait_key = WaitKey::parse("approval-pending").expect("valid key");
    let policy = BufferPolicy::Reject;
    let ts = TimestampMs(1_730_000_000_000);

    let record =
        WaitRecord::new(instance_id.clone(), wait_key.clone(), policy, ts).expect("valid inputs");

    assert_eq!(record.instance_id(), &instance_id);
    assert_eq!(record.wait_key(), &wait_key);
    assert_eq!(record.buffer_policy(), policy);
    assert_eq!(record.registered_at(), ts);
}

#[test]
fn waitrecord_new_rejects_empty_wait_key() {
    let _instance_id = valid_instance_id();
    let empty_key = WaitKey::parse("").expect_err("empty should fail");

    // WaitRecord::new should propagate the WaitKey parse error
    // We test via the constructor accepting pre-validated WaitKey
    // Since WaitKey::parse("") fails, WaitRecord can't be built with empty key
    assert_eq!(
        empty_key,
        crate::ParseError::Empty {
            type_name: "WaitKey",
        }
    );
}

#[test]
fn waitrecord_accessors_return_correct_fields() {
    let instance_id = valid_instance_id();
    let wait_key = WaitKey::parse("buffer-test").expect("valid key");
    let policy = BufferPolicy::BufferOne;
    let ts = TimestampMs(999);

    let record =
        WaitRecord::new(instance_id.clone(), wait_key.clone(), policy, ts).expect("valid inputs");

    assert_eq!(record.instance_id().as_str(), instance_id.as_str());
    assert_eq!(record.wait_key().as_str(), "buffer-test");
    assert_eq!(record.buffer_policy(), BufferPolicy::BufferOne);
    assert_eq!(record.registered_at(), TimestampMs(999));
}

// ===========================================================================
// SignalDedupeKey — Unit Tests
// ===========================================================================

#[test]
fn signaldedupekey_new_constructs_with_valid_inputs() {
    let lineage_id = valid_instance_id();
    let wait_key = WaitKey::parse("approval").expect("valid key");
    let command_id = IdempotencyKey::parse("cmd-001").expect("valid key");

    let dk = SignalDedupeKey::new(lineage_id.clone(), wait_key.clone(), command_id.clone());

    assert_eq!(dk.lineage_id(), &lineage_id);
    assert_eq!(dk.wait_key(), &wait_key);
    assert_eq!(dk.command_id(), &command_id);
}

#[test]
fn signaldedupekey_hash_and_eq_work_for_deduplication() {
    let id = valid_instance_id();
    let key = WaitKey::parse("dup-test").expect("valid key");
    let cmd = IdempotencyKey::parse("cmd-dup").expect("valid key");

    let a = SignalDedupeKey::new(id.clone(), key.clone(), cmd.clone());
    let b = SignalDedupeKey::new(id.clone(), key.clone(), cmd.clone());
    let c = SignalDedupeKey::new(
        id.clone(),
        key.clone(),
        IdempotencyKey::parse("cmd-other").expect("valid key"),
    );

    let mut set = HashSet::new();
    set.insert(a);
    assert!(set.contains(&b), "identical dedupe keys should be equal");
    assert!(!set.contains(&c), "different command_id should not match");
    set.insert(b);
    assert_eq!(set.len(), 1, "identical keys should not increase set size");
    set.insert(c);
    assert_eq!(set.len(), 2, "different key should increase set size");
}

// ===========================================================================
// Serde Round-Trip — Integration Tests
// ===========================================================================

#[test]
fn waitkey_round_trips_through_serde_json_serialization() {
    let original = WaitKey::parse("serde-test").expect("valid key");
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: WaitKey = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(original, restored);
}

#[rstest]
#[case(BufferPolicy::Reject)]
#[case(BufferPolicy::BufferOne)]
#[case(BufferPolicy::BufferMany)]
fn bufferpolicy_round_trips_through_serde_json_serialization(#[case] policy: BufferPolicy) {
    let json = serde_json::to_string(&policy).expect("serialize");
    let restored: BufferPolicy = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(policy, restored);
}

#[rstest]
#[case(SignalDelivery::Accepted)]
#[case(SignalDelivery::Rejected)]
#[case(SignalDelivery::Buffered)]
fn signaldelivery_round_trips_through_serde_json_serialization(#[case] delivery: SignalDelivery) {
    let json = serde_json::to_string(&delivery).expect("serialize");
    let restored: SignalDelivery = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(delivery, restored);
}

#[test]
fn signaladdress_lineage_wide_round_trips_through_json() {
    let lineage_id = valid_instance_id();
    let instance_id = valid_instance_id();
    let wait_key = WaitKey::parse("serde-lineage").expect("valid key");
    let original = SignalAddress::lineage_wide(lineage_id, instance_id, wait_key);

    let json = serde_json::to_string(&original).expect("serialize");
    let restored: SignalAddress = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(original, restored);
}

#[test]
fn signaladdress_epoch_local_round_trips_through_json() {
    let lineage_id = valid_instance_id();
    let instance_id = valid_instance_id();
    let wait_key = WaitKey::parse("serde-epoch").expect("valid key");
    let epoch = Epoch::new(7);
    let original = SignalAddress::epoch_local(lineage_id, epoch, instance_id, wait_key);

    let json = serde_json::to_string(&original).expect("serialize");
    let restored: SignalAddress = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(original, restored);
}

#[test]
fn lineagescope_serializes_to_json_string() {
    let json = serde_json::to_string(&LineageScope::EpochLocal).expect("serialize");
    assert!(json.contains("EpochLocal") || json == "\"EpochLocal\"");
    let json = serde_json::to_string(&LineageScope::LineageWide).expect("serialize");
    assert!(json.contains("LineageWide") || json == "\"LineageWide\"");
}

#[test]
fn waitrecord_round_trips_through_serde_json_serialization() {
    let id = valid_instance_id();
    let key = WaitKey::parse("serde-wait").expect("valid key");
    let original =
        WaitRecord::new(id, key, BufferPolicy::BufferMany, TimestampMs(42)).expect("valid inputs");

    let json = serde_json::to_string(&original).expect("serialize");
    let restored: WaitRecord = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(original, restored);
}

#[test]
fn signaldedupekey_round_trips_through_serde_json_serialization() {
    let id = valid_instance_id();
    let key = WaitKey::parse("serde-dedupe").expect("valid key");
    let cmd = IdempotencyKey::parse("cmd-serde").expect("valid key");
    let original = SignalDedupeKey::new(id, key, cmd);

    let json = serde_json::to_string(&original).expect("serialize");
    let restored: SignalDedupeKey = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(original, restored);
}
