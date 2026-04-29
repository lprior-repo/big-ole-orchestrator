use crate::{BlobFailureAction, BlobRef, BlobStatus, OutputPolicy, OutputRef, INLINED_MAX_BYTES};

#[test]
fn bdd_given_blob_output_when_publication_fails_then_required_blocks_step() {
    let policy = OutputPolicy::Required;
    let failed_status = BlobStatus::Failed;

    let action = policy.blob_failure_action(failed_status);

    assert_eq!(
        action,
        BlobFailureAction::BlockStep,
        "Required output must block step when blob fails"
    );
}

#[test]
fn bdd_given_blob_output_when_publication_fails_then_optional_allows_inline_completion() {
    let policy = OutputPolicy::Optional;
    let failed_status = BlobStatus::Failed;

    let action = policy.blob_failure_action(failed_status);

    assert_eq!(
        action,
        BlobFailureAction::CompleteWithInline,
        "Optional output must allow completion with inline data when blob fails"
    );
}

#[test]
fn bdd_given_blob_output_when_publication_fails_then_failed_status_is_terminal() {
    let failed = BlobStatus::Failed;

    for target in BlobStatus::all_variants() {
        assert!(
            !failed.can_transition_to(*target),
            "Failed blob must be terminal, but can transition to {:?}",
            target
        );
    }
}

#[test]
fn bdd_given_inline_blob_under_limit_when_stored_then_embedded_in_event() {
    let data = vec![0xAB; INLINED_MAX_BYTES];

    let output = OutputRef::inline(data.clone()).expect("should accept data at limit");

    assert!(output.is_inline(), "Data at limit must be stored inline");
    assert!(!output.is_blob_ref(), "Must not be an external blob ref");
    assert_eq!(output.as_inline(), Some(data.as_slice()));
    assert_eq!(
        output.as_blob_ref(),
        None,
        "Must not have external blob ref"
    );
}

#[test]
fn bdd_given_inline_blob_well_under_limit_when_stored_then_embedded_in_event() {
    let data = vec![0x42; 100];

    let output = OutputRef::inline(data.clone()).expect("should accept small data");

    assert!(output.is_inline());
    assert_eq!(output.as_inline(), Some(data.as_slice()));
    assert_eq!(output.as_blob_ref(), None);
}

#[test]
fn bdd_given_inline_blob_when_classified_under_limit_then_inline_variant() {
    let data = vec![0xFF; INLINED_MAX_BYTES - 1];

    let output = OutputRef::classify(data).expect("should classify under-limit data");

    assert!(output.is_inline());
    assert!(!output.is_blob_ref());
}

#[test]
fn bdd_given_blob_exceeding_threshold_when_inline_attempted_then_rejected() {
    let data = vec![0x00; INLINED_MAX_BYTES + 1];

    let result = OutputRef::inline(data);

    assert!(
        result.is_err(),
        "Must reject data exceeding inline threshold"
    );
    assert_eq!(
        result,
        Err(crate::ParseError::ExceedsMaxLength {
            type_name: "OutputRef.inline",
            max: INLINED_MAX_BYTES,
            actual: INLINED_MAX_BYTES + 1,
        })
    );
}

#[test]
fn bdd_given_blob_exceeding_threshold_when_classified_then_rejected() {
    let data = vec![0xDD; INLINED_MAX_BYTES + 512];

    let result = OutputRef::classify(data);

    assert!(result.is_err());
}

#[test]
fn bdd_given_blob_exceeding_threshold_when_blobref_created_then_external_reference() {
    let large_data_size: u64 = (INLINED_MAX_BYTES as u64) + 1;

    let blob = BlobRef::new(
        "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
        large_data_size,
        "abcdef0123456789abcdef0123456789",
    )
    .expect("should construct BlobRef for large payload");

    let output = OutputRef::blob_ref(blob);
    assert!(output.is_blob_ref(), "Must be external blob reference");
    assert!(!output.is_inline(), "Must not be inline");
    assert!(output.as_inline().is_none(), "Must not have inline data");
    assert!(output.as_blob_ref().is_some(), "Must have external BlobRef");
}

#[test]
fn bdd_given_blob_exceeding_threshold_when_blobref_created_then_size_preserved() {
    let large_size: u64 = (INLINED_MAX_BYTES as u64) * 10;

    let blob = BlobRef::new(
        "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
        large_size,
        "abcdef0123456789abcdef0123456789",
    )
    .expect("should construct");

    assert_eq!(blob.size_bytes(), large_size);

    let output = OutputRef::blob_ref(blob);
    let retrieved = output.as_blob_ref().expect("should have blob ref");
    assert_eq!(retrieved.size_bytes(), large_size);
}
