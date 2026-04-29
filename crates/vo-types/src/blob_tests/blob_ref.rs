use crate::{
    BlobFailureAction, BlobRef, BlobStatus, OutputPolicy, OutputRef, ParseError, INLINED_MAX_BYTES,
};
use rstest::rstest;
use serde_json::json;

#[test]
fn blobref_constructs_with_all_valid_fields() {
    let blob = BlobRef::new(
        "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
        1024,
        "abcdef0123456789abcdef0123456789",
    );
    let blob = blob.expect("BlobRef should construct with valid fields");
    assert_eq!(blob.blob_id(), "01H5JQX7K3R4T6V8W0X2Y4Z6A8");
    assert_eq!(blob.size_bytes(), 1024);
    assert_eq!(blob.content_hash(), "abcdef0123456789abcdef0123456789");
}

#[test]
fn blobref_constructs_with_minimum_valid_content_hash() {
    let blob = BlobRef::new("01H5JQX7K3R4T6V8W0X2Y4Z6A8", 1, "abcdef01");
    let blob = blob.expect("BlobRef should construct with 8-char hash");
    assert_eq!(blob.content_hash(), "abcdef01");
    assert_eq!(blob.size_bytes(), 1);
}

#[test]
fn blobref_rejects_empty_blob_id() {
    let result = BlobRef::new("", 1024, "abcdef0123456789abcdef0123456789");
    assert_eq!(
        result,
        Err(ParseError::Empty {
            type_name: "BlobRef.blob_id"
        })
    );
}

#[test]
fn blobref_rejects_invalid_ulid_blob_id() {
    let result = BlobRef::new("not-a-ulid", 1024, "abcdef0123456789abcdef0123456789");
    assert!(matches!(
        result,
        Err(ParseError::InvalidFormat {
            type_name: "BlobRef.blob_id",
            ..
        })
    ));
}

#[test]
fn blobref_rejects_blob_id_with_wrong_length() {
    let result = BlobRef::new("01H5JQX7", 1024, "abcdef0123456789abcdef0123456789");
    assert!(matches!(
        result,
        Err(ParseError::InvalidFormat {
            type_name: "BlobRef.blob_id",
            ..
        })
    ));
}

#[test]
fn blobref_rejects_empty_content_hash() {
    let result = BlobRef::new("01H5JQX7K3R4T6V8W0X2Y4Z6A8", 1024, "");
    assert_eq!(
        result,
        Err(ParseError::Empty {
            type_name: "BlobRef.content_hash"
        })
    );
}

#[test]
fn blobref_rejects_non_hex_content_hash() {
    let result = BlobRef::new("01H5JQX7K3R4T6V8W0X2Y4Z6A8", 1024, "ghijklmnop");
    assert_eq!(
        result,
        Err(ParseError::InvalidCharacters {
            type_name: "BlobRef.content_hash",
            invalid_chars: "ghijklmnop".to_string()
        })
    );
}

#[test]
fn blobref_rejects_odd_length_content_hash() {
    let result = BlobRef::new("01H5JQX7K3R4T6V8W0X2Y4Z6A8", 1024, "abcde");
    assert!(matches!(
        result,
        Err(ParseError::InvalidFormat {
            type_name: "BlobRef.content_hash",
            ..
        })
    ));
}

#[test]
fn blobref_rejects_short_content_hash() {
    let result = BlobRef::new("01H5JQX7K3R4T6V8W0X2Y4Z6A8", 1024, "ab");
    assert!(matches!(
        result,
        Err(ParseError::InvalidFormat {
            type_name: "BlobRef.content_hash",
            ..
        })
    ));
}

#[test]
fn blobref_rejects_zero_size_bytes() {
    let result = BlobRef::new(
        "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
        0,
        "abcdef0123456789abcdef0123456789",
    );
    assert_eq!(
        result,
        Err(ParseError::ZeroValue {
            type_name: "BlobRef.size_bytes"
        })
    );
}

#[test]
fn blobref_exposes_all_fields_via_accessors() {
    let blob = BlobRef::new(
        "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
        42,
        "abcdef0123456789abcdef0123456789",
    )
    .expect("should construct");
    assert_eq!(blob.blob_id(), "01H5JQX7K3R4T6V8W0X2Y4Z6A8");
    assert_eq!(blob.size_bytes(), 42);
    assert_eq!(blob.content_hash(), "abcdef0123456789abcdef0123456789");
}

#[test]
fn blobref_serde_roundtrips() {
    let blob = BlobRef::new(
        "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
        1024,
        "abcdef0123456789abcdef0123456789",
    )
    .expect("should construct");
    let json_str = serde_json::to_string(&blob).expect("serialize");
    let recovered: BlobRef = serde_json::from_str(&json_str).expect("deserialize");
    assert_eq!(blob, recovered);
}

#[test]
fn blobref_serializes_to_expected_json_structure() {
    let blob = BlobRef::new(
        "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
        1024,
        "abcdef0123456789abcdef0123456789",
    )
    .expect("should construct");
    let json_val = serde_json::to_value(&blob).expect("serialize");
    assert_eq!(json_val["blob_id"], json!("01H5JQX7K3R4T6V8W0X2Y4Z6A8"));
    assert_eq!(json_val["size_bytes"], json!(1024));
    assert_eq!(
        json_val["content_hash"],
        json!("abcdef0123456789abcdef0123456789")
    );
}

#[test]
fn blobref_equality_works_for_same_values() {
    let a = BlobRef::new(
        "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
        100,
        "abcdef0123456789abcdef0123456789",
    )
    .expect("should construct");
    let b = BlobRef::new(
        "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
        100,
        "abcdef0123456789abcdef0123456789",
    )
    .expect("should construct");
    assert_eq!(a, b);
}

#[test]
fn blobref_inequality_works_for_different_values() {
    let a = BlobRef::new(
        "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
        100,
        "abcdef0123456789abcdef0123456789",
    )
    .expect("should construct");
    let b = BlobRef::new(
        "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
        200,
        "abcdef0123456789abcdef0123456789",
    )
    .expect("should construct");
    assert_ne!(a, b);
}

#[test]
fn blobref_clone_produces_equal_value() {
    let blob = BlobRef::new(
        "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
        100,
        "abcdef0123456789abcdef0123456789",
    )
    .expect("should construct");
    assert_eq!(blob.clone(), blob);
}
