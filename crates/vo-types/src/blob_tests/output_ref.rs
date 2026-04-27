use crate::{BlobRef, OutputRef, ParseError, INLINED_MAX_BYTES};

#[test]
fn outputref_inline_constructs_when_within_max() {
    let data = vec![1u8; 100];
    let result = OutputRef::inline(data.clone()).expect("should construct");
    assert!(result.is_inline());
    assert!(!result.is_blob_ref());
    assert_eq!(result.as_inline(), Some(data.as_slice()));
    assert_eq!(result.as_blob_ref(), None);
}

#[test]
fn outputref_inline_accepts_exactly_max_bytes() {
    let data = vec![0u8; INLINED_MAX_BYTES];
    let result = OutputRef::inline(data.clone()).expect("should accept exactly max");
    assert_eq!(result.as_inline(), Some(data.as_slice()));
}

#[test]
fn outputref_inline_accepts_empty_bytes() {
    let result = OutputRef::inline(vec![]).expect("should accept empty");
    assert!(result.is_inline());
    assert_eq!(result.as_inline(), Some(&[][..]));
}

#[test]
fn outputref_inline_rejects_when_exceeds_max() {
    let data = vec![0u8; INLINED_MAX_BYTES + 1];
    let result = OutputRef::inline(data);
    assert_eq!(
        result,
        Err(ParseError::ExceedsMaxLength {
            type_name: "OutputRef.inline",
            max: INLINED_MAX_BYTES,
            actual: INLINED_MAX_BYTES + 1,
        })
    );
}

#[test]
fn outputref_inline_rejects_huge_data() {
    let data = vec![0u8; 1_000_000];
    let result = OutputRef::inline(data);
    assert_eq!(
        result,
        Err(ParseError::ExceedsMaxLength {
            type_name: "OutputRef.inline",
            max: INLINED_MAX_BYTES,
            actual: 1_000_000,
        })
    );
}

#[test]
fn outputref_blob_ref_constructs_from_valid_blobref() {
    let blob = BlobRef::new(
        "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
        1024,
        "abcdef0123456789abcdef0123456789",
    )
    .expect("should construct");
    let output = OutputRef::blob_ref(blob.clone());
    assert!(!output.is_inline());
    assert!(output.is_blob_ref());
    assert_eq!(output.as_inline(), None);
    assert_eq!(output.as_blob_ref(), Some(&blob));
}

#[test]
fn outputref_discriminators_return_correct_values_for_inline() {
    let output = OutputRef::inline(vec![1, 2, 3]).expect("should construct");
    assert!(output.is_inline());
    assert!(!output.is_blob_ref());
    assert_eq!(output.as_inline(), Some(&[1, 2, 3][..]));
    assert_eq!(output.as_blob_ref(), None);
}

#[test]
fn outputref_discriminators_return_correct_values_for_blob_ref() {
    let blob = BlobRef::new(
        "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
        1024,
        "abcdef0123456789abcdef0123456789",
    )
    .expect("should construct");
    let output = OutputRef::blob_ref(blob);
    assert!(!output.is_inline());
    assert!(output.is_blob_ref());
    assert_eq!(output.as_inline(), None);
    assert!(output.as_blob_ref().is_some());
}

#[test]
fn outputref_inline_serde_roundtrips() {
    let output = OutputRef::inline(vec![10, 20, 30]).expect("should construct");
    let json_str = serde_json::to_string(&output).expect("serialize");
    let recovered: OutputRef = serde_json::from_str(&json_str).expect("deserialize");
    assert_eq!(output, recovered);
}

#[test]
fn outputref_blobref_variant_serde_roundtrips() {
    let blob = BlobRef::new(
        "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
        1024,
        "abcdef0123456789abcdef0123456789",
    )
    .expect("should construct");
    let output = OutputRef::blob_ref(blob);
    let json_str = serde_json::to_string(&output).expect("serialize");
    let recovered: OutputRef = serde_json::from_str(&json_str).expect("deserialize");
    assert_eq!(output, recovered);
}

#[test]
fn inlined_max_bytes_is_4096() {
    assert_eq!(INLINED_MAX_BYTES, 4096);
}

#[test]
fn outputref_equality_works_for_inline() {
    let a = OutputRef::inline(vec![1, 2]).expect("should construct");
    let b = OutputRef::inline(vec![1, 2]).expect("should construct");
    assert_eq!(a, b);
}

#[test]
fn outputref_inequality_works_for_different_inline_data() {
    let a = OutputRef::inline(vec![1, 2]).expect("should construct");
    let b = OutputRef::inline(vec![3, 4]).expect("should construct");
    assert_ne!(a, b);
}