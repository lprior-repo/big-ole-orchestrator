use crate::ParseError;
use crate::*;

#[test]
fn instance_id_accepts_valid_ulid_when_input_is_wellformed() {
    let id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID");
    assert_eq!(id.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
}

#[test]
fn instance_id_rejects_empty_with_empty_error_when_input_is_empty() {
    assert_eq!(
        InstanceId::parse(""),
        Err(ParseError::Empty {
            type_name: "InstanceId"
        })
    );
}

#[test]
fn instance_id_rejects_wrong_length_with_invalid_format_when_input_is_not_26_chars() {
    let result = InstanceId::parse("01H5JYV4XH");
    assert!(matches!(
        result,
        Err(ParseError::InvalidFormat {
            type_name: "InstanceId",
            ref reason
        }) if reason.contains("26")
    ));
}

#[test]
fn instance_id_rejects_invalid_chars_with_invalid_format_when_input_has_non_crockford_chars() {
    let result = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFM@");
    assert!(matches!(
        result,
        Err(ParseError::InvalidFormat {
            type_name: "InstanceId",
            ..
        })
    ));
}

#[test]
fn instance_id_rejects_malformed_ulid_with_invalid_format_when_ulid_validation_fails() {
    let result = InstanceId::parse("00000000000000000000000000");
    assert!(matches!(
        result,
        Err(ParseError::InvalidFormat {
            type_name: "InstanceId",
            ref reason
        }) if reason.to_lowercase().contains("validation")
            || reason.to_lowercase().contains("ulid")
            || reason.to_lowercase().contains("nil")
    ));
}

#[test]
fn instance_id_rejects_long_input_with_invalid_format_when_input_exceeds_26_chars() {
    let result = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMAAAA");
    assert!(matches!(
        result,
        Err(ParseError::InvalidFormat {
            type_name: "InstanceId",
            ref reason
        }) if reason.contains("26")
    ));
}

#[test]
fn instance_id_rejects_leading_whitespace_with_invalid_format_when_input_has_space_prefix() {
    let result = InstanceId::parse(" 01H5JYV4XHGSR2F8KZ9BWNRFMA");
    assert!(matches!(
        result,
        Err(ParseError::InvalidFormat {
            type_name: "InstanceId",
            ..
        })
    ));
}

#[test]
fn instance_id_rejects_trailing_whitespace_with_invalid_format_when_input_has_space_suffix() {
    let result = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA ");
    assert!(matches!(
        result,
        Err(ParseError::InvalidFormat {
            type_name: "InstanceId",
            ..
        })
    ));
}

#[test]
fn instance_id_display_equals_inner_string() {
    let id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid");
    assert_eq!(format!("{id}"), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
}

#[test]
fn instance_id_display_round_trips_through_parse_when_valid() {
    let id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid");
    let s = format!("{id}");
    assert_eq!(InstanceId::parse(&s), Ok(id));
}

#[test]
fn instance_id_try_from_string_valid() {
    let id = InstanceId::try_from("01H5JYV4XHGSR2F8KZ9BWNRFMA".to_string()).expect("valid");
    assert_eq!(id.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
}

#[test]
fn instance_id_try_from_string_invalid() {
    let result = InstanceId::try_from("bad".to_string());
    assert!(matches!(
        result,
        Err(ParseError::InvalidFormat {
            type_name: "InstanceId",
            ..
        })
    ));
}

#[test]
fn instance_id_from_into_string() {
    let id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid");
    let s: String = id.into();
    assert_eq!(s, "01H5JYV4XHGSR2F8KZ9BWNRFMA");
}

#[test]
fn instance_id_to_bytes_returns_correct_bytes_for_valid_ulid() {
    let id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let bytes = id.to_bytes().expect("valid ULID should convert to bytes");
    assert_ne!(bytes, [0; 16]);
    assert_ne!(bytes, [1; 16]);
    let reconstructed = InstanceId::from_bytes(bytes);
    assert_eq!(reconstructed.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
}

#[test]
fn instance_id_to_bytes_returns_error_when_ulid_invalid() {
    let id = InstanceId("invalid".to_string());
    assert!(matches!(
        id.to_bytes(),
        Err(ParseError::InvalidFormat { .. })
    ));
}
