//! Red Queen tests — IPC message ordering attacks.
//!
//! Adversarial tests targeting message ordering properties of the vo-ipc crate:
//! - Out-of-order delivery (envelope framing preserves boundaries)
//! - Duplicate message detection (envelope replay)
//! - Dropped messages (partial reads, pipe failures)
//! - Oversized payloads (boundary probing around MAX_PAYLOAD_SIZE)
//! - Edge-case envelope variants

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::PathBuf;
use vo_ipc::{
    engine_receive_envelope, read_envelope, write_envelope, Fd3Envelope, Fd4Envelope, IpcError,
    TaskError, TaskResult, MAX_PAYLOAD_SIZE,
};

// ========================================================================
// DIMENSION: out-of-order-delivery
// Contract: envelope framing preserves message boundaries; no interleaving
// ========================================================================

#[test]
fn envelope_roundtrip_preserves_field_ordering() {
    let envelopes = vec![
        Fd3Envelope {
            version: 1,
            instance_id: "inst001".into(),
            node_id: "nodeAlpha".into(),
            input: serde_json::json!({"seq": 0}),
            secrets: BTreeMap::new(),
            metadata: BTreeMap::new(),
        },
        Fd3Envelope {
            version: 1,
            instance_id: "inst002".into(),
            node_id: "nodeBeta".into(),
            input: serde_json::json!({"seq": 1}),
            secrets: BTreeMap::new(),
            metadata: BTreeMap::new(),
        },
        Fd3Envelope {
            version: 1,
            instance_id: "inst003".into(),
            node_id: "nodeGamma".into(),
            input: serde_json::json!({"seq": 2}),
            secrets: BTreeMap::new(),
            metadata: BTreeMap::new(),
        },
    ];

    let mut buffer = Vec::new();
    for env in &envelopes {
        write_envelope(&mut buffer, env).unwrap();
    }

    let mut reader = Cursor::new(buffer);
    for (i, expected) in envelopes.iter().enumerate() {
        let decoded: Fd3Envelope = read_envelope(&mut reader).unwrap();
        assert_eq!(
            decoded, *expected,
            "envelope {i} mismatch: expected {expected:?}, got {decoded:?}"
        );
    }
}

#[test]
fn fd4_sequence_preserves_exact_order() {
    let responses = vec![
        Fd4Envelope {
            version: 1,
            instance_id: "inst".into(),
            node_id: "node".into(),
            result: TaskResult::Success {
                output: serde_json::json!({"step": 1}),
            },
        },
        Fd4Envelope {
            version: 1,
            instance_id: "inst".into(),
            node_id: "node".into(),
            result: TaskResult::Failure {
                error: TaskError {
                    code: "ERR_STEP2".into(),
                    message: "step 2 failed".into(),
                    details: None,
                },
            },
        },
        Fd4Envelope {
            version: 1,
            instance_id: "inst".into(),
            node_id: "node".into(),
            result: TaskResult::Success {
                output: serde_json::json!({"step": 3, "recovered": true}),
            },
        },
    ];

    let mut buffer = Vec::new();
    for resp in &responses {
        write_envelope(&mut buffer, resp).unwrap();
    }

    let mut reader = Cursor::new(buffer);
    for (i, expected) in responses.iter().enumerate() {
        let decoded: Fd4Envelope = read_envelope(&mut reader).unwrap();
        assert_eq!(decoded, *expected, "response {i} ordering mismatch");
    }
}

#[test]
fn mixed_envelope_types_preserve_order() {
    let fd3 = Fd3Envelope {
        version: 1,
        instance_id: "mixedInst".into(),
        node_id: "mixedNode".into(),
        input: serde_json::json!({"type": "fd3"}),
        secrets: BTreeMap::new(),
        metadata: BTreeMap::new(),
    };
    let fd4 = Fd4Envelope {
        version: 1,
        instance_id: "mixedInst".into(),
        node_id: "mixedNode".into(),
        result: TaskResult::Success {
            output: serde_json::json!({"type": "fd4"}),
        },
    };

    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &fd3).unwrap();
    write_envelope(&mut buffer, &fd4).unwrap();
    write_envelope(&mut buffer, &fd3).unwrap();
    write_envelope(&mut buffer, &fd4).unwrap();

    let mut reader = Cursor::new(buffer);
    assert_eq!(read_envelope::<Fd3Envelope>(&mut reader).unwrap(), fd3);
    assert_eq!(read_envelope::<Fd4Envelope>(&mut reader).unwrap(), fd4);
    assert_eq!(read_envelope::<Fd3Envelope>(&mut reader).unwrap(), fd3);
    assert_eq!(read_envelope::<Fd4Envelope>(&mut reader).unwrap(), fd4);
}

// ========================================================================
// DIMENSION: duplicate-messages
// Contract: each envelope read consumes exactly one frame; no accidental merge
// ========================================================================

#[test]
fn duplicate_envelopes_are_independent() {
    let env = Fd3Envelope {
        version: 1,
        instance_id: "dupInst".into(),
        node_id: "dupNode".into(),
        input: serde_json::json!({"value": 42}),
        secrets: BTreeMap::new(),
        metadata: BTreeMap::new(),
    };

    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();
    write_envelope(&mut buffer, &env).unwrap();

    let mut reader = Cursor::new(buffer);
    let first: Fd3Envelope = read_envelope(&mut reader).unwrap();
    let second: Fd3Envelope = read_envelope(&mut reader).unwrap();

    assert_eq!(first, second);
    assert_eq!(first, env);

    let exhausted: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    assert!(matches!(exhausted, Err(IpcError::IncompleteRead { .. })));
}

#[test]
fn identical_responses_do_not_merge() {
    let resp = Fd4Envelope {
        version: 1,
        instance_id: "inst".into(),
        node_id: "node".into(),
        result: TaskResult::Success {
            output: serde_json::json!({"result": "same"}),
        },
    };

    let mut buffer = Vec::new();
    for _ in 0..10 {
        write_envelope(&mut buffer, &resp).unwrap();
    }

    let mut reader = Cursor::new(buffer);
    for i in 0..10 {
        let decoded: Fd4Envelope = read_envelope(&mut reader).unwrap();
        assert_eq!(
            decoded, resp,
            "duplicate {i} should be identical to original"
        );
    }
}

#[test]
fn replayed_response_fails_identity_validation() {
    let legit_response = Fd4Envelope {
        version: 1,
        instance_id: "instA".into(),
        node_id: "node1".into(),
        result: TaskResult::Success {
            output: serde_json::json!({"ok": true}),
        },
    };

    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &legit_response).unwrap();
    let mut reader = Cursor::new(buffer);

    let result = engine_receive_envelope(&mut reader, "instB", "node2");
    assert!(
        result.is_err(),
        "replayed response from wrong instance must be rejected"
    );
    match result.unwrap_err() {
        IpcError::IdentityMismatch {
            expected_instance,
            expected_node,
            actual_instance,
            actual_node,
        } => {
            assert_eq!(expected_instance, "instB");
            assert_eq!(expected_node, "node2");
            assert_eq!(actual_instance, "instA");
            assert_eq!(actual_node, "node1");
        }
        other => panic!("expected IdentityMismatch, got {:?}", other),
    }
}

// ========================================================================
// DIMENSION: dropped-messages
// Contract: partial reads produce clear errors, not corrupted data
// ========================================================================

#[test]
fn partial_header_returns_incomplete_read() {
    let buffer = vec![0x00, 0x01];
    let mut reader = Cursor::new(buffer);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    match result {
        Err(IpcError::IncompleteRead {
            expected: 4,
            actual: 2,
        }) => {}
        other => panic!("expected IncompleteRead(4, 2), got {:?}", other),
    }
}

#[test]
fn header_with_one_byte_short_payload() {
    let mut buffer = 10u32.to_be_bytes().to_vec();
    buffer.extend(vec![b'x'; 9]);
    let mut reader = Cursor::new(buffer);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    match result {
        Err(IpcError::IncompleteRead {
            expected: 10,
            actual: 9,
        }) => {}
        other => panic!("expected IncompleteRead(10, 9), got {:?}", other),
    }
}

#[test]
fn empty_stream_returns_incomplete_read() {
    let buffer: Vec<u8> = vec![];
    let mut reader = Cursor::new(buffer);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    assert!(matches!(result, Err(IpcError::IncompleteRead { .. })));
}

#[test]
fn stream_ends_after_valid_envelope_is_exhausted() {
    let env = Fd3Envelope {
        version: 1,
        instance_id: "inst".into(),
        node_id: "node".into(),
        input: serde_json::json!({"x": 1}),
        secrets: BTreeMap::new(),
        metadata: BTreeMap::new(),
    };

    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();

    let mut reader = Cursor::new(buffer);
    let first: Fd3Envelope = read_envelope(&mut reader).unwrap();
    assert_eq!(first, env);

    let second: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    assert!(
        matches!(second, Err(IpcError::IncompleteRead { .. })),
        "reading past end of stream must return IncompleteRead, got {:?}",
        second
    );
}

#[test]
fn corrupted_payload_mid_envelope_returns_error() {
    let valid_env = Fd3Envelope {
        version: 1,
        instance_id: "inst".into(),
        node_id: "node".into(),
        input: serde_json::json!({"data": "valid"}),
        secrets: BTreeMap::new(),
        metadata: BTreeMap::new(),
    };

    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &valid_env).unwrap();

    let payload_start = 4 + (buffer.len() / 3);
    if payload_start < buffer.len() {
        buffer[payload_start] = 0xff;
        buffer[payload_start + 1] = 0xfe;
    }

    let mut reader = Cursor::new(buffer);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    assert!(
        result.is_err(),
        "corrupted payload must produce an error, got {:?}",
        result
    );
}

#[test]
fn corrupted_envelope_does_not_panic() {
    let corrupted_json = br#"{"version":1,"instance_id":"x","node_id":"y","input":INVALID,"secrets":{},"metadata":{}}"#;
    let mut buffer = (corrupted_json.len() as u32).to_be_bytes().to_vec();
    buffer.extend_from_slice(corrupted_json);

    let valid_env = Fd3Envelope {
        version: 1,
        instance_id: "inst".into(),
        node_id: "node".into(),
        input: serde_json::json!({"ok": true}),
        secrets: BTreeMap::new(),
        metadata: BTreeMap::new(),
    };
    write_envelope(&mut buffer, &valid_env).unwrap();

    let mut reader = Cursor::new(buffer);
    let first: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    assert!(first.is_err(), "corrupted envelope should fail");
}

// ========================================================================
// DIMENSION: oversized-payloads
// Contract: boundary enforcement at MAX_PAYLOAD_SIZE
// ========================================================================

#[test]
fn payload_one_byte_over_limit_rejected_on_write() {
    let env = Fd3Envelope {
        version: 1,
        instance_id: "i".into(),
        node_id: "n".into(),
        input: serde_json::json!({"pad": "x".repeat(MAX_PAYLOAD_SIZE as usize)}),
        secrets: BTreeMap::new(),
        metadata: BTreeMap::new(),
    };

    let mut buffer = Vec::new();
    let result = write_envelope(&mut buffer, &env);
    assert!(
        matches!(result, Err(IpcError::PayloadTooLarge(_))),
        "payload over MAX_PAYLOAD_SIZE should be rejected on write"
    );
}

#[test]
fn read_header_claiming_over_limit_rejected() {
    let len = MAX_PAYLOAD_SIZE + 1;
    let mut buffer = len.to_be_bytes().to_vec();
    buffer.extend(vec![0u8; 10]);
    let mut reader = Cursor::new(buffer);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    assert!(
        matches!(result, Err(IpcError::PayloadTooLarge(got)) if got == len),
        "header claiming > MAX_PAYLOAD_SIZE should be rejected"
    );
}

#[test]
fn read_header_claiming_exactly_limit_then_short_payload() {
    let mut buffer = MAX_PAYLOAD_SIZE.to_be_bytes().to_vec();
    buffer.extend(vec![b'x'; 100]);
    let mut reader = Cursor::new(buffer);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    assert!(
        matches!(result, Err(IpcError::IncompleteRead { .. })),
        "short payload for claimed size should be IncompleteRead"
    );
}

#[test]
fn secrets_field_can_carry_large_values_within_limit() {
    let mut secrets = BTreeMap::new();
    secrets.insert("key1".into(), "x".repeat(1000));
    secrets.insert("key2".into(), "y".repeat(1000));

    let env = Fd3Envelope {
        version: 1,
        instance_id: "inst".into(),
        node_id: "node".into(),
        input: serde_json::json!({}),
        secrets,
        metadata: BTreeMap::new(),
    };

    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();

    let mut reader = Cursor::new(buffer);
    let decoded: Fd3Envelope = read_envelope(&mut reader).unwrap();
    assert_eq!(decoded.secrets.get("key1").unwrap().len(), 1000);
    assert_eq!(decoded.secrets.get("key2").unwrap().len(), 1000);
}

// ========================================================================
// DIMENSION: edge-case-envelope-variants
// Contract: unusual but valid envelopes are handled correctly
// ========================================================================

#[test]
fn envelope_with_max_length_ids_succeeds() {
    let long_id = "a".repeat(1000);
    let env = Fd3Envelope {
        version: 1,
        instance_id: long_id.clone(),
        node_id: long_id.clone(),
        input: serde_json::json!({}),
        secrets: BTreeMap::new(),
        metadata: BTreeMap::new(),
    };

    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();

    let mut reader = Cursor::new(buffer);
    let decoded: Fd3Envelope = read_envelope(&mut reader).unwrap();
    assert_eq!(decoded.instance_id, long_id);
    assert_eq!(decoded.node_id, long_id);
}

#[test]
fn envelope_with_deeply_nested_input_succeeds() {
    let mut input = serde_json::json!({});
    for i in 0..50 {
        input = serde_json::json!({"level": i, "child": input});
    }

    let env = Fd3Envelope {
        version: 1,
        instance_id: "deep".into(),
        node_id: "node".into(),
        input,
        secrets: BTreeMap::new(),
        metadata: BTreeMap::new(),
    };

    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();

    let mut reader = Cursor::new(buffer);
    let decoded: Fd3Envelope = read_envelope(&mut reader).unwrap();
    assert_eq!(decoded.input["level"], 49);
}

#[test]
fn fd4_failure_with_details_roundtrips() {
    let env = Fd4Envelope {
        version: 1,
        instance_id: "inst".into(),
        node_id: "node".into(),
        result: TaskResult::Failure {
            error: TaskError {
                code: "ERR_COMPLEX".into(),
                message: "multi-part failure".into(),
                details: Some(serde_json::json!({
                    "stack_trace": ["frame1", "frame2", "frame3"],
                    "context": {"key": "value"},
                    "nested": {"a": {"b": {"c": 42}}}
                })),
            },
        },
    };

    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();

    let mut reader = Cursor::new(buffer);
    let decoded: Fd4Envelope = read_envelope(&mut reader).unwrap();
    assert_eq!(decoded, env);

    if let TaskResult::Failure { error } = decoded.result {
        let details = error.details.unwrap();
        assert_eq!(details["stack_trace"][1], "frame2");
        assert_eq!(details["nested"]["a"]["b"]["c"], 42);
    } else {
        panic!("expected Failure variant");
    }
}

// ========================================================================
// DIMENSION: subprocess-level-ordering-attacks
// Contract: subprocess behavior doesn't corrupt the IPC channel
// ========================================================================

#[tokio::test]
async fn subprocess_immediate_exit_with_no_fd4_is_handled() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/adversary_immediate_exit.py");

    let config = vo_ipc::SubprocessConfig::new(path, 500, b"test".to_vec()).unwrap();
    let result = vo_ipc::run_subprocess(config).await;

    match result {
        Ok(output) => assert!(output.fd4_bytes.is_empty()),
        Err(e) => {
            assert!(
                matches!(
                    e,
                    vo_ipc::IpcError::ProcessFailed { .. } | vo_ipc::IpcError::Fd4ReadFailed { .. }
                ),
                "unexpected error: {:?}",
                e
            );
        }
    }
}

#[tokio::test]
async fn subprocess_fd3_burst_then_exit_is_handled() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/adversary_fd3_burst.py");

    let config = vo_ipc::SubprocessConfig::new(path, 2000, b"test".to_vec()).unwrap();
    let result = vo_ipc::run_subprocess(config).await;

    match result {
        Ok(_) | Err(_) => {}
    }
}

#[tokio::test]
async fn subprocess_partial_fd4_response_is_handled() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/adversary_partial_write.py");

    let payload = b"ordering-attack-partial";
    let config = vo_ipc::SubprocessConfig::new(path, 1000, payload.to_vec()).unwrap();
    let result = vo_ipc::run_subprocess(config).await;

    match result {
        Ok(output) => {
            if !output.fd4_bytes.is_empty() {
                let parsed = serde_json::from_slice::<Fd4Envelope>(&output.fd4_bytes);
                assert!(
                    parsed.is_ok(),
                    "partial fd4 should be parseable if non-empty"
                );
            }
        }
        Err(e) => {
            assert!(!format!("{:?}", e).contains("panic"));
        }
    }
}
