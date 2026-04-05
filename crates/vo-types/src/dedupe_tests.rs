use crate::ParseError;
use crate::*;

// ========== DedupeKey ==========

#[test]
fn dedupe_key_constructs_successfully_when_input_is_valid() {
    let key = DedupeKey::parse("provider-event-12345").expect("valid dedupe key");
    assert_eq!(key.as_str(), "provider-event-12345");
}

#[test]
fn dedupe_key_constructs_successfully_when_input_is_single_char() {
    let key = DedupeKey::parse("x").expect("valid single-char key");
    assert_eq!(key.as_str(), "x");
}

#[test]
fn dedupe_key_constructs_successfully_when_input_contains_unicode() {
    let key = DedupeKey::parse("event-日本語-key").expect("valid unicode key");
    assert_eq!(key.as_str(), "event-日本語-key");
}

#[test]
fn dedupe_key_accepts_exactly_256_chars_when_at_boundary() {
    let input = "a".repeat(256);
    let key = DedupeKey::parse(&input).expect("valid 256-char key");
    assert_eq!(key.as_str(), input);
}

#[test]
fn dedupe_key_rejects_empty_input_with_empty_error() {
    assert_eq!(
        DedupeKey::parse(""),
        Err(ParseError::Empty {
            type_name: "DedupeKey"
        })
    );
}

#[test]
fn dedupe_key_rejects_input_exceeding_256_chars() {
    let input = "a".repeat(257);
    assert_eq!(
        DedupeKey::parse(&input),
        Err(ParseError::ExceedsMaxLength {
            type_name: "DedupeKey",
            max: 256,
            actual: 257
        })
    );
}

#[test]
fn dedupe_key_display_shows_inner_string() {
    let key = DedupeKey::parse("event-abc").expect("valid key");
    assert_eq!(format!("{key}"), "event-abc");
}

#[test]
fn dedupe_key_try_from_string_roundtrips() {
    let original = "my-dedupe-key";
    let key = DedupeKey::try_from(original.to_string()).expect("valid key");
    assert_eq!(key.as_str(), original);
}

#[test]
fn dedupe_key_from_into_string_roundtrips() {
    let original = "my-dedupe-key";
    let key = DedupeKey::parse(original).expect("valid key");
    let back: String = key.into();
    assert_eq!(back, original);
}

#[test]
fn dedupe_key_serde_roundtrips() {
    let key = DedupeKey::parse("serde-test-key").expect("valid key");
    let json = serde_json::to_string(&key).expect("serialize");
    let recovered: DedupeKey = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(key, recovered);
}

#[test]
fn dedupe_key_serde_rejects_empty_string() {
    let json = "\"\"";
    let result: Result<DedupeKey, _> = serde_json::from_str(json);
    assert!(matches!(result, Err(_)));
}

#[test]
fn dedupe_key_equality_compares_inner_values() {
    let a = DedupeKey::parse("same-key").expect("valid");
    let b = DedupeKey::parse("same-key").expect("valid");
    let c = DedupeKey::parse("other-key").expect("valid");
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn dedupe_key_hash_is_consistent() {
    use std::collections::HashSet;
    let a = DedupeKey::parse("hash-key").expect("valid");
    let b = DedupeKey::parse("hash-key").expect("valid");
    let mut set = HashSet::new();
    set.insert(a);
    assert!(set.contains(&b));
}

// ========== DedupePartitionKey ==========

fn valid_instance_id() -> InstanceId {
    InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID")
}

#[test]
fn dedupe_partition_key_constructs_when_valid_inputs() {
    let id = valid_instance_id();
    let pk = DedupePartitionKey::new(id.clone(), "workflow_start").expect("valid partition key");
    assert_eq!(pk.instance_id(), &id);
    assert_eq!(pk.command_type(), "workflow_start");
}

#[test]
fn dedupe_partition_key_constructs_when_command_type_is_single_char() {
    let id = valid_instance_id();
    let pk = DedupePartitionKey::new(id, "x").expect("valid");
    assert_eq!(pk.command_type(), "x");
}

#[test]
fn dedupe_partition_key_rejects_empty_command_type() {
    let id = valid_instance_id();
    assert_eq!(
        DedupePartitionKey::new(id, ""),
        Err(ParseError::Empty {
            type_name: "DedupePartitionKey"
        })
    );
}

#[test]
fn dedupe_partition_key_rejects_command_type_exceeding_256_chars() {
    let id = valid_instance_id();
    let long_ct = "a".repeat(257);
    assert_eq!(
        DedupePartitionKey::new(id, &long_ct),
        Err(ParseError::ExceedsMaxLength {
            type_name: "DedupePartitionKey",
            max: 256,
            actual: 257
        })
    );
}

#[test]
fn dedupe_partition_key_accepts_exactly_256_char_command_type() {
    let id = valid_instance_id();
    let ct = "b".repeat(256);
    let pk = DedupePartitionKey::new(id, &ct).expect("valid");
    assert_eq!(pk.command_type(), ct);
}

#[test]
fn dedupe_partition_key_equality_compares_both_components() {
    let id = valid_instance_id();
    let a = DedupePartitionKey::new(id.clone(), "start").expect("valid");
    let b = DedupePartitionKey::new(id.clone(), "start").expect("valid");
    let c = DedupePartitionKey::new(id, "signal").expect("valid");
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn dedupe_partition_key_different_instance_ids_are_not_equal() {
    let id1 = valid_instance_id();
    // Generate a different ULID by changing one char
    let id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").expect("valid ULID");
    let a = DedupePartitionKey::new(id1, "start").expect("valid");
    let b = DedupePartitionKey::new(id2, "start").expect("valid");
    assert_ne!(a, b);
}

#[test]
fn dedupe_partition_key_serde_roundtrips() {
    let id = valid_instance_id();
    let pk = DedupePartitionKey::new(id, "callback").expect("valid");
    let json = serde_json::to_string(&pk).expect("serialize");
    let recovered: DedupePartitionKey = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(pk, recovered);
}

#[test]
fn dedupe_partition_key_serde_preserves_field_values() {
    let id = valid_instance_id();
    let pk = DedupePartitionKey::new(id.clone(), "signal").expect("valid");
    let json = serde_json::to_value(&pk).expect("serialize");
    assert_eq!(json["instance_id"], id.as_str());
    assert_eq!(json["command_type"], "signal");
}

#[test]
fn dedupe_partition_key_command_type_contains_unicode() {
    let id = valid_instance_id();
    let pk = DedupePartitionKey::new(id, "承認-approval").expect("valid");
    assert_eq!(pk.command_type(), "承認-approval");
}
