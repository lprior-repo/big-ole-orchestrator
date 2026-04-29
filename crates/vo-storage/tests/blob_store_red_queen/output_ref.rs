use crate::helpers::{make_blob_ref, VALID_SHA256};
use vo_types::{BlobRef, OutputRef, INLINED_MAX_BYTES};

#[test]
fn red_queen_outputref_inline_within_max_bytes() {
    let data = vec![0u8; INLINED_MAX_BYTES];
    let result = OutputRef::inline(data.clone());
    assert!(result.is_ok(), "Must accept exactly INLINED_MAX_BYTES");
    let output = result.unwrap();
    assert!(output.is_inline());
    assert!(!output.is_blob_ref());
    assert_eq!(output.as_inline(), Some(data.as_slice()));
}

#[test]
fn red_queen_outputref_inline_exactly_at_boundary() {
    let data = vec![1u8; INLINED_MAX_BYTES];
    let result = OutputRef::inline(data);
    assert!(
        result.is_ok(),
        "Must accept exactly INLINED_MAX_BYTES bytes"
    );
}

#[test]
fn red_queen_outputref_inline_exceeds_max_rejected() {
    let data = vec![2u8; INLINED_MAX_BYTES + 1];
    let result = OutputRef::inline(data);
    assert!(
        result.is_err(),
        "Must reject data exceeding INLINED_MAX_BYTES"
    );
}

#[test]
fn red_queen_outputref_blob_ref_construction() {
    let blob = make_blob_ref();
    let output = OutputRef::blob_ref(blob.clone());
    assert!(!output.is_inline());
    assert!(output.is_blob_ref());
    assert_eq!(output.as_blob_ref(), Some(&blob));
    assert_eq!(output.as_inline(), None);
}

#[test]
fn red_queen_outputref_inline_and_blob_ref_are_unequal() {
    let inline_output = OutputRef::inline(vec![1, 2, 3]).unwrap();
    let blob_output = OutputRef::blob_ref(make_blob_ref());
    assert_ne!(
        inline_output, blob_output,
        "Inline and BlobRef variants must be unequal"
    );
}

#[test]
fn red_queen_outputref_classify_small_data_as_inline() {
    let small_data = vec![0u8; 100];
    let result = OutputRef::classify(small_data.clone());
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(
        output.is_inline(),
        "Small data must be classified as inline"
    );
}

#[test]
fn red_queen_outputref_classify_exactly_max_as_inline() {
    let data = vec![0u8; INLINED_MAX_BYTES];
    let result = OutputRef::classify(data);
    assert!(result.is_ok(), "Exactly INLINED_MAX_BYTES must succeed");
}

#[test]
fn red_queen_outputref_classify_exceeds_max_rejected() {
    let data = vec![0u8; INLINED_MAX_BYTES + 1];
    let result = OutputRef::classify(data);
    assert!(result.is_err(), "Exceeding INLINED_MAX_BYTES must fail");
}

#[test]
fn red_queen_outputref_dual_representation_serde_preserves_variant() {
    let blob_ref_output = OutputRef::blob_ref(make_blob_ref());
    let json = serde_json::to_string(&blob_ref_output).unwrap();
    let recovered: OutputRef = serde_json::from_str(&json).unwrap();
    assert_eq!(blob_ref_output, recovered);
    assert!(recovered.is_blob_ref());

    let inline_output = OutputRef::inline(vec![5, 6, 7]).unwrap();
    let json = serde_json::to_string(&inline_output).unwrap();
    let recovered: OutputRef = serde_json::from_str(&json).unwrap();
    assert_eq!(inline_output, recovered);
    assert!(recovered.is_inline());
}

#[test]
fn red_queen_outputref_empty_inline_is_valid() {
    let result = OutputRef::inline(vec![]);
    assert!(result.is_ok(), "Empty inline data must be valid");
    let output = result.unwrap();
    assert!(output.is_inline());
    assert_eq!(output.as_inline(), Some(&[][..]));
}