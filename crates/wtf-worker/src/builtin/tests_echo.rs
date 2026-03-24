use bytes::Bytes;

use super::echo_handler;
use super::test_helpers::make_task;

// ── Happy Path ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_echo_returns_payload_unchanged_for_ascii_bytes() {
    let task = make_task("echo", b"hello world");
    let result = echo_handler(task).await;
    assert_eq!(result, Ok(Bytes::from_static(b"hello world")));
}

#[tokio::test]
async fn test_echo_returns_payload_unchanged_for_binary_bytes() {
    let task = make_task("echo", b"\x00\x01\x02\xff\xfe");
    let result = echo_handler(task).await;
    assert_eq!(result, Ok(Bytes::from_static(b"\x00\x01\x02\xff\xfe")));
}

#[tokio::test]
async fn test_echo_returns_payload_unchanged_for_empty_payload() {
    let task = make_task("echo", b"");
    let result = echo_handler(task).await;
    assert_eq!(result, Ok(Bytes::new()));
}

// ── Edge Cases ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_echo_preserves_large_payload_1mb() {
    let large = vec![0xAB_u8; 1_048_576]; // 1 MB
    let task = make_task("echo", &large);
    let result = echo_handler(task).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 1_048_576);
}

#[tokio::test]
async fn test_echo_preserves_null_bytes_in_payload() {
    let task = make_task("echo", b"\x00\x00\x00");
    let result = echo_handler(task).await;
    assert_eq!(result, Ok(Bytes::from_static(b"\x00\x00\x00")));
}

// ── Invariant ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_invariant_echo_handler_never_panics() {
    let payloads: &[&[u8]] = &[
        b"",
        b"\x00\xff",
        b"null",
        br#"{"key":"value"}"#,
        &[0; 1024],
    ];
    for payload in payloads {
        let task = make_task("echo", payload);
        let result = tokio::spawn(echo_handler(task)).await;
        assert!(result.is_ok(), "echo should not panic for any payload");
        assert!(result.unwrap().is_ok(), "echo should always return Ok");
    }
}
