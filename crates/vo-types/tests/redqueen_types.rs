//! RED-QUEEN coevolutionary adversarial tests for vo-types.
//! Attacks: serde bypass of constructors, schema evolution, type confusion,
//! roundtrip corruption, and field injection through derive macros.

use serde_json::json;
use vo_types::{BlobRef, EncryptedBlob, EventEnvelope, InstanceId, WrappedDek, WorkflowName};

// ── Serde Derive Bypasses Constructor Validation ──────────────────────────────

#[test]
fn wrapped_dek_serde_accepts_short_bytes() {
    let short: WrappedDek =
        serde_json::from_value(json!([1, 2, 3])).expect("serde bypasses constructor");
    assert_eq!(short.as_bytes().len(), 3);
}

#[test]
fn encrypted_blob_serde_accepts_wrong_iv_and_tag_lengths() {
    let blob: EncryptedBlob = serde_json::from_value(json!({
        "iv": [0], "ciphertext": [42], "tag": [0, 0]
    }))
    .expect("serde accepts iv=1, tag=2");
    assert_eq!(blob.iv.len(), 1);
    assert_eq!(blob.tag.len(), 2);
}

#[test]
fn blob_ref_serde_accepts_zero_size_and_bad_hash() {
    let bad: BlobRef = serde_json::from_value(json!({
        "blob_id": "00000000000000000000000000",
        "content_hash": "ZZZZZZ", "size_bytes": 0
    }))
    .expect("serde accepts size=0 and uppercase hash");
    assert_eq!(bad.size_bytes(), 0);
}

// ── Schema Evolution ───────────────────────────────────────────────────────────

#[test]
fn event_envelope_rejects_future_schema_version() {
    let json_str = serde_json::to_string(&json!({
        "version": 255,
        "instance_id": "01H0CHPTKV0ME1N2A3B4C5D6E7",
        "sequence": 1, "timestamp_ms": 1000,
        "payload": {"type": "WorkflowStarted"}, "metadata": {}
    })).unwrap();
    let result = EventEnvelope::from_str(&json_str);
    assert!(result.is_err());
}

#[test]
fn event_envelope_rejects_zero_sequence() {
    let json_str = serde_json::to_string(&json!({
        "version": 1,
        "instance_id": "01H0CHPTKV0ME1N2A3B4C5D6E7",
        "sequence": 0, "timestamp_ms": 1000,
        "payload": {"type": "WorkflowStarted"}, "metadata": {}
    })).unwrap();
    let result = EventEnvelope::from_str(&json_str);
    assert!(result.is_err());
}

#[test]
fn event_envelope_rejects_empty_instance_id() {
    let json_str = serde_json::to_string(&json!({
        "version": 1, "instance_id": "",
        "sequence": 1, "timestamp_ms": 1000,
        "payload": {"type": "WorkflowStarted"}, "metadata": {}
    })).unwrap();
    let result = EventEnvelope::from_str(&json_str);
    assert!(result.is_err());
}

#[test]
fn event_envelope_rejects_non_object_payload() {
    for bad_payload in [json!(42), json!("string"), json!(null), json!([])] {
        let json_str = serde_json::to_string(&json!({
            "version": 1,
            "instance_id": "01H0CHPTKV0ME1N2A3B4C5D6E7",
            "sequence": 1, "timestamp_ms": 1000,
            "payload": bad_payload, "metadata": {}
        })).unwrap();
        let result = EventEnvelope::from_str(&json_str);
        assert!(result.is_err());
    }
}

// ── String Newtype Adversarial ─────────────────────────────────────────────────

#[test]
fn instance_id_rejects_nil_ulid() {
    let result: Result<InstanceId, _> =
        serde_json::from_value(json!("00000000000000000000000000"));
    assert!(result.is_err());
}

#[test]
fn workflow_name_rejects_consecutive_separators() {
    for bad in ["foo--bar", "foo__bar", "foo-_bar", "foo_-bar"] {
        let result: Result<WorkflowName, _> = serde_json::from_value(json!(bad));
        assert!(result.is_err(), "must reject '{bad}'");
    }
}

#[test]
fn workflow_name_rejects_leading_trailing_separators() {
    for bad in ["-foo", "foo-", "foo_"] {
        let result: Result<WorkflowName, _> = serde_json::from_value(json!(bad));
        assert!(result.is_err(), "must reject '{bad}'");
    }
    // Note: leading underscore IS valid per design (_foo accepted)
}

#[test]
fn workflow_name_roundtrip_preserves_normalization() {
    let name = WorkflowName::parse("my-workflow_v2").unwrap();
    let ser = serde_json::to_string(&name).unwrap();
    let back: WorkflowName = serde_json::from_str(&ser).unwrap();
    assert_eq!(name, back);
}

// ── Type Confusion ─────────────────────────────────────────────────────────────

#[test]
fn instance_id_rejects_wrong_json_types() {
    for bad in [json!(123), json!(true), json!(null), json!([]), json!({})] {
        let result: Result<InstanceId, _> = serde_json::from_value(bad);
        assert!(result.is_err());
    }
}

// ── Roundtrip Integrity ───────────────────────────────────────────────────────

#[test]
fn event_envelope_roundtrip_fidelity() {
    let raw = r#"{"schema_version":1,"instance_id":"01H0CHPTKV0ME1N2A3B4C5D6E7","sequence":1,"timestamp_ms":1000,"payload":{"type":"WorkflowStarted"},"metadata":{}}"#;
    let env: EventEnvelope = serde_json::from_str(raw).unwrap();
    let out = serde_json::to_string(&env).unwrap();
    let back: EventEnvelope = serde_json::from_str(&out).unwrap();
    assert_eq!(env.instance_id, back.instance_id);
    assert_eq!(env.sequence, back.sequence);
    assert_eq!(env.payload, back.payload);
}

#[test]
fn event_envelope_rejects_missing_required_fields() {
    let result = serde_json::from_value::<EventEnvelope>(json!({"schema_version": 1}));
    assert!(result.is_err());
}

#[test]
fn instance_id_normalizes_to_uppercase() {
    let id: InstanceId =
        serde_json::from_value(json!("01h0chptkv0me1n2a3b4c5d6e7")).unwrap();
    let ser = serde_json::to_string(&id).unwrap();
    assert!(ser.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '"'));
}
