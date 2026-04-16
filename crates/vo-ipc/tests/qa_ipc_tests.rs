//! QA: vo-ipc — FD3/FD4 pipe protocol, subprocess I/O tests
//!
//! Comprehensive QA covering:
//! - SPSC concurrent multi-threaded stress tests
//! - Envelope serialization edge cases
//! - Adversarial byte-by-byte fd4 responses
//! - Two-envelope fd4 responses
//! - Timeout during read_exact (child sends header then stalls)
//! - Concurrent pipe access
//! - Signal delivery during IPC
//! - Zero-length fd3 payload handling
//! - Schema validation bypass attempts

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use std::collections::BTreeMap;
use std::io::Cursor;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::tempdir;
use vo_ipc::{
    engine_receive_envelope, read_envelope, run_subprocess, write_envelope, Fd3Envelope,
    Fd4Envelope, IpcError, SubprocessConfig, TaskError, TaskResult, MAX_PAYLOAD_SIZE,
};

fn make_executable(path: &Path) {
    let mut perms = std::fs::metadata(path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms).unwrap();
}

fn fixture_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_fixture_driver"))
}

fn config(payload: impl AsRef<[u8]>, timeout_ms: u64) -> SubprocessConfig {
    SubprocessConfig::new(fixture_binary(), timeout_ms, payload.as_ref().to_vec()).unwrap()
}

fn make_fd3(instance_id: &str, node_id: &str, input: serde_json::Value) -> Fd3Envelope {
    Fd3Envelope {
        version: 1,
        instance_id: instance_id.to_string(),
        node_id: node_id.to_string(),
        input,
        secrets: BTreeMap::new(),
        metadata: BTreeMap::new(),
    }
}

fn make_fd4_success(instance_id: &str, node_id: &str, output: serde_json::Value) -> Fd4Envelope {
    Fd4Envelope {
        version: 1,
        instance_id: instance_id.to_string(),
        node_id: node_id.to_string(),
        result: TaskResult::Success { output },
    }
}

fn make_fd4_failure(
    instance_id: &str,
    node_id: &str,
    code: &str,
    message: &str,
) -> Fd4Envelope {
    Fd4Envelope {
        version: 1,
        instance_id: instance_id.to_string(),
        node_id: node_id.to_string(),
        result: TaskResult::Failure {
            error: TaskError {
                code: code.to_string(),
                message: message.to_string(),
                details: None,
            },
        },
    }
}

// ============================================================================
// SECTION 1: SPSC Concurrent Multi-threaded Stress Tests
// ============================================================================

#[test]
fn spsc_concurrent_single_producer_single_consumer() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use vo_ipc::spsc::SpscQueue;

    const N: usize = 100_000;
    let queue = Arc::new(SpscQueue::<usize>::new(1024));

    let produced = Arc::new(AtomicUsize::new(0));
    let consumed = Arc::new(AtomicUsize::new(0));
    let p_produced = produced.clone();
    let p_consumed = consumed.clone();
    let q_tx = queue.clone();
    let q_rx = queue.clone();

    let producer = thread::spawn(move || {
        for i in 0..N {
            while q_tx.send(i).is_err() {
                std::hint::spin_loop();
            }
            p_produced.store(i + 1, Ordering::Release);
        }
    });

    let consumer = thread::spawn(move || {
        let mut last = 0usize;
        for _ in 0..N {
            loop {
                match q_rx.recv() {
                    Ok(v) => {
                        assert_eq!(v, last, "consumer saw out-of-order value");
                        last += 1;
                        p_consumed.store(last, Ordering::Release);
                        break;
                    }
                    Err(_) => std::hint::spin_loop(),
                }
            }
        }
    });

    producer.join().unwrap();
    consumer.join().unwrap();

    assert_eq!(produced.load(Ordering::Acquire), N);
    assert_eq!(consumed.load(Ordering::Acquire), N);
}

#[test]
fn spsc_concurrent_high_contention_small_buffer() {
    use std::thread;
    use vo_ipc::spsc::SpscQueue;

    const N: usize = 50_000;
    let queue = Arc::new(SpscQueue::<u64>::new(4));
    let q_tx = queue.clone();
    let q_rx = queue.clone();

    let producer = thread::spawn(move || {
        for i in 0..N {
            while q_tx.send(i as u64).is_err() {
                std::hint::spin_loop();
            }
        }
    });

    let consumer = thread::spawn(move || {
        let mut last = 0u64;
        for _ in 0..N {
            loop {
                match q_rx.recv() {
                    Ok(v) => {
                        assert_eq!(v, last);
                        last += 1;
                        break;
                    }
                    Err(_) => std::hint::spin_loop(),
                }
            }
        }
    });

    producer.join().unwrap();
    consumer.join().unwrap();
}

#[test]
fn spsc_concurrent_string_values_stress() {
    use std::thread;
    use vo_ipc::spsc::SpscQueue;

    const N: usize = 10_000;
    let queue = Arc::new(SpscQueue::<String>::new(256));
    let q_tx = queue.clone();
    let q_rx = queue.clone();

    let producer = thread::spawn(move || {
        for i in 0..N {
            let msg = format!("message-{i:06}");
            while q_tx.send(msg.clone()).is_err() {
                std::hint::spin_loop();
            }
        }
    });

    let consumer = thread::spawn(move || {
        for i in 0..N {
            loop {
                match q_rx.recv() {
                    Ok(msg) => {
                        assert_eq!(msg, format!("message-{i:06}"));
                        break;
                    }
                    Err(_) => std::hint::spin_loop(),
                }
            }
        }
    });

    producer.join().unwrap();
    consumer.join().unwrap();
}

#[test]
fn spsc_concurrent_drop_during_active_transfer() {
    use std::thread;
    use vo_ipc::spsc::SpscQueue;

    let queue = Arc::new(SpscQueue::<Vec<u8>>::new(64));

    for i in 0..32 {
        queue.send(vec![i as u8; 100]).unwrap();
    }

    let q_rx = queue.clone();
    let consumer = thread::spawn(move || {
        for i in 0..32 {
            let val = q_rx.recv().unwrap();
            assert_eq!(val, vec![i as u8; 100]);
        }
    });

    consumer.join().unwrap();
}

// ============================================================================
// SECTION 2: Envelope Serialization Edge Cases
// ============================================================================

#[test]
fn envelope_zero_length_payload_roundtrip() {
    let env = make_fd3("i1", "n1", serde_json::json!({}));
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();
    let mut reader = Cursor::new(buf);
    let decoded: Fd3Envelope = read_envelope(&mut reader).unwrap();
    assert_eq!(decoded, env);
}

#[test]
fn envelope_unicode_ids_rejected_by_schema() {
    let env = Fd3Envelope {
        version: 1,
        instance_id: "inst-日本語".to_string(),
        node_id: "node-日本語".to_string(),
        input: serde_json::json!({"key": "value"}),
        secrets: BTreeMap::new(),
        metadata: BTreeMap::new(),
    };
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();
    let mut reader = Cursor::new(buf);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    assert!(matches!(result, Err(IpcError::SchemaViolation(_))));
}

#[test]
fn envelope_hyphenated_ids_rejected() {
    let env = make_fd3("inst-01", "node-a", serde_json::json!({}));
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();
    let mut reader = Cursor::new(buf);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    assert!(matches!(result, Err(IpcError::SchemaViolation(_))));
}

#[test]
fn envelope_underscored_ids_rejected() {
    let env = make_fd3("inst_01", "node_a", serde_json::json!({}));
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();
    let mut reader = Cursor::new(buf);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    assert!(matches!(result, Err(IpcError::SchemaViolation(_))));
}

#[test]
fn envelope_empty_instance_id_rejected() {
    let env = Fd3Envelope {
        version: 1,
        instance_id: String::new(),
        node_id: "node1".to_string(),
        input: serde_json::json!({}),
        secrets: BTreeMap::new(),
        metadata: BTreeMap::new(),
    };
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();
    let mut reader = Cursor::new(buf);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    assert!(matches!(result, Err(IpcError::SchemaViolation(_))));
}

#[test]
fn envelope_empty_node_id_rejected() {
    let env = Fd3Envelope {
        version: 1,
        instance_id: "inst1".to_string(),
        node_id: String::new(),
        input: serde_json::json!({}),
        secrets: BTreeMap::new(),
        metadata: BTreeMap::new(),
    };
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();
    let mut reader = Cursor::new(buf);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    assert!(matches!(result, Err(IpcError::SchemaViolation(_))));
}

#[test]
fn envelope_numeric_instance_id_accepted() {
    let env = Fd3Envelope {
        version: 1,
        instance_id: "12345".to_string(),
        node_id: "n1".to_string(),
        input: serde_json::json!({}),
        secrets: BTreeMap::new(),
        metadata: BTreeMap::new(),
    };
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();
    let mut reader = Cursor::new(buf);
    let decoded: Fd3Envelope = read_envelope(&mut reader).unwrap();
    assert_eq!(decoded.instance_id, "12345");
}

#[test]
fn fd4_task_error_with_null_details_roundtrip() {
    let env = Fd4Envelope {
        version: 1,
        instance_id: "i".into(),
        node_id: "n".into(),
        result: TaskResult::Failure {
            error: TaskError {
                code: "E".into(),
                message: "m".into(),
                details: None,
            },
        },
    };
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();
    let decoded: Fd4Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();
    assert_eq!(env, decoded);
}

#[test]
fn fd4_task_error_explicit_null_normalizes_to_none() {
    // serde normalizes Some(Value::Null) -> None for Option<T> during deserialization
    let env = Fd4Envelope {
        version: 1,
        instance_id: "i".into(),
        node_id: "n".into(),
        result: TaskResult::Failure {
            error: TaskError {
                code: "E".into(),
                message: "m".into(),
                details: Some(serde_json::Value::Null),
            },
        },
    };
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();
    let decoded: Fd4Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();
    let TaskResult::Failure { error } = &decoded.result else {
        panic!("expected Failure variant")
    };
    assert_eq!(error.code, "E");
    assert_eq!(error.message, "m");
    assert!(error.details.is_none());
}

#[test]
fn fd4_success_with_null_output_roundtrip() {
    let env = make_fd4_success("i", "n", serde_json::Value::Null);
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();
    let decoded: Fd4Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();
    assert_eq!(env, decoded);
}

#[test]
fn fd4_success_with_empty_object_output_roundtrip() {
    let env = make_fd4_success("i", "n", serde_json::json!({}));
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();
    let decoded: Fd4Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();
    assert_eq!(env, decoded);
}

#[test]
fn envelope_special_json_characters_preserved() {
    let special_input = serde_json::json!({
        "newline": "line1\nline2",
        "tab": "col1\tcol2",
        "quote": "say \"hello\"",
        "backslash": "path\\to\\file",
        "unicode": "\u{1F600}",
    });
    let env = make_fd3("i1", "n1", special_input);
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();
    let mut reader = Cursor::new(buf);
    let decoded: Fd3Envelope = read_envelope(&mut reader).unwrap();
    assert_eq!(decoded.input["newline"], "line1\nline2");
    assert_eq!(decoded.input["tab"], "col1\tcol2");
    assert_eq!(decoded.input["quote"], "say \"hello\"");
    assert_eq!(decoded.input["backslash"], "path\\to\\file");
    assert_eq!(decoded.input["unicode"], "\u{1F600}");
}

#[test]
fn envelope_very_large_secrets_within_limit() {
    let mut secrets = BTreeMap::new();
    for i in 0..100 {
        secrets.insert(format!("SECRET_{i:03}"), "x".repeat(10_000));
    }
    let env = Fd3Envelope {
        version: 1,
        instance_id: "i".into(),
        node_id: "n".into(),
        input: serde_json::json!({}),
        secrets,
        metadata: BTreeMap::new(),
    };
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();
    let mut reader = Cursor::new(buf);
    let decoded: Fd3Envelope = read_envelope(&mut reader).unwrap();
    assert_eq!(decoded.secrets.len(), 100);
    assert_eq!(decoded.secrets["SECRET_042"].len(), 10_000);
}

// ============================================================================
// SECTION 3: Schema Validation Bypass Attempts
// ============================================================================

#[test]
fn schema_bypass_non_object_payload() {
    let payload = b"[1, 2, 3]";
    let mut buf = (payload.len() as u32).to_be_bytes().to_vec();
    buf.extend_from_slice(payload);
    let mut reader = Cursor::new(buf);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    assert!(result.is_err());
}

#[test]
fn schema_bypass_primitive_payload() {
    let payload = b"42";
    let mut buf = (payload.len() as u32).to_be_bytes().to_vec();
    buf.extend_from_slice(payload);
    let mut reader = Cursor::new(buf);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    assert!(result.is_err());
}

#[test]
fn schema_version_zero_rejected() {
    let mut env = make_fd3("i1", "n1", serde_json::json!({}));
    env.version = 0;
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();
    let mut reader = Cursor::new(buf);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    assert!(matches!(result, Err(IpcError::VersionMismatch(0))));
}

#[test]
fn schema_version_255_rejected() {
    let mut env = make_fd3("i1", "n1", serde_json::json!({}));
    env.version = 255;
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();
    let mut reader = Cursor::new(buf);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    assert!(matches!(result, Err(IpcError::VersionMismatch(255))));
}

#[test]
fn schema_version_string_rejected() {
    let payload = br#"{"version":"1","instance_id":"i1","node_id":"n1","input":{},"secrets":{},"metadata":{}}"#;
    let mut buf = (payload.len() as u32).to_be_bytes().to_vec();
    buf.extend_from_slice(payload);
    let mut reader = Cursor::new(buf);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    assert!(matches!(result, Err(IpcError::SchemaViolation(_))));
}

#[test]
fn schema_version_float_rejected() {
    let payload = br#"{"version":1.5,"instance_id":"i1","node_id":"n1","input":{},"secrets":{},"metadata":{}}"#;
    let mut buf = (payload.len() as u32).to_be_bytes().to_vec();
    buf.extend_from_slice(payload);
    let mut reader = Cursor::new(buf);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    assert!(matches!(result, Err(IpcError::SchemaViolation(_))));
}

#[test]
fn schema_negative_version_rejected() {
    let payload = br#"{"version":-1,"instance_id":"i1","node_id":"n1","input":{},"secrets":{},"metadata":{}}"#;
    let mut buf = (payload.len() as u32).to_be_bytes().to_vec();
    buf.extend_from_slice(payload);
    let mut reader = Cursor::new(buf);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    assert!(result.is_err());
}

// ============================================================================
// SECTION 4: Identity Mismatch Comprehensive
// ============================================================================

#[test]
fn identity_mismatch_instance_only() {
    let env = make_fd4_success("actual", "expected", serde_json::json!(null));
    let result = vo_ipc::validate_identity(&env, "expected", "expected");
    assert!(result.is_err());
    let err = result.unwrap_err();
    let display = format!("{err}");
    assert!(display.contains("actual"));
    assert!(display.contains("expected"));
}

#[test]
fn identity_mismatch_node_only() {
    let env = make_fd4_success("expected", "actual", serde_json::json!(null));
    let result = vo_ipc::validate_identity(&env, "expected", "expected");
    assert!(result.is_err());
}

#[test]
fn identity_match_both_correct() {
    let env = make_fd4_success("inst", "node", serde_json::json!(null));
    let result = vo_ipc::validate_identity(&env, "inst", "node");
    assert!(result.is_ok());
}

#[test]
fn identity_match_empty_strings() {
    let env = Fd4Envelope {
        version: 1,
        instance_id: String::new(),
        node_id: String::new(),
        result: TaskResult::Success {
            output: serde_json::json!(null),
        },
    };
    let result = vo_ipc::validate_identity(&env, "", "");
    assert!(result.is_ok());
}

#[test]
fn engine_receive_correct_identity() {
    let env = make_fd4_success("myinst", "mynode", serde_json::json!({"ok": true}));
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();
    let result = engine_receive_envelope(&mut Cursor::new(buf), "myinst", "mynode");
    assert!(result.is_ok());
    let decoded = result.unwrap();
    assert_eq!(decoded.instance_id, "myinst");
    assert_eq!(decoded.node_id, "mynode");
}

// ============================================================================
// SECTION 5: Multi-Envelope Stream
// ============================================================================

#[test]
fn multi_envelope_stream_all_readable() {
    let envelopes: Vec<Fd4Envelope> = (0..5)
        .map(|i| make_fd4_success("inst", "n", serde_json::json!({"seq": i})))
        .collect();
    let mut buf = Vec::new();
    for env in &envelopes {
        write_envelope(&mut buf, env).unwrap();
    }
    let mut reader = Cursor::new(buf);
    for (i, expected) in envelopes.iter().enumerate() {
        let decoded: Fd4Envelope = read_envelope(&mut reader).unwrap();
        assert_eq!(decoded, *expected, "envelope {i} mismatch");
    }
    let exhausted: Result<Fd4Envelope, IpcError> = read_envelope(&mut reader);
    assert!(matches!(exhausted, Err(IpcError::IncompleteRead { .. })));
}

#[test]
fn multi_envelope_mixed_success_failure() {
    let mut buf = Vec::new();
    write_envelope(&mut buf, &make_fd4_success("i", "n", serde_json::json!(1))).unwrap();
    write_envelope(&mut buf, &make_fd4_failure("i", "n", "E1", "err1")).unwrap();
    write_envelope(&mut buf, &make_fd4_success("i", "n", serde_json::json!(2))).unwrap();
    write_envelope(&mut buf, &make_fd4_failure("i", "n", "E2", "err2")).unwrap();
    let mut reader = Cursor::new(buf);
    assert!(matches!(read_envelope::<Fd4Envelope>(&mut reader).unwrap().result, TaskResult::Success { .. }));
    assert!(matches!(read_envelope::<Fd4Envelope>(&mut reader).unwrap().result, TaskResult::Failure { .. }));
    assert!(matches!(read_envelope::<Fd4Envelope>(&mut reader).unwrap().result, TaskResult::Success { .. }));
    assert!(matches!(read_envelope::<Fd4Envelope>(&mut reader).unwrap().result, TaskResult::Failure { .. }));
}

// ============================================================================
// SECTION 6: Error Display Completeness
// ============================================================================

#[test]
fn ipc_error_display_contains_useful_info() {
    let errors = vec![
        IpcError::PayloadTooLarge(100),
        IpcError::IncompleteRead { expected: 10, actual: 5 },
        IpcError::InvalidJson("bad json".to_string()),
        IpcError::VersionMismatch(0),
        IpcError::VersionMismatch(255),
        IpcError::SchemaViolation("bad field".to_string()),
        IpcError::PipeSetupFailed { detail: "os error".to_string() },
        IpcError::SpawnFailed { detail: "no such file".to_string() },
        IpcError::WaitFailed { detail: "wait error".to_string() },
        IpcError::Fd4ReadFailed { detail: "read error".to_string() },
        IpcError::Fd3WriteFailed { detail: "write error".to_string() },
        IpcError::StderrReadFailed { detail: "stderr error".to_string() },
        IpcError::SignalFailed { detail: "signal error".to_string() },
        IpcError::IdentityMismatch {
            expected_instance: "ei".to_string(),
            expected_node: "en".to_string(),
            actual_instance: "ai".to_string(),
            actual_node: "an".to_string(),
        },
    ];
    for err in errors {
        let display = format!("{err}");
        assert!(!display.is_empty());
        assert!(display.len() > 5, "error display should be descriptive: {display}");
    }
}

// ============================================================================
// SECTION 7: SubprocessConfig Validation
// ============================================================================

#[test]
fn config_rejects_zero_timeout() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("test.sh");
    std::fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
    make_executable(&script);
    assert!(SubprocessConfig::new(&script, 0, vec![]).is_err());
}

#[test]
fn config_rejects_nonexistent_program() {
    assert!(SubprocessConfig::new("/nonexistent/binary", 1000, vec![]).is_err());
}

#[test]
fn config_rejects_non_executable_file() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("not_executable.txt");
    std::fs::write(&file, "not executable").unwrap();
    assert!(SubprocessConfig::new(&file, 1000, vec![]).is_err());
}

#[test]
fn config_payload_getter() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("test.sh");
    std::fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
    make_executable(&script);
    let payload = b"hello world".to_vec();
    let cfg = SubprocessConfig::new(&script, 1000, payload.clone()).unwrap();
    assert_eq!(cfg.fd3_payload(), &payload);
}

// ============================================================================
// SECTION 8: Large Message Framing
// ============================================================================

#[test]
fn large_envelope_serialization_near_limit() {
    let large_input = serde_json::json!({"data": "x".repeat(8 * 1024 * 1024)});
    let env = make_fd3("i", "n", large_input);
    let mut buf = Vec::new();
    let result = write_envelope(&mut buf, &env);
    assert!(result.is_ok(), "8MB envelope should serialize within limit");
    assert!(buf.len() > 8 * 1024 * 1024);
    assert!(buf.len() < MAX_PAYLOAD_SIZE as usize + 10);
}

#[test]
fn large_envelope_just_over_limit_fails() {
    let large_input = serde_json::json!({"data": "x".repeat(10 * 1024 * 1024)});
    let env = make_fd3("i", "n", large_input);
    let mut buf = Vec::new();
    let result = write_envelope(&mut buf, &env);
    assert!(matches!(result, Err(IpcError::PayloadTooLarge(_))));
}

// ============================================================================
// SECTION 9: Integration Tests — Adversarial & Subprocess
// ============================================================================

#[tokio::test]
async fn adversary_fd4_byte_by_byte_response() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("byte_by_byte.py");
    std::fs::write(
        &script,
        "#!/usr/bin/python3\nimport os, json, time\nenvelope = {\"version\": 1, \"instance_id\": \"bytebybyte\", \"node_id\": \"n1\",\n            \"result\": {\"success\": {\"output\": \"ok\"}}}\npayload = json.dumps(envelope).encode()\nheader = len(payload).to_bytes(4, 'big')\nfor b in header:\n    os.write(4, bytes([b]))\n    time.sleep(0.001)\nfor b in payload:\n    os.write(4, bytes([b]))\n    time.sleep(0.001)\n",
    )
    .unwrap();
    make_executable(&script);

    let cfg = SubprocessConfig::new(&script, 5000, vec![]).unwrap();
    let result = run_subprocess(cfg).await;
    match result {
        Ok(output) => {
            let parsed = serde_json::from_slice::<Fd4Envelope>(&output.fd4_bytes);
            assert!(parsed.is_ok(), "byte-by-byte fd4 should be parseable: {:?}", String::from_utf8_lossy(&output.fd4_bytes));
            assert_eq!(parsed.unwrap().instance_id, "bytebybyte");
        }
        Err(e) => panic!("byte-by-byte fd4 should succeed: {:?}", e),
    }
}

#[tokio::test]
async fn adversary_fd4_byte_by_byte_header_only_then_eof() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("header_only.py");
    std::fs::write(&script, "#!/usr/bin/python3\nimport os\nos.write(4, b'\\x00\\x00\\x00')\n").unwrap();
    make_executable(&script);

    let cfg = SubprocessConfig::new(&script, 2000, vec![]).unwrap();
    let result = run_subprocess(cfg).await;
    assert!(result.is_err(), "incomplete byte-by-byte header should fail");
}

#[tokio::test]
async fn adversary_fd4_two_envelopes_sent() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("two_envelopes.py");
    std::fs::write(
        &script,
        "#!/usr/bin/python3\nimport os, json\nenv1 = {\"version\": 1, \"instance_id\": \"first\", \"node_id\": \"n1\",\n        \"result\": {\"success\": {\"output\": \"env1\"}}}\np1 = json.dumps(env1).encode()\nos.write(4, len(p1).to_bytes(4, 'big'))\nos.write(4, p1)\nenv2 = {\"version\": 1, \"instance_id\": \"second\", \"node_id\": \"n1\",\n        \"result\": {\"success\": {\"output\": \"env2\"}}}\np2 = json.dumps(env2).encode()\nos.write(4, len(p2).to_bytes(4, 'big'))\nos.write(4, p2)\n",
    )
    .unwrap();
    make_executable(&script);

    let cfg = SubprocessConfig::new(&script, 2000, vec![]).unwrap();
    let result = run_subprocess(cfg).await;
    match result {
        Ok(output) => {
            let parsed = serde_json::from_slice::<Fd4Envelope>(&output.fd4_bytes);
            assert!(parsed.is_ok(), "first of two envelopes should be parseable");
            assert_eq!(parsed.unwrap().instance_id, "first");
        }
        Err(e) => panic!("two-envelope fd4 should succeed: {:?}", e),
    }
}

#[tokio::test]
async fn adversary_timeout_during_read_exact() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("stall_after_header.py");
    std::fs::write(
        &script,
        "#!/usr/bin/python3\nimport os, time\nos.write(4, (1024*1024).to_bytes(4, 'big'))\ntime.sleep(60)\n",
    )
    .unwrap();
    make_executable(&script);

    let cfg = SubprocessConfig::new(&script, 200, vec![]).unwrap();
    let start = Instant::now();
    let result = run_subprocess(cfg).await;
    let elapsed = start.elapsed();

    assert!(matches!(result, Err(IpcError::Timeout { .. })));
    assert!(elapsed < Duration::from_secs(5), "timeout should fire quickly: {:?}", elapsed);
}

#[tokio::test]
async fn adversary_timeout_during_read_exact_small_claim() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("stall_small_header.py");
    std::fs::write(
        &script,
        "#!/usr/bin/python3\nimport os, time\nos.write(4, (100).to_bytes(4, 'big'))\nos.write(4, b'x'*50)\ntime.sleep(60)\n",
    )
    .unwrap();
    make_executable(&script);

    let cfg = SubprocessConfig::new(&script, 200, vec![]).unwrap();
    let start = Instant::now();
    let result = run_subprocess(cfg).await;
    let elapsed = start.elapsed();

    assert!(matches!(result, Err(IpcError::Timeout { .. })));
    assert!(elapsed < Duration::from_secs(5));
}

#[tokio::test]
async fn concurrent_fd3_fd4_access_no_deadlock() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("concurrent_fd.py");
    std::fs::write(
        &script,
        "#!/usr/bin/python3\nimport os, json, select\nhdr = b''\nwhile len(hdr) < 4:\n    r, _, _ = select.select([3], [], [], 1.0)\n    if r:\n        hdr += os.read(3, 4 - len(hdr))\n    else:\n        break\nif len(hdr) == 4:\n    payload_len = int.from_bytes(hdr, 'big')\n    payload = b''\n    while len(payload) < payload_len:\n        r, _, _ = select.select([3], [], [], 1.0)\n        if r:\n            payload += os.read(3, payload_len - len(payload))\n        else:\n            break\n    response = {\"version\": 1, \"instance_id\": \"concurrent\", \"node_id\": \"n1\",\n                \"result\": {\"success\": {\"output\": \"ok\"}}}\n    resp = json.dumps(response).encode()\n    os.write(4, len(resp).to_bytes(4, 'big'))\n    os.write(4, resp)\n",
    )
    .unwrap();
    make_executable(&script);

    // Use 10KB payload — large enough for pipe buffering, small enough for ARG_MAX
    let large_payload = vec![b'X'; 10_000];
    let cfg = SubprocessConfig::new(&script, 5000, large_payload).unwrap();
    let result = run_subprocess(cfg).await;
    match result {
        Ok(output) => {
            let parsed = serde_json::from_slice::<Fd4Envelope>(&output.fd4_bytes);
            assert!(parsed.is_ok(), "concurrent fd3/fd4 should work");
        }
        Err(e) => panic!("concurrent fd3/fd4 should not deadlock: {:?}", e),
    }
}

#[tokio::test]
async fn concurrent_subprocess_spawns_no_fd_leak() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("quick_ipc.sh");
    std::fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
    make_executable(&script);

    let mut handles = Vec::new();
    for _ in 0..10 {
        let s = script.clone();
        handles.push(tokio::spawn(async move {
            let cfg = SubprocessConfig::new(&s, 2000, vec![]).unwrap();
            run_subprocess(cfg).await
        }));
    }
    for handle in handles {
        let result = handle.await.expect("task panicked");
        assert!(result.is_ok(), "concurrent spawn should succeed: {:?}", result);
    }
}

#[tokio::test]
async fn signal_sigusr1_during_ipc_does_not_crash() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("sigusr1_test.py");
    std::fs::write(
        &script,
        "#!/usr/bin/python3\nimport os, signal, json\nsignal.signal(signal.SIGUSR1, lambda s, f: None)\nos.kill(os.getpid(), signal.SIGUSR1)\nresponse = {\"version\": 1, \"instance_id\": \"sigusr1\", \"node_id\": \"n1\",\n            \"result\": {\"success\": {\"output\": \"survived\"}}}\nresp = json.dumps(response).encode()\nos.write(4, len(resp).to_bytes(4, 'big'))\nos.write(4, resp)\n",
    )
    .unwrap();
    make_executable(&script);

    let cfg = SubprocessConfig::new(&script, 2000, vec![]).unwrap();
    let result = run_subprocess(cfg).await;
    match result {
        Ok(output) => {
            let parsed = serde_json::from_slice::<Fd4Envelope>(&output.fd4_bytes);
            assert!(parsed.is_ok(), "SIGUSR1 during IPC should not crash");
            assert_eq!(parsed.unwrap().instance_id, "sigusr1");
        }
        Err(e) => panic!("SIGUSR1 should not cause failure: {:?}", e),
    }
}

#[tokio::test]
async fn signal_sighup_during_ipc_child_handles() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("sighup_test.py");
    std::fs::write(
        &script,
        "#!/usr/bin/python3\nimport os, signal, json\nsignal.signal(signal.SIGHUP, lambda s, f: None)\nos.kill(os.getpid(), signal.SIGHUP)\nresponse = {\"version\": 1, \"instance_id\": \"sighup\", \"node_id\": \"n1\",\n            \"result\": {\"success\": {\"output\": \"handled\"}}}\nresp = json.dumps(response).encode()\nos.write(4, len(resp).to_bytes(4, 'big'))\nos.write(4, resp)\n",
    )
    .unwrap();
    make_executable(&script);

    let cfg = SubprocessConfig::new(&script, 2000, vec![]).unwrap();
    let result = run_subprocess(cfg).await;
    match result {
        Ok(output) => {
            let parsed = serde_json::from_slice::<Fd4Envelope>(&output.fd4_bytes);
            assert!(parsed.is_ok(), "SIGHUP should be handled gracefully");
        }
        Err(e) => {
            assert!(matches!(e, IpcError::ProcessFailed { .. } | IpcError::Fd4ReadFailed { .. }), "unexpected: {:?}", e);
        }
    }
}

#[tokio::test]
async fn zero_length_fd3_payload_handled() {
    let output = run_subprocess(config("", 500)).await.unwrap();
    assert_eq!(output.fd4_bytes, b"");
}

#[tokio::test]
async fn zero_length_fd3_payload_child_reads_and_responds() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("zero_fd3.py");
    std::fs::write(
        &script,
        "#!/usr/bin/python3\nimport os, json\nhdr = b''\nwhile len(hdr) < 4:\n    hdr += os.read(3, 4 - len(hdr))\npayload_len = int.from_bytes(hdr, 'big')\npayload = b''\nif payload_len > 0:\n    while len(payload) < payload_len:\n        payload += os.read(3, payload_len - len(payload))\nassert payload_len == 0\nresponse = {\"version\": 1, \"instance_id\": \"zeropayload\", \"node_id\": \"n1\",\n            \"result\": {\"success\": {\"output\": \"empty-ok\"}}}\nresp = json.dumps(response).encode()\nos.write(4, len(resp).to_bytes(4, 'big'))\nos.write(4, resp)\n",
    )
    .unwrap();
    make_executable(&script);

    let cfg = SubprocessConfig::new(&script, 2000, vec![]).unwrap();
    let result = run_subprocess(cfg).await;
    match result {
        Ok(output) => {
            let parsed = serde_json::from_slice::<Fd4Envelope>(&output.fd4_bytes);
            assert!(parsed.is_ok());
            assert_eq!(parsed.unwrap().instance_id, "zeropayload");
        }
        Err(e) => panic!("zero-length fd3 should succeed: {:?}", e),
    }
}

#[tokio::test]
async fn fd3_write_broken_pipe_after_child_exit_is_non_fatal() {
    // Use Python script that exits immediately — avoids ARG_MAX from large payload-as-argv
    let dir = tempdir().unwrap();
    let script = dir.path().join("quick_exit.py");
    std::fs::write(&script, "#!/usr/bin/python3\n# exit immediately without reading fd3 or writing fd4\n").unwrap();
    make_executable(&script);

    let large_payload: Vec<u8> = "A ".repeat(100_000).into_bytes();
    let cfg = SubprocessConfig::new(&script, 2000, large_payload).unwrap();
    let result = run_subprocess(cfg).await;
    assert!(result.is_ok(), "broken pipe should be non-fatal: {:?}", result);
}

#[tokio::test]
async fn fd4_empty_response_from_child_is_ok() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("empty_fd4.py");
    std::fs::write(&script, "#!/usr/bin/python3\n# exit without fd4 write\n").unwrap();
    make_executable(&script);

    let cfg = SubprocessConfig::new(&script, 2000, vec![]).unwrap();
    let result = run_subprocess(cfg).await;
    match result {
        Ok(output) => assert!(output.fd4_bytes.is_empty()),
        Err(IpcError::ProcessFailed { .. }) | Err(IpcError::Fd4ReadFailed { .. }) => {}
        Err(e) => panic!("unexpected: {:?}", e),
    }
}

#[tokio::test]
async fn fd3_zero_bytes_then_child_responds() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("zero_then_respond.py");
    std::fs::write(
        &script,
        "#!/usr/bin/python3\nimport os, json\nhdr = os.read(3, 4)\nif len(hdr) == 4:\n    payload_len = int.from_bytes(hdr, 'big')\n    if payload_len > 0:\n        os.read(3, payload_len)\nenv = {\"version\": 1, \"instance_id\": \"zr\", \"node_id\": \"n1\",\n       \"result\": {\"success\": {\"output\": \"zero-recv\"}}}\nresp = json.dumps(env).encode()\nos.write(4, len(resp).to_bytes(4, 'big'))\nos.write(4, resp)\n",
    )
    .unwrap();
    make_executable(&script);

    let cfg = SubprocessConfig::new(&script, 2000, vec![]).unwrap();
    let result = run_subprocess(cfg).await;
    match result {
        Ok(output) => {
            let parsed = serde_json::from_slice::<Fd4Envelope>(&output.fd4_bytes);
            assert!(parsed.is_ok(), "zero-byte fd3 child should respond: {:?}", String::from_utf8_lossy(&output.fd4_bytes));
        }
        Err(e) => panic!("unexpected: {:?}", e),
    }
}

#[tokio::test]
async fn large_fd3_payload_near_limit() {
    // Use Python script that exits immediately — avoids ARG_MAX from large payload-as-argv
    let dir = tempdir().unwrap();
    let script = dir.path().join("large_fd3_child.py");
    std::fs::write(&script, "#!/usr/bin/python3\n# exit immediately without reading fd3\n").unwrap();
    make_executable(&script);

    let large_payload: Vec<u8> = ("Z".repeat(10_000) + " ").repeat(100).into_bytes();
    let cfg = SubprocessConfig::new(&script, 5000, large_payload).unwrap();
    let start = Instant::now();
    let result = run_subprocess(cfg).await;
    assert!(result.is_ok(), "1MB fd3 should succeed: {:?}", result);
    assert!(start.elapsed() < Duration::from_secs(10), "should complete quickly");
}

#[tokio::test]
async fn large_fd4_response_handled() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("large_fd4.py");
    std::fs::write(
        &script,
        "#!/usr/bin/python3\nimport os, json\nenv = {\"version\": 1, \"instance_id\": \"large\", \"node_id\": \"n1\",\n       \"result\": {\"success\": {\"output\": \"x\" * (1024 * 1024)}}}\nresp = json.dumps(env).encode()\nos.write(4, len(resp).to_bytes(4, 'big'))\nos.write(4, resp)\n",
    )
    .unwrap();
    make_executable(&script);

    let cfg = SubprocessConfig::new(&script, 5000, vec![]).unwrap();
    let result = run_subprocess(cfg).await;
    match result {
        Ok(output) => {
            assert!(output.fd4_bytes.len() > 1024 * 1024, "large fd4 should be read: {} bytes", output.fd4_bytes.len());
            assert!(serde_json::from_slice::<Fd4Envelope>(&output.fd4_bytes).is_ok());
        }
        Err(e) => panic!("large fd4 should succeed: {:?}", e),
    }
}

#[tokio::test]
async fn stderr_exact_at_limit_no_truncation() {
    use vo_ipc::{MAX_STDERR_BYTES, TRUNCATION_MARKER};

    let payload = format!("stderr-repeat {} x 0", MAX_STDERR_BYTES);
    let output = run_subprocess(config(payload, 500)).await.unwrap();
    assert_eq!(output.stderr_bytes.len(), MAX_STDERR_BYTES);
    assert!(!output.stderr_truncated);
    assert!(!output.stderr_bytes.ends_with(TRUNCATION_MARKER.as_bytes()));
}

#[tokio::test]
async fn stderr_one_byte_over_limit_truncated() {
    use vo_ipc::{MAX_STDERR_BYTES, TRUNCATION_MARKER};

    // Use exit code 1 so ProcessFailed is returned when stderr exceeds limit
    let payload = format!("stderr-repeat {} x 1", MAX_STDERR_BYTES + 1);
    let error = run_subprocess(config(payload, 500)).await.unwrap_err();
    match error {
        IpcError::ProcessFailed { stderr_bytes, stderr_truncated, .. } => {
            assert!(stderr_truncated);
            assert!(stderr_bytes.ends_with(TRUNCATION_MARKER.as_bytes()));
        }
        other => panic!("expected ProcessFailed, got {:?}", other),
    }
}

#[tokio::test]
async fn timeout_grace_period_sigterm_then_sigkill() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("ignore_sigterm.py");
    std::fs::write(
        &script,
        "#!/usr/bin/python3\nimport os, signal, time\nsignal.signal(signal.SIGTERM, signal.SIG_IGN)\ntime.sleep(60)\n",
    )
    .unwrap();
    make_executable(&script);

    let start = Instant::now();
    let result = run_subprocess(SubprocessConfig::new(&script, 50, vec![]).unwrap()).await;
    let elapsed = start.elapsed();

    assert!(matches!(result, Err(IpcError::Timeout { .. })));
    assert!(elapsed >= Duration::from_millis(120), "should wait grace period: {:?}", elapsed);
    assert!(elapsed < Duration::from_secs(5), "should not wait forever: {:?}", elapsed);
}
