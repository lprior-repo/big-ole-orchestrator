use crate::{BlobFailureAction, BlobRef, BlobStatus, OutputPolicy, OutputRef, INLINED_MAX_BYTES};

#[test]
fn replay_never_requires_optional_blob() {
    let optional_policy = OutputPolicy::Optional;
    let failure_action = optional_policy.blob_failure_action(BlobStatus::Failed);
    assert_eq!(
        failure_action,
        BlobFailureAction::CompleteWithInline,
        "Optional blob failure must allow completion with inline data only"
    );
}

#[test]
fn adr040_published_blob_must_pass_through_durably_stored() {
    assert!(BlobStatus::Pending.can_transition_to(BlobStatus::DurablyStored));
    assert!(BlobStatus::DurablyStored.can_transition_to(BlobStatus::Published));
    assert!(!BlobStatus::Pending.can_transition_to(BlobStatus::Published));
}

#[test]
fn adr040_blob_failure_semantics_required_blocks_step() {
    for status in BlobStatus::all_variants() {
        let action = OutputPolicy::Required.blob_failure_action(*status);
        assert_eq!(
            action,
            BlobFailureAction::BlockStep,
            "Required policy must always block, got {:?} for status {:?}",
            action,
            status
        );
    }
}

#[test]
fn adr040_optional_blob_allows_completion_only_on_failure() {
    assert_eq!(
        OutputPolicy::Optional.blob_failure_action(BlobStatus::Failed),
        BlobFailureAction::CompleteWithInline
    );
    assert_eq!(
        OutputPolicy::Optional.blob_failure_action(BlobStatus::Pending),
        BlobFailureAction::BlockStep
    );
    assert_eq!(
        OutputPolicy::Optional.blob_failure_action(BlobStatus::DurablyStored),
        BlobFailureAction::BlockStep
    );
    assert_eq!(
        OutputPolicy::Optional.blob_failure_action(BlobStatus::Published),
        BlobFailureAction::BlockStep
    );
}

#[test]
fn adr040_inline_data_never_exceeds_max() {
    let max_data = vec![0u8; INLINED_MAX_BYTES];
    assert!(OutputRef::inline(max_data).is_ok());
    let over_data = vec![0u8; INLINED_MAX_BYTES + 1];
    assert!(OutputRef::inline(over_data).is_err());
}

#[test]
fn adr040_blob_ref_requires_valid_content_hash() {
    let valid_hash = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
    let blob = BlobRef::new("01H5JQX7K3R4T6V8W0X2Y4Z6A8", 100, valid_hash);
    assert!(blob.is_ok());
    let invalid_hash = "not-a-hash";
    let blob = BlobRef::new("01H5JQX7K3R4T6V8W0X2Y4Z6A8", 100, invalid_hash);
    assert!(blob.is_err());
}

#[test]
fn adr040_blob_status_published_is_irreversible() {
    let published = BlobStatus::Published;
    for target in BlobStatus::all_variants() {
        assert!(
            !published.can_transition_to(*target),
            "Published should be terminal, but allowed transition to {:?}",
            target
        );
    }
}

#[test]
fn adr040_blob_status_failed_is_irreversible() {
    let failed = BlobStatus::Failed;
    for target in BlobStatus::all_variants() {
        assert!(
            !failed.can_transition_to(*target),
            "Failed should be terminal, but allowed transition to {:?}",
            target
        );
    }
}

#[test]
fn adr040_output_ref_classify_respects_size_boundary() {
    let small = vec![0u8; INLINED_MAX_BYTES];
    let result = OutputRef::classify(small);
    assert!(result.is_ok());
    assert!(result.unwrap().is_inline());

    let large = vec![0u8; INLINED_MAX_BYTES + 1];
    let result = OutputRef::classify(large);
    assert!(result.is_err());
}
