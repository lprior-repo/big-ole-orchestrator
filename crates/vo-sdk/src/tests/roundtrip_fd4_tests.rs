use std::io::Cursor;

use vo_ipc::{write_envelope, Fd4Envelope, TaskResult};

fn round_trip_via_fd4_envelope(instance_id: &str, node_id: &str, payload: serde_json::Value) {
    let envelope = Fd4Envelope {
        version: 1,
        instance_id: instance_id.to_string(),
        node_id: node_id.to_string(),
        result: TaskResult::Success {
            output: payload.clone(),
        },
    };

    let mut buf = Vec::new();
    write_envelope(&mut buf, &envelope).expect("write should succeed");

    let mut cursor = Cursor::new(buf);
    let decoded: Fd4Envelope = vo_ipc::read_envelope(&mut cursor).expect("read should succeed");

    assert_eq!(decoded.version, 1);
    assert_eq!(decoded.instance_id, instance_id);
    assert_eq!(decoded.node_id, node_id);
    assert_eq!(
        decoded.result,
        TaskResult::Success { output: payload },
        "output must match"
    );
}

#[test]
fn sdk_fd4_write_engine_read_roundtrip_empty() {
    round_trip_via_fd4_envelope("testinstance", "testnode", serde_json::json!(null));
}

#[test]
fn sdk_fd4_write_engine_read_roundtrip_empty_object() {
    round_trip_via_fd4_envelope("testinstance", "testnode", serde_json::json!({}));
}

#[test]
fn sdk_fd4_write_engine_read_roundtrip_empty_array() {
    round_trip_via_fd4_envelope("testinstance", "testnode", serde_json::json!([]));
}

#[test]
fn sdk_fd4_write_engine_read_roundtrip_simple_string() {
    round_trip_via_fd4_envelope("testinstance", "testnode", serde_json::json!("hello world"));
}

#[test]
fn sdk_fd4_write_engine_read_roundtrip_simple_number() {
    round_trip_via_fd4_envelope("testinstance", "testnode", serde_json::json!(42));
}

#[test]
fn sdk_fd4_write_engine_read_roundtrip_simple_bool() {
    round_trip_via_fd4_envelope("testinstance", "testnode", serde_json::json!(true));
}

#[test]
fn sdk_fd4_write_engine_read_roundtrip_simple_null() {
    round_trip_via_fd4_envelope("testinstance", "testnode", serde_json::json!(null));
}

#[test]
fn sdk_fd4_write_engine_read_roundtrip_nested_object() {
    round_trip_via_fd4_envelope(
        "testinstance",
        "testnode",
        serde_json::json!({
            "user": {"id": 1, "name": "Alice"},
            "items": [{"sku": "abc", "qty": 2}]
        }),
    );
}

#[test]
fn sdk_fd4_write_engine_read_roundtrip_array_with_mixed_types() {
    round_trip_via_fd4_envelope(
        "testinstance",
        "testnode",
        serde_json::json!([1, "two", true, null, {"key": "value"}]),
    );
}

#[test]
fn sdk_fd4_write_engine_read_roundtrip_large_payload_1kb() {
    let large_string = "x".repeat(1024);
    round_trip_via_fd4_envelope("testinstance", "testnode", serde_json::json!(large_string));
}

#[test]
fn sdk_fd4_write_engine_read_roundtrip_large_payload_64kb() {
    let large_string = "x".repeat(64 * 1024);
    round_trip_via_fd4_envelope("testinstance", "testnode", serde_json::json!(large_string));
}

#[test]
fn sdk_fd4_write_engine_read_roundtrip_unicode() {
    round_trip_via_fd4_envelope(
        "testinstance",
        "testnode",
        serde_json::json!("日本語テスト"),
    );
}

#[test]
fn sdk_fd4_write_engine_read_roundtrip_unicode_in_object() {
    round_trip_via_fd4_envelope(
        "testinstance",
        "testnode",
        serde_json::json!({
            "name": "日本語",
            "emoji": "😀"
        }),
    );
}

#[test]
fn sdk_fd4_write_engine_read_roundtrip_special_chars() {
    round_trip_via_fd4_envelope(
        "testinstance",
        "testnode",
        serde_json::json!("line\nbreak\ttab\\backslash\"quote"),
    );
}

#[test]
fn sdk_fd4_write_engine_read_with_instance_and_node_ids() {
    round_trip_via_fd4_envelope("inst123", "node456", serde_json::json!({"result": 42}));
}

#[test]
fn sdk_fd4_write_engine_read_with_alphanumeric_ids() {
    round_trip_via_fd4_envelope("testinstance01", "nodeabc123", serde_json::json!([1, 2, 3]));
}
