//! Proptests for read_input_inner and write_success_inner / write_failure_inner.

#![allow(clippy::unwrap_used)]

use std::io::Cursor;

use proptest::prelude::*;
use serde_json::json;

use crate::tests::read_input_inner_with_state as read_input_inner;
use crate::tests::{write_failure_inner_with_state as write_failure_inner, write_success_inner_with_state as write_success_inner};
use crate::{SdkError, TaskFailureKind};

use super::valid_envelope;

prop_compose! {
    fn valid_idempotency_key()(s in "[a-z][a-z0-9_-]{0,30}") -> String { s }
}

fn json_value_strategy() -> impl Strategy<Value = serde_json::Value> {
    let leaf = prop_oneof![
        Just(json!(null)),
        any::<bool>().prop_map_into(),
        any::<i64>().prop_map_into(),
        any::<f64>().prop_map(|f| json!(f)),
        ".*".prop_map(|s| json!(s)),
    ];
    leaf.prop_recursive(3, 64, 8, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..4).prop_map(|v| json!(v)),
            prop::collection::hash_map(".*", inner, 0..4).prop_map(|m| json!(m)),
        ]
    })
}

proptest! {
    #[test]
    fn read_valid_envelope_always_parses(key in valid_idempotency_key(), value in json_value_strategy()) {
        let payload = valid_envelope(&key, &value);
        let mut cursor = Cursor::new(payload);
        let mut is_read = false;
        let result = read_input_inner(&mut cursor, &mut is_read);
        prop_assert!(result.is_ok(), "valid envelope should parse: {:?}", result);
        let input = result.unwrap();
        prop_assert_eq!(input.idempotency_key.as_str(), key);
    }

    #[test]
    fn read_invalid_json_always_fails(input in ".{0,20}") {
        let mut cursor = Cursor::new(input.as_bytes().to_vec());
        let mut is_read = false;
        let result = read_input_inner(&mut cursor, &mut is_read);
        if let Ok(parsed) = serde_json::from_slice::<serde_json::Value>(input.as_bytes()) {
            if let Some(obj) = parsed.as_object() {
                if obj.contains_key("idempotency_key") && obj.contains_key("data") {
                    if let Some(k) = obj["idempotency_key"].as_str() {
                        if vo_types::IdempotencyKey::parse(k).is_ok() {
                            return Ok(());
                        }
                    }
                }
            }
        }
        prop_assert_eq!(result, Err(SdkError::InvalidInput));
    }

    #[test]
    fn write_success_any_json_value(output in json_value_strategy()) {
        let mut buf: Vec<u8> = Vec::new();
        let mut is_written = false;
        let result = write_success_inner(&mut buf, &output, &mut is_written);
        prop_assert!(is_written, "guard must always be set");
        if result.is_ok() {
            let written: serde_json::Value =
                serde_json::from_slice(&buf).expect("should be valid JSON");
            prop_assert_eq!(written["status"].as_str(), Some("success"));
        }
    }

    #[test]
    fn write_failure_any_message_length(message in ".{0,2000}") {
        let mut buf: Vec<u8> = Vec::new();
        let mut is_written = false;
        let result = write_failure_inner(
            &mut buf,
            TaskFailureKind::User,
            &message,
            &mut is_written,
        );
        prop_assert!(is_written, "guard must always be set");
        if message.len() <= 1024 {
            prop_assert!(result.is_ok(), "messages <= 1024 bytes should succeed");
            let written: serde_json::Value =
                serde_json::from_slice(&buf).expect("should be valid JSON");
            prop_assert_eq!(written["status"].as_str(), Some("failure"));
        } else {
            prop_assert_eq!(result, Err(SdkError::InvalidInput));
        }
    }

    #[test]
    fn write_failure_all_kinds_roundtrip(kind_idx in 0usize..3) {
        let kind = match kind_idx {
            0 => TaskFailureKind::User,
            1 => TaskFailureKind::System,
            _ => TaskFailureKind::Timeout,
        };
        let mut buf: Vec<u8> = Vec::new();
        let mut is_written = false;
        write_failure_inner(&mut buf, kind, "msg", &mut is_written).unwrap();
        let written: serde_json::Value =
            serde_json::from_slice(&buf).expect("valid JSON");
        prop_assert_eq!(written["status"].as_str(), Some("failure"));
        prop_assert_eq!(written["kind"].as_str(), Some(kind.as_str()));
    }

    #[test]
    fn read_guard_set_on_all_outcomes(key in valid_idempotency_key()) {
        let payload = valid_envelope(&key, &json!(null));
        let mut cursor = Cursor::new(payload);
        let mut is_read = false;
        let _ = read_input_inner(&mut cursor, &mut is_read);
        prop_assert!(is_read, "guard must be set regardless of outcome");
    }

    #[test]
    fn write_success_guard_set_on_all_outcomes(output in json_value_strategy()) {
        let mut buf: Vec<u8> = Vec::new();
        let mut is_written = false;
        let _ = write_success_inner(&mut buf, &output, &mut is_written);
        prop_assert!(is_written, "guard must be set regardless of outcome");
    }

    #[test]
    fn write_failure_guard_set_on_all_outcomes(message in ".{0,2000}") {
        let mut buf: Vec<u8> = Vec::new();
        let mut is_written = false;
        let _ = write_failure_inner(
            &mut buf,
            TaskFailureKind::System,
            &message,
            &mut is_written,
        );
        prop_assert!(is_written, "guard must be set regardless of outcome");
    }

    #[test]
    fn read_double_guard_always_fails(key in valid_idempotency_key()) {
        let payload = valid_envelope(&key, &json!(null));
        let mut cursor = Cursor::new(payload);
        let mut is_read = false;
        read_input_inner(&mut cursor, &mut is_read).unwrap();
        let mut cursor2 = Cursor::new(valid_envelope(&key, &json!(null)));
        let result = read_input_inner(&mut cursor2, &mut is_read);
        prop_assert_eq!(result, Err(SdkError::FdNotOpen));
    }

    #[test]
    fn write_success_double_guard_always_fails(output in json_value_strategy()) {
        let mut buf: Vec<u8> = Vec::new();
        let mut is_written = false;
        let _ = write_success_inner(&mut buf, &json!(1), &mut is_written);
        let result = write_success_inner(&mut buf, &output, &mut is_written);
        prop_assert_eq!(result, Err(SdkError::AlreadyWritten));
    }

    #[test]
    fn write_failure_double_guard_always_fails(message in ".{0,100}") {
        let mut buf: Vec<u8> = Vec::new();
        let mut is_written = false;
        write_failure_inner(&mut buf, TaskFailureKind::User, "first", &mut is_written).unwrap();
        let result = write_failure_inner(&mut buf, TaskFailureKind::User, &message, &mut is_written);
        prop_assert_eq!(result, Err(SdkError::AlreadyWritten));
    }
}
