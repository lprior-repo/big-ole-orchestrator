use crate::*;

// --- Serde Serialize matches Display ---

#[test]
fn serde_serialize_instance_id_matches_display() {
    let id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid");
    let json = serde_json::to_string(&id).expect("serialize");
    assert_eq!(json, "\"01H5JYV4XHGSR2F8KZ9BWNRFMA\"");
}

#[test]
fn serde_serialize_workflow_name_matches_display() {
    let wn = WorkflowName::parse("deploy-prod").expect("valid");
    let json = serde_json::to_string(&wn).expect("serialize");
    assert_eq!(json, "\"deploy-prod\"");
}

#[test]
fn serde_serialize_node_name_matches_display() {
    let nn = NodeName::parse("compile-artifact").expect("valid");
    let json = serde_json::to_string(&nn).expect("serialize");
    assert_eq!(json, "\"compile-artifact\"");
}

#[test]
fn serde_serialize_binary_hash_matches_display() {
    let bh = BinaryHash::parse("abcdef0123456789").expect("valid");
    let json = serde_json::to_string(&bh).expect("serialize");
    assert_eq!(json, "\"abcdef0123456789\"");
}

#[test]
fn serde_serialize_timer_id_matches_display() {
    let ti = TimerId::parse("timer-123").expect("valid");
    let json = serde_json::to_string(&ti).expect("serialize");
    assert_eq!(json, "\"timer-123\"");
}

#[test]
fn serde_serialize_idempotency_key_matches_display() {
    let ik = IdempotencyKey::parse("key-abc").expect("valid");
    let json = serde_json::to_string(&ik).expect("serialize");
    assert_eq!(json, "\"key-abc\"");
}

#[test]
fn serde_serialize_sequence_number_matches_display() {
    let sn = SequenceNumber::new_unchecked(42);
    let json = serde_json::to_string(&sn).expect("serialize");
    assert_eq!(json, "42");
}

#[test]
fn serde_serialize_event_version_matches_display() {
    let ev = EventVersion::new_unchecked(1);
    let json = serde_json::to_string(&ev).expect("serialize");
    assert_eq!(json, "1");
}

#[test]
fn serde_serialize_attempt_number_matches_display() {
    let an = AttemptNumber::new_unchecked(3);
    let json = serde_json::to_string(&an).expect("serialize");
    assert_eq!(json, "3");
}

#[test]
fn serde_serialize_timeout_ms_matches_display() {
    let tm = TimeoutMs::new_unchecked(5000);
    let json = serde_json::to_string(&tm).expect("serialize");
    assert_eq!(json, "5000");
}

#[test]
fn serde_serialize_duration_ms_matches_display() {
    let dm = DurationMs(5000);
    let json = serde_json::to_string(&dm).expect("serialize");
    assert_eq!(json, "5000");
}

#[test]
fn serde_serialize_timestamp_ms_matches_display() {
    let ts = TimestampMs(1710000000000);
    let json = serde_json::to_string(&ts).expect("serialize");
    assert_eq!(json, "1710000000000");
}

#[test]
fn serde_serialize_fire_at_ms_matches_display() {
    let fa = FireAtMs(1710000000000);
    let json = serde_json::to_string(&fa).expect("serialize");
    assert_eq!(json, "1710000000000");
}

#[test]
fn serde_serialize_max_attempts_matches_display() {
    let ma = MaxAttempts::new_unchecked(3);
    let json = serde_json::to_string(&ma).expect("serialize");
    assert_eq!(json, "3");
}

// --- Serde Deserialize (valid) ---

#[test]
fn serde_deserialize_valid_instance_id() {
    let json = "\"01H5JYV4XHGSR2F8KZ9BWNRFMA\"";
    let result: Result<InstanceId, _> = serde_json::from_str(json);
    assert!(result.is_ok());
}

#[test]
fn serde_deserialize_valid_workflow_name() {
    let json = "\"deploy-prod\"";
    let result: Result<WorkflowName, _> = serde_json::from_str(json);
    assert!(result.is_ok());
}

#[test]
fn serde_deserialize_valid_node_name() {
    let json = "\"compile-artifact\"";
    let result: Result<NodeName, _> = serde_json::from_str(json);
    assert!(result.is_ok());
}

#[test]
fn serde_deserialize_valid_binary_hash() {
    let json = "\"abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789\"";
    let result: Result<BinaryHash, _> = serde_json::from_str(json);
    assert!(result.is_ok());
}

#[test]
fn serde_deserialize_valid_timer_id() {
    let json = "\"timer-123\"";
    let result: Result<TimerId, _> = serde_json::from_str(json);
    assert!(result.is_ok());
}

#[test]
fn serde_deserialize_valid_idempotency_key() {
    let json = "\"key-abc\"";
    let result: Result<IdempotencyKey, _> = serde_json::from_str(json);
    assert!(result.is_ok());
}

#[test]
fn serde_deserialize_valid_sequence_number() {
    let json = "42";
    let result: Result<SequenceNumber, _> = serde_json::from_str(json);
    assert!(result.is_ok());
}

#[test]
fn serde_deserialize_valid_event_version() {
    let json = "1";
    let result: Result<EventVersion, _> = serde_json::from_str(json);
    assert!(result.is_ok());
}

#[test]
fn serde_deserialize_valid_attempt_number() {
    let json = "3";
    let result: Result<AttemptNumber, _> = serde_json::from_str(json);
    assert!(result.is_ok());
}

#[test]
fn serde_deserialize_valid_timeout_ms() {
    let json = "5000";
    let result: Result<TimeoutMs, _> = serde_json::from_str(json);
    assert!(result.is_ok());
}

#[test]
fn serde_deserialize_valid_duration_ms() {
    let json = "1500";
    let result: Result<DurationMs, _> = serde_json::from_str(json);
    assert!(result.is_ok());
}

#[test]
fn serde_deserialize_valid_timestamp_ms() {
    let json = "1710000000000";
    let result: Result<TimestampMs, _> = serde_json::from_str(json);
    assert!(result.is_ok());
}

#[test]
fn serde_deserialize_valid_fire_at_ms() {
    let json = "1710000000000";
    let result: Result<FireAtMs, _> = serde_json::from_str(json);
    assert!(result.is_ok());
}

#[test]
fn serde_deserialize_valid_max_attempts() {
    let json = "3";
    let result: Result<MaxAttempts, _> = serde_json::from_str(json);
    assert!(result.is_ok());
}

// --- Serde Deserialize (rejection) ---

#[test]
fn serde_deserialize_rejects_empty_instance_id() {
    let json = "\"\"";
    let result: Result<InstanceId, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_deserialize_rejects_empty_workflow_name() {
    let json = "\"\"";
    let result: Result<WorkflowName, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_deserialize_rejects_empty_node_name() {
    let json = "\"\"";
    let result: Result<NodeName, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_deserialize_rejects_empty_binary_hash() {
    let json = "\"\"";
    let result: Result<BinaryHash, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_deserialize_rejects_empty_timer_id() {
    let json = "\"\"";
    let result: Result<TimerId, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_deserialize_rejects_empty_idempotency_key() {
    let json = "\"\"";
    let result: Result<IdempotencyKey, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_deserialize_rejects_zero_sequence_number() {
    let json = "0";
    let result: Result<SequenceNumber, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_deserialize_rejects_zero_event_version() {
    let json = "0";
    let result: Result<EventVersion, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_deserialize_rejects_zero_attempt_number() {
    let json = "0";
    let result: Result<AttemptNumber, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_deserialize_rejects_zero_timeout_ms() {
    let json = "0";
    let result: Result<TimeoutMs, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_deserialize_rejects_zero_max_attempts() {
    let json = "0";
    let result: Result<MaxAttempts, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

// --- Serde Round-trip ---

#[test]
fn serde_round_trip_instance_id() {
    let original = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid");
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: InstanceId = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn serde_round_trip_workflow_name() {
    let original = WorkflowName::parse("deploy-prod").expect("valid");
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: WorkflowName = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn serde_round_trip_node_name() {
    let original = NodeName::parse("compile-artifact").expect("valid");
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: NodeName = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn serde_round_trip_binary_hash() {
    let original =
        BinaryHash::parse("abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789")
            .expect("valid");
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: BinaryHash = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn serde_round_trip_timer_id() {
    let original = TimerId::parse("timer-123").expect("valid");
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: TimerId = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn serde_round_trip_idempotency_key() {
    let original = IdempotencyKey::parse("key-abc").expect("valid");
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: IdempotencyKey = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn serde_round_trip_sequence_number() {
    let original = SequenceNumber::new_unchecked(42);
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: SequenceNumber = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn serde_round_trip_event_version() {
    let original = EventVersion::new_unchecked(1);
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: EventVersion = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn serde_round_trip_attempt_number() {
    let original = AttemptNumber::new_unchecked(3);
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: AttemptNumber = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn serde_round_trip_timeout_ms() {
    let original = TimeoutMs::new_unchecked(5000);
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: TimeoutMs = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn serde_round_trip_duration_ms() {
    let original = DurationMs(5000);
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: DurationMs = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn serde_round_trip_timestamp_ms() {
    let original = TimestampMs(1710000000000);
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: TimestampMs = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn serde_round_trip_fire_at_ms() {
    let original = FireAtMs(1710000000000);
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: FireAtMs = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn serde_round_trip_max_attempts() {
    let original = MaxAttempts::new_unchecked(3);
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: MaxAttempts = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored, original);
}

// --- Serde: wrong JSON types ---

#[test]
fn serde_string_type_rejects_unquoted_number() {
    let json = "42";
    let result: Result<InstanceId, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "InstanceId should reject bare integer JSON"
    );
}

#[test]
fn serde_integer_type_rejects_string() {
    let json = "\"42\"";
    let result: Result<SequenceNumber, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "SequenceNumber should reject quoted string JSON"
    );
}

#[test]
fn serde_rejects_null_for_instance_id() {
    let json = "null";
    let result: Result<InstanceId, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_rejects_null_for_sequence_number() {
    let json = "null";
    let result: Result<SequenceNumber, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_rejects_null_for_duration_ms() {
    let json = "null";
    let result: Result<DurationMs, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_rejects_array_for_workflow_name() {
    let json = "[\"deploy-prod\"]";
    let result: Result<WorkflowName, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_rejects_array_for_sequence_number() {
    let json = "[42]";
    let result: Result<SequenceNumber, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_rejects_object_for_instance_id() {
    let json = "{\"value\": \"01H5JYV4XHGSR2F8KZ9BWNRFMA\"}";
    let result: Result<InstanceId, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_rejects_boolean_for_sequence_number() {
    let json = "true";
    let result: Result<SequenceNumber, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_rejects_negative_for_sequence_number() {
    let json = "-1";
    let result: Result<SequenceNumber, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_rejects_float_for_timeout_ms() {
    let json = "3.14";
    let result: Result<TimeoutMs, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_rejects_empty_string_for_sequence_number() {
    let json = "\"\"";
    let result: Result<SequenceNumber, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_rejects_string_for_event_version() {
    let json = "\"1\"";
    let result: Result<EventVersion, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_rejects_string_for_attempt_number() {
    let json = "\"3\"";
    let result: Result<AttemptNumber, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_rejects_string_for_max_attempts() {
    let json = "\"3\"";
    let result: Result<MaxAttempts, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_rejects_string_for_fire_at_ms() {
    let json = "\"1710000000000\"";
    let result: Result<FireAtMs, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_rejects_string_for_timestamp_ms() {
    let json = "\"1710000000000\"";
    let result: Result<TimestampMs, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_rejects_string_for_timeout_ms() {
    let json = "\"5000\"";
    let result: Result<TimeoutMs, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_rejects_string_for_duration_ms() {
    let json = "\"1500\"";
    let result: Result<DurationMs, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_json_u64_max_for_sequence_number() {
    let json = "18446744073709551615";
    let result: Result<SequenceNumber, _> = serde_json::from_str(json);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().as_u64(), u64::MAX);
}

#[test]
fn serde_json_zero_for_duration_ms() {
    let json = "0";
    let result: Result<DurationMs, _> = serde_json::from_str(json);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().as_u64(), 0);
}

#[test]
fn serde_rejects_malformed_json_instance_id() {
    let json = "\"unterminated";
    let result: Result<InstanceId, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_rejects_malformed_json_sequence_number() {
    let json = "not a number";
    let result: Result<SequenceNumber, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_string_type_rejects_number_for_workflow_name() {
    let json = "42";
    let result: Result<WorkflowName, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_string_type_rejects_number_for_node_name() {
    let json = "42";
    let result: Result<NodeName, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_string_type_rejects_number_for_binary_hash() {
    let json = "42";
    let result: Result<BinaryHash, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_string_type_rejects_number_for_timer_id() {
    let json = "42";
    let result: Result<TimerId, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn serde_string_type_rejects_number_for_idempotency_key() {
    let json = "42";
    let result: Result<IdempotencyKey, _> = serde_json::from_str(json);
    assert!(result.is_err());
}
