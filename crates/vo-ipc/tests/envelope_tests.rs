use rstest::rstest;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Cursor;
use vo_ipc::*;

#[test]
fn fd3_envelope_roundtrip_is_idempotent() {
    // Given
    let mut secrets = BTreeMap::new();
    secrets.insert("api_key".to_string(), "supersecret".to_string());

    let env = Fd3Envelope {
        version: 1,
        instance_id: "inst1".to_string(),
        node_id: "node1".to_string(),
        input: serde_json::json!({"foo": "bar"}),
        secrets,
        metadata: BTreeMap::new(),
    };

    let mut buffer = Vec::new();

    // When
    write_envelope(&mut buffer, &env).unwrap();
    let mut reader = Cursor::new(buffer);
    let decoded: Fd3Envelope = read_envelope(&mut reader).unwrap();

    // Then
    assert_eq!(env, decoded);
}

#[test]
fn fd4_success_roundtrip_is_idempotent() {
    // Given
    let env = Fd4Envelope {
        version: 1,
        instance_id: "inst1".to_string(),
        node_id: "node1".to_string(),
        result: TaskResult::Success {
            output: serde_json::json!({"result": 42}),
        },
    };

    let mut buffer = Vec::new();

    // When
    write_envelope(&mut buffer, &env).unwrap();
    let mut reader = Cursor::new(buffer);
    let decoded: Fd4Envelope = read_envelope(&mut reader).unwrap();

    // Then
    assert_eq!(env, decoded);
}

#[test]
fn read_envelope_fails_at_one_byte_over_limit() {
    let len: u32 = 10_485_760 + 1;
    let mut buffer = len.to_be_bytes().to_vec();
    buffer.extend(vec![0u8; 10]);
    let mut reader = Cursor::new(buffer);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    match result {
        Err(IpcError::PayloadTooLarge(got)) => assert_eq!(got, len),
        other => panic!("Expected PayloadTooLarge, got {:?}", other),
    }
}

#[test]
fn read_envelope_succeeds_at_exactly_limit() {
    let limit = 10_485_760;
    let mut env = Fd3Envelope {
        version: 1,
        instance_id: "i".into(),
        node_id: "n".into(),
        input: serde_json::json!(null),
        secrets: BTreeMap::new(),
        metadata: BTreeMap::new(),
    };

    // We want to construct an envelope that is exactly 'limit' bytes when serialized.
    // The base JSON size (without padding) is:
    let _base_json = serde_json::to_vec(&env).unwrap();
    // We add a padding field. The field name and structure add some overhead.
    // "\"metadata\":{\"padding\":\"...\"}"
    // Let's just use a simple string for the input field.
    let overhead = 85; // Approximate overhead for the envelope structure with IDs
    let padding_size = limit as usize - overhead;
    env.input = serde_json::json!("x".repeat(padding_size));

    let mut json = serde_json::to_vec(&env).unwrap();
    // Adjust precisely
    if json.len() < limit as usize {
        let diff = limit as usize - json.len();
        env.input = serde_json::json!("x".repeat(padding_size + diff));
        json = serde_json::to_vec(&env).unwrap();
    } else if json.len() > limit as usize {
        let diff = json.len() - limit as usize;
        env.input = serde_json::json!("x".repeat(padding_size - diff));
        json = serde_json::to_vec(&env).unwrap();
    }

    assert_eq!(
        json.len(),
        limit as usize,
        "Failed to construct exact limit JSON"
    );

    let mut buffer = (json.len() as u32).to_be_bytes().to_vec();
    buffer.extend(json);
    let mut reader = Cursor::new(buffer);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    match result {
        Ok(got) => assert_eq!(got, env),
        Err(e) => panic!("Expected Ok, got {:?}", e),
    }
}

#[rstest]
#[case("", "instance_id cannot be empty")]
#[case("node_1", "node_id contains invalid characters")]
fn read_envelope_validates_ids(#[case] invalid_id: &str, #[case] expected_msg: &str) {
    // Given
    let env_json = serde_json::json!({
        "version": 1,
        "instance_id": if expected_msg.contains("instance") { invalid_id } else { "inst1" },
        "node_id": if expected_msg.contains("node") { invalid_id } else { "node1" },
        "input": {},
        "secrets": {},
        "metadata": {}
    });
    let payload = serde_json::to_vec(&env_json).unwrap();
    let mut buffer = (payload.len() as u32).to_be_bytes().to_vec();
    buffer.extend(payload);

    let mut reader = Cursor::new(buffer);

    // When
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);

    // Then
    match result {
        Err(IpcError::SchemaViolation(msg)) => assert!(
            msg.contains(expected_msg),
            "Expected message containing '{}', got '{}'",
            expected_msg,
            msg
        ),
        other => panic!("Expected SchemaViolation, got {:?}", other),
    }
}

#[test]
fn validate_identity_succeeds_on_match() {
    // Given
    let env = Fd4Envelope {
        version: 1,
        instance_id: "inst1".to_string(),
        node_id: "node1".to_string(),
        result: TaskResult::Success {
            output: serde_json::Value::Null,
        },
    };

    // When / Then
    validate_identity(&env, "inst1", "node1").expect("Should match");
}

#[test]
fn validate_identity_fails_on_mismatched_instance_id() {
    // Given
    let env = Fd4Envelope {
        version: 1,
        instance_id: "inst1".to_string(),
        node_id: "node1".to_string(),
        result: TaskResult::Success {
            output: serde_json::Value::Null,
        },
    };

    // When
    let result = validate_identity(&env, "inst2", "node1");

    // Then
    match result {
        Err(IpcError::IdentityMismatch {
            expected_instance, ..
        }) => {
            assert_eq!(expected_instance, "inst2");
        }
        other => panic!("Expected IdentityMismatch, got {:?}", other),
    }
}

#[test]
fn fd4_failure_roundtrip_is_idempotent() {
    // Given
    let env = Fd4Envelope {
        version: 1,
        instance_id: "inst1".to_string(),
        node_id: "node1".to_string(),
        result: TaskResult::Failure {
            error: TaskError {
                code: "ERR_TIMEOUT".into(),
                message: "Process timed out".into(),
                details: None,
            },
        },
    };

    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();
    let mut reader = Cursor::new(buffer);
    let decoded: Fd4Envelope = read_envelope(&mut reader).unwrap();

    assert_eq!(env, decoded);
}

#[test]
fn write_envelope_works_with_generic_struct() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Simple {
        a: i32,
    }
    let val = Simple { a: 42 };

    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &val).unwrap();

    assert_eq!(&buffer[..4], &[0, 0, 0, 8]); // {"a":42} is 8 bytes

    let mut reader = Cursor::new(buffer);
    let decoded: Simple = read_envelope(&mut reader).unwrap();
    assert_eq!(val, decoded);
}

#[test]
fn fd3_envelope_roundtrip_with_newlines_and_unicode() {
    let env = Fd3Envelope {
        version: 1,
        instance_id: "inst1".to_string(),
        node_id: "node1".to_string(),
        input: serde_json::json!({
            "text_with_newlines": "line1\nline2\nline3",
            "chinese": "中文测试",
            "emoji": "Hello 👋 World 🌍",
            "mixed": "Newlines\nAnd\tTabs\tAnd\r\nCRLF",
            "unicodeNormalization": "ÅÅÆØØ"
        }),
        secrets: {
            let mut m = BTreeMap::new();
            m.insert(
                "key_with_newlines".to_string(),
                "value\nwith\nnewlines".to_string(),
            );
            m
        },
        metadata: {
            let mut m = BTreeMap::new();
            m.insert(
                "trailing_newline".to_string(),
                "ends_with_newline\n".to_string(),
            );
            m.insert("chinese_key".to_string(), "值".to_string());
            m
        },
    };

    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();
    let mut reader = Cursor::new(buffer);
    let decoded: Fd3Envelope = read_envelope(&mut reader).unwrap();

    assert_eq!(env, decoded);
}

#[test]
fn fd4_envelope_roundtrip_with_embedded_newlines_in_result() {
    let env = Fd4Envelope {
        version: 1,
        instance_id: "inst1".to_string(),
        node_id: "node1".to_string(),
        result: TaskResult::Success {
            output: serde_json::json!({
                "log": "Step 1 completed\nStep 2 completed\nStep 3 failed\n",
                "multiline_string": "First line\nSecond line\nThird line",
                "unicode": "Émoji test: 🎭🎯🎲"
            }),
        },
    };

    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();
    let mut reader = Cursor::new(buffer);
    let decoded: Fd4Envelope = read_envelope(&mut reader).unwrap();

    assert_eq!(env, decoded);
}

#[test]
fn read_envelope_fails_on_zero_length_prefix() {
    let buffer = vec![0u8; 4];
    let mut reader = Cursor::new(buffer);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    assert!(matches!(result, Err(IpcError::InvalidJson(_))));
}

#[test]
fn read_envelope_fails_on_one_byte_payload() {
    let mut buffer = vec![0, 0, 0, 1];
    buffer.push(b'{');
    let mut reader = Cursor::new(buffer);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    assert!(matches!(result, Err(IpcError::InvalidJson(_))));
}

#[test]
fn read_envelope_fails_on_truncated_length_prefix() {
    let buffer = vec![0, 0, 0];
    let mut reader = Cursor::new(buffer);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    match result {
        Err(IpcError::IncompleteRead {
            expected: 4,
            actual: 3,
        }) => (),
        other => panic!("Expected IncompleteRead(4, 3), got {:?}", other),
    }
}

#[test]
fn read_envelope_fails_on_truncated_payload_body() {
    let mut buffer = vec![0, 0, 0, 10];
    buffer.extend(vec![b'{'; 5]);
    let mut reader = Cursor::new(buffer);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    match result {
        Err(IpcError::IncompleteRead {
            expected: 10,
            actual: 5,
        }) => (),
        other => panic!("Expected IncompleteRead(10, 5), got {:?}", other),
    }
}

#[test]
fn read_envelope_fails_on_invalid_utf8_payload() {
    let payload = vec![0xff, 0xfe];
    let mut buffer = (payload.len() as u32).to_be_bytes().to_vec();
    buffer.extend(payload);
    let mut reader = Cursor::new(buffer);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    assert!(matches!(result, Err(IpcError::InvalidJson(_))));
}

#[test]
fn read_envelope_fails_on_unsupported_version() {
    let env_json = serde_json::json!({
        "version": 2,
        "instance_id": "inst1",
        "node_id": "node1",
        "input": {},
        "secrets": {},
        "metadata": {}
    });
    let payload = serde_json::to_vec(&env_json).unwrap();
    let mut buffer = (payload.len() as u32).to_be_bytes().to_vec();
    buffer.extend(payload);
    let mut reader = Cursor::new(buffer);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    assert!(matches!(result, Err(IpcError::VersionMismatch(2))));
}

#[test]
fn read_envelope_fails_on_missing_version_field() {
    let env_json = serde_json::json!({
        "instance_id": "inst1",
        "node_id": "node1",
        "input": {},
        "secrets": {},
        "metadata": {}
    });
    let payload = serde_json::to_vec(&env_json).unwrap();
    let mut buffer = (payload.len() as u32).to_be_bytes().to_vec();
    buffer.extend(payload);
    let mut reader = Cursor::new(buffer);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
    // serde returns missing field error which we map to SchemaViolation
    assert!(matches!(result, Err(IpcError::SchemaViolation(_))));
}

#[test]
fn read_envelope_succeeds_with_single_character_ids() {
    let env = Fd3Envelope {
        version: 1,
        instance_id: "a".into(),
        node_id: "1".into(),
        input: serde_json::json!({}),
        secrets: BTreeMap::new(),
        metadata: BTreeMap::new(),
    };
    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();
    let mut reader = Cursor::new(buffer);
    let decoded: Fd3Envelope = read_envelope(&mut reader).unwrap();
    assert_eq!(env, decoded);
}

#[test]
fn write_envelope_fails_when_payload_exceeds_limit() {
    // We need a large struct to exceed 10MB
    let large_input = vec![b'x'; 11 * 1024 * 1024];
    let env = Fd3Envelope {
        version: 1,
        instance_id: "i".into(),
        node_id: "n".into(),
        input: serde_json::json!(large_input),
        secrets: BTreeMap::new(),
        metadata: BTreeMap::new(),
    };
    let mut buffer = Vec::new();
    let result = write_envelope(&mut buffer, &env);
    assert!(matches!(result, Err(IpcError::PayloadTooLarge(_))));
}

#[test]
fn write_envelope_fails_with_10mb_payload() {
    let large_input = vec![b'x'; 10 * 1024 * 1024];
    let env = Fd3Envelope {
        version: 1,
        instance_id: "i".into(),
        node_id: "n".into(),
        input: serde_json::json!(large_input),
        secrets: BTreeMap::new(),
        metadata: BTreeMap::new(),
    };
    let mut buffer = Vec::new();
    let result = write_envelope(&mut buffer, &env);
    match result {
        Err(IpcError::PayloadTooLarge(size)) => {
            assert!(size > 0, "PayloadTooLarge should contain actual size")
        }
        other => panic!("Expected PayloadTooLarge, got {:?}", other),
    }
}

#[test]
fn engine_receive_envelope_succeeds_on_identity_match() {
    let env = Fd4Envelope {
        version: 1,
        instance_id: "inst1".into(),
        node_id: "node1".into(),
        result: TaskResult::Success {
            output: serde_json::json!({}),
        },
    };
    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();

    let mut reader = Cursor::new(buffer);
    let result = engine_receive_envelope(&mut reader, "inst1", "node1");
    match result {
        Ok(got) => assert_eq!(got, env),
        Err(e) => panic!("Expected Ok, got {:?}", e),
    }
}

#[test]
fn engine_receive_envelope_fails_on_identity_mismatch() {
    let env = Fd4Envelope {
        version: 1,
        instance_id: "inst1".into(),
        node_id: "node1".into(),
        result: TaskResult::Success {
            output: serde_json::json!({}),
        },
    };
    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();

    let mut reader = Cursor::new(buffer);
    let result = engine_receive_envelope(&mut reader, "inst2", "node1");
    assert!(matches!(result, Err(IpcError::IdentityMismatch { .. })));
}

struct PartialReadCursor {
    data: Vec<u8>,
    chunk_size: usize,
    offset: usize,
}

impl PartialReadCursor {
    fn new(data: Vec<u8>, chunk_size: usize) -> Self {
        Self {
            data,
            chunk_size,
            offset: 0,
        }
    }
}

impl std::io::Read for PartialReadCursor {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.offset >= self.data.len() {
            return Ok(0);
        }
        let remaining = self.data.len() - self.offset;
        let to_read = remaining.min(self.chunk_size).min(buf.len());
        buf[..to_read].copy_from_slice(&self.data[self.offset..self.offset + to_read]);
        self.offset += to_read;
        Ok(to_read)
    }
}

#[test]
fn read_envelope_handles_partial_header_reads() {
    let env = Fd4Envelope {
        version: 1,
        instance_id: "inst1".into(),
        node_id: "node1".into(),
        result: TaskResult::Success {
            output: serde_json::json!({"result": 42}),
        },
    };

    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();
    let original_len = buffer.len();

    let mut reader = PartialReadCursor::new(buffer, 4);
    let result: Result<Fd4Envelope, IpcError> = read_envelope(&mut reader);
    match result {
        Ok(got) => assert_eq!(env, got),
        Err(e) => panic!(
            "Expected Ok (buffer was {} bytes), got {:?}",
            original_len, e
        ),
    }
}

#[test]
fn read_envelope_handles_partial_payload_reads() {
    let env = Fd4Envelope {
        version: 1,
        instance_id: "inst1".into(),
        node_id: "node1".into(),
        result: TaskResult::Success {
            output: serde_json::json!({"result": 42}),
        },
    };

    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();

    let mut reader = PartialReadCursor::new(buffer, 100);
    let result: Result<Fd4Envelope, IpcError> = read_envelope(&mut reader);
    match result {
        Ok(got) => assert_eq!(env, got),
        Err(e) => panic!("Expected Ok, got {:?}", e),
    }
}

#[test]
fn read_envelope_returns_incomplete_read_on_early_eof() {
    let env = Fd4Envelope {
        version: 1,
        instance_id: "inst1".into(),
        node_id: "node1".into(),
        result: TaskResult::Success {
            output: serde_json::json!({"result": 42}),
        },
    };

    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();
    buffer.truncate(50);

    let mut reader = PartialReadCursor::new(buffer, 4);
    let result: Result<Fd4Envelope, IpcError> = read_envelope(&mut reader);
    assert!(matches!(result, Err(IpcError::IncompleteRead { .. })));
}
