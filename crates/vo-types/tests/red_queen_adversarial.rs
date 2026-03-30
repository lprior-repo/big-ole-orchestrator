use vo_types::events::EventEnvelope;
use vo_types::RetryPolicy;

#[test]
fn rq_break_retry_policy_via_deserialization() {
    // Contract: RetryPolicy requires max_attempts >= 1 and backoff_multiplier >= 1.0.
    // Adversarial attack: Bypass RetryPolicy::new using serde_json.
    let json = r#"{
        "max_attempts": 0,
        "backoff_ms": 100,
        "backoff_multiplier": 0.5
    }"#;

    let policy: RetryPolicy =
        serde_json::from_str(json).expect("Deserialization succeeds but shouldn't");

    // Contract is broken! We have an invalid state.
    assert_eq!(policy.max_attempts, 0);
    assert_eq!(policy.backoff_multiplier, 0.5);
}

#[test]
fn rq_break_retry_policy_via_public_fields() {
    // Adversarial attack: Bypass RetryPolicy::new using struct literal because fields are public.
    let policy = RetryPolicy {
        max_attempts: 0,
        backoff_ms: 100,
        backoff_multiplier: f32::NAN,
    };

    // Contract is broken!
    assert_eq!(policy.max_attempts, 0);
    assert!(policy.backoff_multiplier.is_nan());
}

#[test]
fn rq_break_event_envelope_massive_instance_id() {
    // Adversarial attack: Provide an extremely large instance_id string to exhaust memory or cause issues in storage.
    let giant_string = "a".repeat(10 * 1024 * 1024); // 10MB string
    let json = format!(
        r#"{{
        "version": 1,
        "instance_id": "{}",
        "sequence": 1,
        "timestamp_ms": 12345,
        "payload": {{"type": "WorkflowStarted", "workflow_id": "w1"}},
        "metadata": {{}}
    }}"#,
        giant_string
    );

    let envelope = EventEnvelope::from_str(&json).expect("Massive instance_id is accepted");
    assert_eq!(envelope.instance_id.len(), 10 * 1024 * 1024);
}

#[test]
fn rq_break_event_envelope_max_timestamp() {
    // Adversarial attack: Edge case timestamp to cause potential overflow in timer arithmetic
    let json = r#"{
        "version": 1,
        "instance_id": "inst-1",
        "sequence": 1,
        "timestamp_ms": 18446744073709551615,
        "payload": {"type": "WorkflowStarted", "workflow_id": "w1"},
        "metadata": {}
    }"#;

    let envelope = EventEnvelope::from_str(json).expect("Max u64 timestamp is accepted");
    assert_eq!(envelope.timestamp_ms, u64::MAX);
}
