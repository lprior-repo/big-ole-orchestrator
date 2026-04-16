//! RED-QUEEN coevolutionary adversarial tests for vo-ipc. Coevolves attack/defense across FD framing, envelope schema, and SPSC overflow.

use std::io::Cursor;
use std::sync::Arc;
use vo_ipc::{
    envelope::{read_envelope, write_envelope, validate_identity, MAX_PAYLOAD_SIZE},
    spsc::{SpscError, SpscQueue},
    Fd3Envelope, Fd4Envelope, IpcError, TaskResult,
};

/// Build a wire frame: 4-byte BE length prefix + JSON payload.
fn frame(json: &str) -> Vec<u8> {
    let mut buf = (json.len() as u32).to_be_bytes().to_vec();
    buf.extend_from_slice(json.as_bytes());
    buf
}

// --- FD framing attacks ---

#[tokio::test]
async fn rq_zero_length_header_is_incomplete_read() {
    let mut empty = Cursor::new(Vec::<u8>::new());
    let err = read_envelope::<Fd4Envelope>(&mut empty).unwrap_err();
    assert!(matches!(err, IpcError::IncompleteRead { expected: 4, actual: 0 }));
}

#[tokio::test]
async fn rq_truncated_header_partial_bytes() {
    for n in 1..=3 {
        let mut c = Cursor::new(vec![0xFF; n]);
        let err = read_envelope::<Fd4Envelope>(&mut c).unwrap_err();
        assert!(matches!(err, IpcError::IncompleteRead { expected: 4, actual: a } if a == n));
    }
}

#[tokio::test]
async fn rq_oversized_payload_rejected_no_oom() {
    let mut buf = (MAX_PAYLOAD_SIZE + 1).to_be_bytes().to_vec();
    buf.push(0x00);
    let err = read_envelope::<Fd4Envelope>(&mut Cursor::new(buf)).unwrap_err();
    assert!(matches!(err, IpcError::PayloadTooLarge(_)));
}

#[tokio::test]
async fn rq_header_ok_payload_truncated() {
    let valid = Fd4Envelope {
        version: 1, instance_id: "t".into(), node_id: "n".into(),
        result: TaskResult::Success { output: serde_json::json!("x") },
    };
    let mut full = Vec::new();
    write_envelope(&mut full, &valid).unwrap();
    full.truncate(full.len() - 1);
    let err = read_envelope::<Fd4Envelope>(&mut Cursor::new(full)).unwrap_err();
    assert!(matches!(err, IpcError::IncompleteRead { .. }));
}

// --- Envelope schema attacks ---

#[tokio::test]
async fn rq_version_zero_rejected() {
    let raw = frame(r#"{"version":0,"instance_id":"a","node_id":"b","result":{"success":{"output":"x"}}}"#);
    let err = read_envelope::<Fd4Envelope>(&mut Cursor::new(raw)).unwrap_err();
    assert!(matches!(err, IpcError::VersionMismatch(0)));
}

#[tokio::test]
async fn rq_version_255_rejected() {
    let raw = frame(r#"{"version":255,"instance_id":"a","node_id":"b","result":{"success":{"output":"x"}}}"#);
    let err = read_envelope::<Fd4Envelope>(&mut Cursor::new(raw)).unwrap_err();
    assert!(matches!(err, IpcError::VersionMismatch(255)));
}

#[tokio::test]
async fn rq_empty_and_special_char_ids_rejected() {
    for id in ["", "has-dash", "p@ss"] {
        let raw = frame(&format!(
            r#"{{"version":1,"instance_id":"{id}","node_id":"b","result":{{"success":{{"output":"x"}}}}}}"#
        ));
        let err = read_envelope::<Fd4Envelope>(&mut Cursor::new(raw)).unwrap_err();
        assert!(matches!(err, IpcError::SchemaViolation(_)), "id '{id}' rejected");
    }
}

#[tokio::test]
async fn rq_identity_spoof_detected() {
    let spoofed = Fd4Envelope {
        version: 1, instance_id: "attacker".into(), node_id: "evil".into(),
        result: TaskResult::Success { output: serde_json::json!({"pwned": true}) },
    };
    let err = validate_identity(&spoofed, "victim-instance", "victim-node").unwrap_err();
    assert!(matches!(err, IpcError::IdentityMismatch { .. }));
    let msg = format!("{err}");
    assert!(msg.contains("attacker") && msg.contains("victim-instance"));
}

#[tokio::test]
async fn rq_unknown_fields_ignored_gracefully() {
    let raw = frame(r#"{"version":1,"instance_id":"a","node_id":"b","injected":"evil","result":{"success":{"output":"x"}}}"#);
    let env = read_envelope::<Fd4Envelope>(&mut Cursor::new(raw)).unwrap();
    assert_eq!(env.instance_id, "a");
}

// --- SPSC channel overflow coevolution ---

#[test]
fn rq_spsc_fill_to_capacity_then_reject() {
    let q = Arc::new(SpscQueue::<u64>::new(16));
    let (tx, rx) = q.sender();
    for i in 0..16 { tx.send(i).unwrap(); }
    assert_eq!(tx.send(999), Err(SpscError::Full));
    for i in 0..16 { assert_eq!(rx.recv().unwrap(), i); }
    assert_eq!(rx.recv(), Err(SpscError::Empty));
}

#[test]
fn rq_spsc_wraparound_preserves_order() {
    let q = Arc::new(SpscQueue::<i32>::new(4));
    let (tx, rx) = q.sender();
    for r in 0..2 {
        for i in 0..4 { tx.send(r * 100 + i).unwrap(); }
        for i in 0..4 { assert_eq!(rx.recv().unwrap(), r * 100 + i); }
    }
}

#[test]
fn rq_spsc_capacity_one_boundary() {
    let q = Arc::new(SpscQueue::<()>::new(1));
    let (tx, rx) = q.sender();
    tx.send(()).unwrap();
    assert_eq!(tx.send(()), Err(SpscError::Full));
    rx.recv().unwrap();
    tx.send(()).unwrap();
    rx.recv().unwrap();
    assert_eq!(rx.recv(), Err(SpscError::Empty));
}

// --- Wire roundtrip ---

#[tokio::test]
async fn rq_roundtrip_large_payload() {
    let env = Fd3Envelope {
        version: 1, instance_id: "perf".into(), node_id: "stress".into(),
        input: serde_json::json!({"key": "x".repeat(100_000)}),
        secrets: std::collections::BTreeMap::new(),
        metadata: std::collections::BTreeMap::new(),
    };
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();
    let parsed: Fd3Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();
    assert_eq!(env, parsed);
}
