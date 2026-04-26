use super::*;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn sample_instance_id() -> InstanceId {
    parse_instance_id("01ARZ3NDEKTSV4RRFFQ69G5FAV")
}

fn alternate_instance_id() -> InstanceId {
    parse_instance_id("01H5JYV4XHGSR2F8KZ9BWNRFMA")
}

fn sample_step_id() -> StepId {
    parse_step_id("step-1")
}

fn alternate_step_id() -> StepId {
    parse_step_id("step_a-1")
}

fn parse_instance_id(raw: &str) -> InstanceId {
    InstanceId::parse(raw).unwrap()
}

fn parse_step_id(raw: &str) -> StepId {
    StepId::parse(raw).unwrap()
}

// ---------------------------------------------------------------------------
// Tests: Lease key encoding/decoding
// ---------------------------------------------------------------------------

#[test]
fn encode_lease_key_returns_length_prefixed_bytes_for_valid_ids() {
    let result = encode_lease_key(&sample_instance_id(), &sample_step_id());

    let expected_bytes = sample_instance_id().to_bytes().unwrap();
    let step_id = sample_step_id();
    let step_bytes = step_id.as_str().as_bytes();
    let mut expected = Vec::new();
    expected.extend_from_slice(&expected_bytes);
    expected.extend(&(step_bytes.len() as u16).to_be_bytes());
    expected.extend_from_slice(step_bytes);
    assert_eq!(result, expected);
}

#[test]
fn encode_lease_key_preserves_hyphen_and_underscore_in_step_id() {
    let result = encode_lease_key(&sample_instance_id(), &alternate_step_id());

    let expected_bytes = sample_instance_id().to_bytes().unwrap();
    let step_id = alternate_step_id();
    let step_bytes = step_id.as_str().as_bytes();
    let mut expected = Vec::new();
    expected.extend_from_slice(&expected_bytes);
    expected.extend(&(step_bytes.len() as u16).to_be_bytes());
    expected.extend_from_slice(step_bytes);
    assert_eq!(result, expected);
}

#[test]
fn encode_lease_key_returns_correct_byte_length_for_known_ids() {
    let result = encode_lease_key(&sample_instance_id(), &sample_step_id());

    assert_eq!(result.len(), 16 + 2 + 6);
}

#[test]
fn decode_lease_key_returns_original_ids_when_input_was_encoded() {
    let encoded = encode_lease_key(&sample_instance_id(), &sample_step_id());

    assert_eq!(
        decode_lease_key(&encoded),
        Ok((sample_instance_id(), sample_step_id()))
    );
}

#[test]
fn decode_lease_key_returns_ids_when_given_encoded_key_bytes() {
    let encoded = encode_lease_key(&sample_instance_id(), &sample_step_id());
    assert_eq!(decode_lease_key(&encoded), Ok((sample_instance_id(), sample_step_id())));
}

fn invalid_instance_reason(raw: &str) -> String {
    match InstanceId::parse(raw) {
        Ok(instance_id) => format!("unexpected valid instance id: {instance_id}"),
        Err(error) => format!("invalid instance_id: {error}"),
    }
}

fn invalid_step_reason(raw: &str) -> String {
    match StepId::parse(raw) {
        Ok(step_id) => format!("unexpected valid step id: {step_id}"),
        Err(error) => format!("invalid step_id: {error}"),
    }
}

#[test]
fn decode_lease_key_rejects_empty_input() {
    assert!(
        matches!(decode_lease_key(b""), Err(LeaseStoreError::Codec { .. })),
        "empty input should be rejected"
    );
}

#[test]
fn decode_lease_key_rejects_too_short_input() {
    assert!(
        matches!(decode_lease_key(&[0u8; 17]), Err(LeaseStoreError::Codec { .. })),
        "input shorter than 18 bytes should be rejected"
    );
}

#[test]
fn decode_lease_key_accepts_binary_instance_id_bytes() {
    let input = [0u8; 17];
    assert!(
        matches!(decode_lease_key(&input), Err(LeaseStoreError::Codec { .. })),
        "binary data shorter than required should be rejected"
    );
}

#[test]
fn decode_lease_key_returns_codec_error_when_instance_segment_empty() {
    assert_eq!(
        decode_lease_key(b"::step-1"),
        Err(LeaseStoreError::Codec {
            reason: format!(
                "invalid instance_id: {}",
                invalid_instance_reason("").trim_start_matches("invalid instance_id: ")
            ),
        })
    );
}

#[test]
fn decode_lease_key_returns_codec_error_when_instance_segment_has_invalid_length() {
    assert_eq!(
        decode_lease_key(b"short::step-1"),
        Err(LeaseStoreError::Codec {
            reason: format!(
                "invalid instance_id: {}",
                invalid_instance_reason("short").trim_start_matches("invalid instance_id: ")
            ),
        })
    );
}

#[test]
fn decode_lease_key_returns_codec_error_when_instance_segment_is_26_char_nil_ulid() {
    assert_eq!(
        decode_lease_key(b"00000000000000000000000000::step-1"),
        Err(LeaseStoreError::Codec {
            reason: format!(
                "invalid instance_id: {}",
                invalid_instance_reason("00000000000000000000000000")
                    .trim_start_matches("invalid instance_id: ")
            ),
        })
    );
}

#[test]
fn decode_lease_key_returns_codec_error_when_step_segment_has_invalid_character() {
    assert_eq!(
        decode_lease_key(b"01ARZ3NDEKTSV4RRFFQ69G5FAV::step:1"),
        Err(LeaseStoreError::Codec {
            reason: invalid_step_reason("step:1"),
        })
    );
}

#[test]
fn decode_lease_key_returns_codec_error_when_key_contains_multiple_delimiters() {
    assert_eq!(
        decode_lease_key(b"01ARZ3NDEKTSV4RRFFQ69G5FAV::step-1::suffix"),
        Err(LeaseStoreError::Codec {
            reason: format!(
                "invalid step_id: {}",
                invalid_step_reason("step-1::suffix").trim_start_matches("invalid step_id: ")
            ),
        })
    );
}

#[test]
fn decode_lease_key_returns_codec_error_when_step_segment_empty() {
    assert_eq!(
        decode_lease_key(b"01ARZ3NDEKTSV4RRFFQ69G5FAV::"),
        Err(LeaseStoreError::Codec {
            reason: invalid_step_reason(""),
        })
    );
}

// ---------------------------------------------------------------------------
// Tests: Lease entry encoding/decoding
// ---------------------------------------------------------------------------

fn sample_entry() -> LeaseEntry {
    LeaseEntry::new("iid".to_string(), "sid".to_string(), 7, 5_000).unwrap()
}

fn lease_entry(instance_id: &str, step_id: &str, fence_token: u64, expires_at: u64) -> LeaseEntry {
    LeaseEntry::new(
        instance_id.to_string(),
        step_id.to_string(),
        fence_token,
        expires_at,
    )
    .unwrap()
}

fn decode_entry_error_reason(input: &[u8]) -> String {
    match serde_json::from_slice::<LeaseEntry>(input) {
        Ok(entry) => format!("unexpected decode success: {entry:?}"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn encode_lease_entry_returns_exact_json_bytes_for_known_entry() {
    let result = encode_lease_entry(&sample_entry());

    assert_eq!(
        result,
        Ok(br#"{"instance_id":"iid","step_id":"sid","fence_token":7,"expires_at":5000}"#.to_vec())
    );
}

#[test]
fn encode_lease_entry_returns_exact_json_bytes_when_expires_at_zero() {
    let entry = lease_entry("iid", "sid", 7, 0);

    assert_eq!(
        encode_lease_entry(&entry),
        Ok(br#"{"instance_id":"iid","step_id":"sid","fence_token":7,"expires_at":0}"#.to_vec())
    );
}

#[test]
fn encode_lease_entry_returns_exact_json_bytes_for_u64_max_fields() {
    let entry = lease_entry("iid", "sid", u64::MAX, u64::MAX);

    assert_eq!(
        encode_lease_entry(&entry),
        Ok(format!(
            "{{\"instance_id\":\"iid\",\"step_id\":\"sid\",\"fence_token\":{},\"expires_at\":{}}}",
            u64::MAX,
            u64::MAX
        )
        .into_bytes())
    );
}

#[test]
fn encode_lease_entry_round_trips_with_decode_for_valid_entry() {
    let entry = sample_entry();

    assert_eq!(
        encode_lease_entry(&entry).and_then(|encoded| decode_lease_entry(&encoded)),
        Ok(entry)
    );
}

#[test]
fn decode_lease_entry_returns_entry_matching_literal_json() {
    assert_eq!(
        decode_lease_entry(
            br#"{"instance_id":"iid","step_id":"sid","fence_token":7,"expires_at":5000}"#
        ),
        Ok(sample_entry())
    );
}

#[test]
fn decode_lease_entry_returns_entry_with_u64_max_fields_from_json() {
    let input = format!(
        "{{\"instance_id\":\"iid\",\"step_id\":\"sid\",\"fence_token\":{},\"expires_at\":{}}}",
        u64::MAX,
        u64::MAX
    );

    assert_eq!(
        decode_lease_entry(input.as_bytes()),
        Ok(lease_entry("iid", "sid", u64::MAX, u64::MAX))
    );
}

#[test]
fn decode_lease_entry_preserves_semantically_invalid_but_shape_valid_payload() {
    let result = decode_lease_entry(
        br#"{"instance_id":"short","step_id":"_bad","fence_token":0,"expires_at":9}"#,
    );

    assert_eq!(
        result,
        Ok(LeaseEntry {
            instance_id: "short".to_string(),
            step_id: "_bad".to_string(),
            fence_token: 0,
            expires_at: 9,
        })
    );
}

#[test]
fn decode_lease_entry_returns_codec_error_when_input_empty() {
    let input = b"";

    assert_eq!(
        decode_lease_entry(input),
        Err(LeaseStoreError::Codec {
            reason: decode_entry_error_reason(input),
        })
    );
}

#[test]
fn decode_lease_entry_returns_codec_error_when_json_is_malformed() {
    let input = br#"{"instance_id":"iid""#;

    assert_eq!(
        decode_lease_entry(input),
        Err(LeaseStoreError::Codec {
            reason: decode_entry_error_reason(input),
        })
    );
}

#[test]
fn decode_lease_entry_returns_codec_error_when_expires_at_missing() {
    let input = br#"{"instance_id":"iid","step_id":"sid","fence_token":7}"#;

    assert_eq!(
        decode_lease_entry(input),
        Err(LeaseStoreError::Codec {
            reason: decode_entry_error_reason(input),
        })
    );
}

#[test]
fn decode_lease_entry_returns_codec_error_when_instance_id_has_wrong_type() {
    let input = br#"{"instance_id":1,"step_id":"sid","fence_token":7,"expires_at":5}"#;

    assert_eq!(
        decode_lease_entry(input),
        Err(LeaseStoreError::Codec {
            reason: decode_entry_error_reason(input),
        })
    );
}

#[test]
fn decode_lease_entry_returns_codec_error_when_fence_token_has_wrong_type() {
    let input = br#"{"instance_id":"iid","step_id":"sid","fence_token":"x","expires_at":5}"#;

    assert_eq!(
        decode_lease_entry(input),
        Err(LeaseStoreError::Codec {
            reason: decode_entry_error_reason(input),
        })
    );
}

#[test]
fn decode_lease_entry_returns_codec_error_when_expires_at_has_wrong_type() {
    let input = br#"{"instance_id":"iid","step_id":"sid","fence_token":7,"expires_at":"x"}"#;

    assert_eq!(
        decode_lease_entry(input),
        Err(LeaseStoreError::Codec {
            reason: decode_entry_error_reason(input),
        })
    );
}

#[test]
fn decode_lease_entry_returns_codec_error_when_json_is_array() {
    let input = br"[]";

    assert_eq!(
        decode_lease_entry(input),
        Err(LeaseStoreError::Codec {
            reason: decode_entry_error_reason(input),
        })
    );
}

// ---------------------------------------------------------------------------
// Tests: LeaseStoreError Display
// ---------------------------------------------------------------------------

#[test]
fn lease_store_error_display_equals_exact_message_for_lease_already_held() {
    let error = LeaseStoreError::LeaseAlreadyHeld {
        instance_id: "iid".to_string(),
        step_id: "sid".to_string(),
    };

    assert_eq!(error.to_string(), "lease already held for iid::sid");
}

#[test]
fn lease_store_error_display_equals_exact_message_for_not_found() {
    let error = LeaseStoreError::NotFound {
        instance_id: "iid".to_string(),
        step_id: "sid".to_string(),
    };

    assert_eq!(error.to_string(), "lease not found for iid::sid");
}

#[test]
fn lease_store_error_display_equals_exact_message_for_stale_fence() {
    let error = LeaseStoreError::StaleFence {
        expected: "7".to_string(),
        actual: "9".to_string(),
    };

    assert_eq!(error.to_string(), "stale fence: expected 7, got 9");
}

#[test]
fn lease_store_error_display_equals_exact_message_for_fence_token_exhausted() {
    let error = LeaseStoreError::FenceTokenExhausted {
        instance_id: "iid".to_string(),
        step_id: "sid".to_string(),
    };

    assert_eq!(error.to_string(), "fence token exhausted for iid::sid");
}

#[test]
fn lease_store_error_display_equals_exact_message_for_storage() {
    let error = LeaseStoreError::Storage {
        reason: "disk full".to_string(),
    };

    assert_eq!(error.to_string(), "lease storage error: disk full");
}

#[test]
fn lease_store_error_display_equals_exact_message_for_codec() {
    let error = LeaseStoreError::Codec {
        reason: "bad json".to_string(),
    };

    assert_eq!(error.to_string(), "lease codec error: bad json");
}

#[test]
fn lease_store_error_display_equals_exact_message_for_invalid_argument() {
    assert_eq!(
        LeaseStoreError::InvalidArgument.to_string(),
        "invalid lease argument"
    );
}

// ---------------------------------------------------------------------------
// Tests: Partition constant and cross-pair IDs
// ---------------------------------------------------------------------------

#[test]
fn lease_partition_equals_leases() {
    assert_eq!(LEASE_PARTITION, "leases");
}

#[test]
fn alternate_ids_are_parsed_for_cross_pair_tests() {
    assert_eq!(
        (
            alternate_instance_id().to_string(),
            alternate_step_id().to_string()
        ),
        (
            "01H5JYV4XHGSR2F8KZ9BWNRFMA".to_string(),
            "step_a-1".to_string(),
        )
    );
}

// ---------------------------------------------------------------------------
// BDD: ADR-020 / ADR-029 — Encode lease keys without delimiter ambiguity
// ---------------------------------------------------------------------------

/// Given instance_id or step_id contains delimiter-like bytes
/// When lease key is encoded
/// Then the key round-trips without collision and no raw :: separator is used
#[test]
fn given_lease_key_components_with_delimiters_when_encoded_then_no_collision_occurs() {
    // Given: multiple (instance_id, step_id) pairs with hyphenated "delimiter-like" content
    let pairs = [
        (
            parse_instance_id("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            parse_step_id("step-1"),
        ),
        (
            parse_instance_id("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            parse_step_id("step-2"),
        ),
        (
            parse_instance_id("01H5JYV4XHGSR2F8KZ9BWNRFMA"),
            parse_step_id("step-1"),
        ),
        (
            parse_instance_id("01H5JYV4XHGSR2F8KZ9BWNRFMA"),
            parse_step_id("step-2"),
        ),
        (
            parse_instance_id("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            parse_step_id("step_a-1"),
        ),
        (
            parse_instance_id("7ZZZZZZZZZZZZZZZZZZZZZZZZZ"),
            parse_step_id("step---a__b"),
        ),
    ];

    // When: encode all keys
    let encoded_keys: Vec<Vec<u8>> = pairs
        .iter()
        .map(|(iid, sid)| encode_lease_key(iid, sid))
        .collect();

    // Then: no two keys are the same (no collisions)
    for i in 0..encoded_keys.len() {
        for j in (i + 1)..encoded_keys.len() {
            assert_ne!(
                encoded_keys[i],
                encoded_keys[j],
                "collision between pair[{}] and pair[{}]",
                i,
                j
            );
        }
    }

    // Then: every key round-trips correctly
    for (idx, ((expected_iid, expected_sid), encoded)) in pairs.iter().zip(encoded_keys.iter()).enumerate() {
        let (decoded_iid, decoded_sid) = decode_lease_key(encoded).unwrap();
        assert_eq!(
            decoded_iid, *expected_iid,
            "round-trip instance_id mismatch at index {idx}"
        );
        assert_eq!(
            decoded_sid, *expected_sid,
            "round-trip step_id mismatch at index {idx}"
        );
    }

    // Then: no encoded key contains the raw `::` separator bytes
    for (idx, encoded) in encoded_keys.iter().enumerate() {
        assert!(
            !encoded.windows(2).any(|w| w == b"::"),
            "encoded key at index {idx} contains raw :: separator bytes"
        );
    }
}
