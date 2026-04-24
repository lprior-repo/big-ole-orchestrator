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
// Tests: LeaseEntry construction and fields
// ---------------------------------------------------------------------------

#[test]
fn lease_entry_returns_entry_when_fields_valid() {
    let result = LeaseEntry::new("iid".to_string(), "sid".to_string(), 7, 5_000);

    assert_eq!(
        result,
        Ok(LeaseEntry {
            instance_id: "iid".to_string(),
            step_id: "sid".to_string(),
            fence_token: 7,
            expires_at: 5_000,
        })
    );
}

#[test]
fn lease_entry_returns_invalid_argument_when_instance_id_empty() {
    let result = LeaseEntry::new(String::new(), "sid".to_string(), 1, 5);

    assert_eq!(result, Err(LeaseStoreError::InvalidArgument));
}

#[test]
fn lease_entry_returns_invalid_argument_when_step_id_empty() {
    let result = LeaseEntry::new("iid".to_string(), String::new(), 1, 5);

    assert_eq!(result, Err(LeaseStoreError::InvalidArgument));
}

#[test]
fn lease_entry_returns_invalid_argument_when_fence_token_zero() {
    let result = LeaseEntry::new("iid".to_string(), "sid".to_string(), 0, 5);

    assert_eq!(result, Err(LeaseStoreError::InvalidArgument));
}

#[test]
fn lease_entry_returns_entry_when_expires_at_zero() {
    let result = LeaseEntry::new("iid".to_string(), "sid".to_string(), 1, 0);

    assert_eq!(result.map(|entry| entry.expires_at()), Ok(0));
}

#[test]
fn lease_entry_returns_entry_when_fence_token_is_u64_max() {
    let result = LeaseEntry::new("iid".to_string(), "sid".to_string(), u64::MAX, 1);

    assert_eq!(result.map(|entry| entry.fence_token()), Ok(u64::MAX));
}

#[test]
fn lease_entry_returns_entry_when_expires_at_is_u64_max() {
    let result = LeaseEntry::new("iid".to_string(), "sid".to_string(), 1, u64::MAX);

    assert_eq!(result.map(|entry| entry.expires_at()), Ok(u64::MAX));
}

#[test]
fn lease_entry_returns_entry_when_both_u64_fields_are_u64_max() {
    let result = LeaseEntry::new("iid".to_string(), "sid".to_string(), u64::MAX, u64::MAX);

    assert_eq!(
        result,
        Ok(LeaseEntry {
            instance_id: "iid".to_string(),
            step_id: "sid".to_string(),
            fence_token: u64::MAX,
            expires_at: u64::MAX,
        })
    );
}

#[test]
fn lease_entry_instance_id_returns_original_string_when_entry_constructed() {
    let entry = LeaseEntry::new("iid".to_string(), "sid".to_string(), 9, 17).unwrap();

    assert_eq!(entry.instance_id(), "iid");
}

#[test]
fn lease_entry_step_id_returns_original_string_when_entry_constructed() {
    let entry = LeaseEntry::new("iid".to_string(), "sid".to_string(), 9, 17).unwrap();

    assert_eq!(entry.step_id(), "sid");
}

#[test]
fn lease_entry_fence_token_returns_original_u64_when_entry_constructed() {
    let entry = LeaseEntry::new("iid".to_string(), "sid".to_string(), 9, 17).unwrap();

    assert_eq!(entry.fence_token(), 9);
}

#[test]
fn lease_entry_expires_at_returns_original_u64_when_entry_constructed() {
    let entry = LeaseEntry::new("iid".to_string(), "sid".to_string(), 9, 17).unwrap();

    assert_eq!(entry.expires_at(), 17);
}

// ---------------------------------------------------------------------------
// Tests: LeaseEntry is_expired
// ---------------------------------------------------------------------------

#[test]
fn lease_entry_is_not_expired_when_now_zero_and_expiry_one() {
    let entry = LeaseEntry::new("iid".to_string(), "sid".to_string(), 1, 1).unwrap();

    assert!(!entry.is_expired(0));
}

#[test]
fn lease_entry_is_expired_when_now_zero_and_expiry_zero() {
    let entry = LeaseEntry::new("iid".to_string(), "sid".to_string(), 1, 0).unwrap();

    assert!(entry.is_expired(0));
}

#[test]
fn lease_entry_is_not_expired_when_now_is_one_less_than_expiry() {
    let entry = LeaseEntry::new("iid".to_string(), "sid".to_string(), 1, 1_000).unwrap();

    assert!(!entry.is_expired(999));
}

#[test]
fn lease_entry_is_expired_when_now_equals_expiry() {
    let entry = LeaseEntry::new("iid".to_string(), "sid".to_string(), 1, 1_000).unwrap();

    assert!(entry.is_expired(1_000));
}

#[test]
fn lease_entry_is_expired_when_now_greater_than_expiry() {
    let entry = LeaseEntry::new("iid".to_string(), "sid".to_string(), 1, 1_000).unwrap();

    assert!(entry.is_expired(1_001));
}

#[test]
fn lease_entry_is_not_expired_when_expiry_is_u64_max_and_now_is_u64_max_minus_one() {
    let entry = LeaseEntry::new("iid".to_string(), "sid".to_string(), 1, u64::MAX).unwrap();

    assert!(!entry.is_expired(u64::MAX - 1));
}

#[test]
fn lease_entry_is_expired_when_expiry_is_u64_max_and_now_is_u64_max() {
    let entry = LeaseEntry::new("iid".to_string(), "sid".to_string(), 1, u64::MAX).unwrap();

    assert!(entry.is_expired(u64::MAX));
}

#[test]
fn lease_entry_remains_expired_when_checked_again_after_boundary() {
    let entry = LeaseEntry::new("iid".to_string(), "sid".to_string(), 1, 1_000).unwrap();

    assert_eq!(
        (entry.is_expired(1_000), entry.is_expired(2_000)),
        (true, true)
    );
}

// ---------------------------------------------------------------------------
// Tests: LeaseEntry to_lease_record
// ---------------------------------------------------------------------------

#[test]
fn lease_entry_to_lease_record_returns_typed_record_when_fields_valid() {
    let entry = LeaseEntry::new(
        sample_instance_id().to_string(),
        sample_step_id().to_string(),
        7,
        5_000,
    )
    .unwrap();

    assert_eq!(
        entry.to_lease_record().map(|record| (
            record.instance_id().to_string(),
            record.step_id().to_string(),
            record.token().inner().get(),
        )),
        Ok((
            sample_instance_id().to_string(),
            sample_step_id().to_string(),
            7,
        ))
    );
}

#[test]
fn lease_entry_to_lease_record_preserves_u64_max_token_when_ids_valid() {
    let entry = LeaseEntry::new(
        sample_instance_id().to_string(),
        sample_step_id().to_string(),
        u64::MAX,
        5_000,
    )
    .unwrap();

    assert_eq!(
        entry
            .to_lease_record()
            .map(|record| record.token().inner().get()),
        Ok(u64::MAX)
    );
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
fn lease_entry_to_lease_record_returns_codec_error_when_fence_token_zero() {
    let entry = LeaseEntry {
        instance_id: sample_instance_id().to_string(),
        step_id: sample_step_id().to_string(),
        fence_token: 0,
        expires_at: 9,
    };

    assert_eq!(
        entry.to_lease_record(),
        Err(LeaseStoreError::Codec {
            reason: "invalid fence token value".to_string(),
        })
    );
}

#[test]
fn lease_entry_to_lease_record_returns_codec_error_when_instance_id_empty() {
    let entry = LeaseEntry {
        instance_id: String::new(),
        step_id: sample_step_id().to_string(),
        fence_token: 1,
        expires_at: 9,
    };

    assert_eq!(
        entry.to_lease_record(),
        Err(LeaseStoreError::Codec {
            reason: format!(
                "invalid instance_id: {}",
                invalid_instance_reason("").trim_start_matches("invalid instance_id: ")
            ),
        })
    );
}

#[test]
fn lease_entry_to_lease_record_returns_codec_error_when_instance_id_length_invalid() {
    let entry = LeaseEntry {
        instance_id: "short".to_string(),
        step_id: sample_step_id().to_string(),
        fence_token: 1,
        expires_at: 9,
    };

    assert_eq!(
        entry.to_lease_record(),
        Err(LeaseStoreError::Codec {
            reason: format!(
                "invalid instance_id: {}",
                invalid_instance_reason("short").trim_start_matches("invalid instance_id: ")
            ),
        })
    );
}

#[test]
fn lease_entry_to_lease_record_returns_codec_error_when_instance_id_is_nil_ulid() {
    let entry = LeaseEntry {
        instance_id: "00000000000000000000000000".to_string(),
        step_id: sample_step_id().to_string(),
        fence_token: 1,
        expires_at: 9,
    };

    assert_eq!(
        entry.to_lease_record(),
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
fn lease_entry_to_lease_record_returns_codec_error_when_step_id_empty() {
    let entry = LeaseEntry {
        instance_id: sample_instance_id().to_string(),
        step_id: String::new(),
        fence_token: 1,
        expires_at: 9,
    };

    assert_eq!(
        entry.to_lease_record(),
        Err(LeaseStoreError::Codec {
            reason: invalid_step_reason(""),
        })
    );
}

#[test]
fn lease_entry_to_lease_record_returns_codec_error_when_step_id_has_invalid_character() {
    let entry = LeaseEntry {
        instance_id: sample_instance_id().to_string(),
        step_id: "step:1".to_string(),
        fence_token: 1,
        expires_at: 9,
    };

    assert_eq!(
        entry.to_lease_record(),
        Err(LeaseStoreError::Codec {
            reason: invalid_step_reason("step:1"),
        })
    );
}

#[test]
fn lease_entry_to_lease_record_returns_codec_error_when_step_id_starts_with_underscore() {
    let entry = LeaseEntry {
        instance_id: sample_instance_id().to_string(),
        step_id: "_step".to_string(),
        fence_token: 1,
        expires_at: 9,
    };

    assert_eq!(
        entry.to_lease_record(),
        Err(LeaseStoreError::Codec {
            reason: invalid_step_reason("_step"),
        })
    );
}
