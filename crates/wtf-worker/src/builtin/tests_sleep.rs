use std::time::Duration;

use tokio::time;

use super::{parse_sleep_ms, sleep_handler, SLEPT_RESULT};
use super::test_helpers::make_task;

// ── Happy Path ───────────────────────────────────────────────────────────────

#[tokio::test(start_paused = true)]
async fn test_sleep_returns_ok_slept_after_duration_10ms() {
    let task = make_task("sleep", br#"{"ms":10}"#);
    let join = tokio::spawn(sleep_handler(task));
    time::advance(Duration::from_millis(10)).await;
    let result = join.await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Ok(SLEPT_RESULT.clone()));
}

#[tokio::test]
async fn test_sleep_returns_ok_slept_after_duration_0ms() {
    let task = make_task("sleep", br#"{"ms":0}"#);
    let result = sleep_handler(task).await;
    assert_eq!(result, Ok(SLEPT_RESULT.clone()));
}

// ── Error Path ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_sleep_rejects_non_utf8_payload() {
    let task = make_task("sleep", b"\xff\xfe");
    let result = sleep_handler(task).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("invalid payload"), "got: {err}");
}

#[tokio::test]
async fn test_sleep_rejects_invalid_json_payload() {
    let task = make_task("sleep", b"not json");
    let result = sleep_handler(task).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("invalid payload"), "got: {err}");
}

#[tokio::test]
async fn test_sleep_rejects_json_without_ms_field() {
    let task = make_task("sleep", br#"{"other":42}"#);
    let result = sleep_handler(task).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("invalid payload"), "got: {err}");
}

#[tokio::test]
async fn test_sleep_rejects_ms_field_with_string_value() {
    let task = make_task("sleep", br#"{"ms":"fast"}"#);
    let result = sleep_handler(task).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("invalid payload"), "got: {err}");
}

#[tokio::test]
async fn test_sleep_rejects_ms_field_with_float_value() {
    let task = make_task("sleep", br#"{"ms":10.5}"#);
    let result = sleep_handler(task).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("invalid payload"), "got: {err}");
}

#[tokio::test]
async fn test_sleep_rejects_ms_field_with_negative_number() {
    let task = make_task("sleep", br#"{"ms":-1}"#);
    let result = sleep_handler(task).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("invalid payload"), "got: {err}");
}

#[tokio::test]
async fn test_sleep_rejects_ms_field_with_nested_object() {
    let task = make_task("sleep", br#"{"ms":{"nested":true}}"#);
    let result = sleep_handler(task).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("invalid payload"), "got: {err}");
}

#[tokio::test]
async fn test_sleep_rejects_empty_payload() {
    let task = make_task("sleep", b"");
    let result = sleep_handler(task).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("invalid payload"), "got: {err}");
}

#[tokio::test]
async fn test_sleep_rejects_json_array_payload() {
    let task = make_task("sleep", b"[1,2,3]");
    let result = sleep_handler(task).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("invalid payload"), "got: {err}");
}

#[tokio::test]
async fn test_sleep_rejects_json_number_payload() {
    let task = make_task("sleep", b"42");
    let result = sleep_handler(task).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("invalid payload"), "got: {err}");
}

#[tokio::test]
async fn test_sleep_rejects_json_null_payload() {
    let task = make_task("sleep", b"null");
    let result = sleep_handler(task).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("invalid payload"), "got: {err}");
}

#[tokio::test]
async fn test_sleep_rejects_json_string_payload() {
    let task = make_task("sleep", br#""hello""#);
    let result = sleep_handler(task).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("invalid payload"), "got: {err}");
}

#[tokio::test]
async fn test_sleep_rejects_json_boolean_payload() {
    let task = make_task("sleep", b"true");
    let result = sleep_handler(task).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("invalid payload"), "got: {err}");
}

// ── Edge Cases ───────────────────────────────────────────────────────────────

#[tokio::test(start_paused = true)]
async fn test_sleep_accepts_payload_with_extra_json_fields() {
    let task = make_task(
        "sleep",
        br#"{"ms":10,"trace_id":"abc","extra":true}"#,
    );
    let join = tokio::spawn(sleep_handler(task));
    time::advance(Duration::from_millis(10)).await;
    let result = join.await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Ok(SLEPT_RESULT.clone()));
}

// Note: u64::MAX as a sleep duration (~584M years) is accepted by the parse
// layer (see test_parse_sleep_ms_u64_max_is_accepted below). Testing the
// actual sleep is infeasible — even with start_paused, tokio cannot
// schedule a timer that large. The parse test suffices.

#[test]
fn test_parse_sleep_ms_u64_max_is_accepted() {
    let result = parse_sleep_ms(br#"{"ms":18446744073709551615}"#);
    assert_eq!(result, Ok(u64::MAX));
}

// ── Invariant ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_invariant_sleep_handler_never_panics_on_any_bytes_input() {
    let payloads: &[&[u8]] = &[
        b"",
        b"\xff\xfe",
        b"not json",
        br#"{"other":42}"#,
        br#"{"ms":"fast"}"#,
        br#"{"ms":10.5}"#,
        br#"{"ms":-1}"#,
        br#"{"ms":{}}"#,
        b"[1,2,3]",
        b"42",
        b"null",
        br#""hello""#,
        b"true",
    ];
    for payload in payloads {
        let task = make_task("sleep", payload);
        let result = tokio::spawn(sleep_handler(task)).await;
        assert!(
            result.is_ok(),
            "sleep should not panic for payload: {payload:?}"
        );
        let _ = result.unwrap();
    }
}

// ── Pure parse_sleep_ms tests ────────────────────────────────────────────────

#[test]
fn test_parse_sleep_ms_valid() {
    assert_eq!(parse_sleep_ms(br#"{"ms":10}"#), Ok(10));
    assert_eq!(parse_sleep_ms(br#"{"ms":0}"#), Ok(0));
    assert_eq!(parse_sleep_ms(br#"{"ms":999999}"#), Ok(999999));
}

#[test]
fn test_parse_sleep_ms_with_extra_fields() {
    assert_eq!(
        parse_sleep_ms(br#"{"ms":5,"trace_id":"abc"}"#),
        Ok(5),
    );
}

#[test]
fn test_parse_sleep_ms_rejects_non_utf8() {
    assert!(parse_sleep_ms(b"\xff\xfe").is_err());
}

#[test]
fn test_parse_sleep_ms_rejects_invalid_json() {
    assert!(parse_sleep_ms(b"not json").is_err());
}

#[test]
fn test_parse_sleep_ms_rejects_missing_ms() {
    assert!(parse_sleep_ms(br#"{"other":42}"#).is_err());
}

#[test]
fn test_parse_sleep_ms_rejects_string_ms() {
    assert!(parse_sleep_ms(br#"{"ms":"fast"}"#).is_err());
}

#[test]
fn test_parse_sleep_ms_rejects_non_object() {
    assert!(parse_sleep_ms(b"[1]").is_err());
    assert!(parse_sleep_ms(b"42").is_err());
    assert!(parse_sleep_ms(b"null").is_err());
    assert!(parse_sleep_ms(br#""hello""#).is_err());
    assert!(parse_sleep_ms(b"true").is_err());
}

#[test]
fn test_parse_sleep_ms_rejects_empty() {
    assert!(parse_sleep_ms(b"").is_err());
}
