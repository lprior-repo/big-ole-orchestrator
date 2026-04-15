use crate::{
    BlobFailureAction, BlobRef, BlobStatus, OutputPolicy, OutputRef, ParseError, INLINED_MAX_BYTES,
};
use rstest::rstest;
use serde_json::json;

// =========================================================================
// BlobRef::new — Happy Path
// =========================================================================

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

// =========================================================================
// BlobRef::new — Error Paths
// =========================================================================

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

// =========================================================================
// BlobRef — Accessors
// =========================================================================

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

// =========================================================================
// BlobRef — Serde Roundtrip
// =========================================================================

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

// =========================================================================
// BlobStatus — State Machine Transitions
// =========================================================================

#[test]
fn pending_can_transition_to_durably_stored() {
    assert!(BlobStatus::Pending.can_transition_to(BlobStatus::DurablyStored));
}

#[test]
fn pending_can_transition_to_failed() {
    assert!(BlobStatus::Pending.can_transition_to(BlobStatus::Failed));
}

#[test]
fn pending_cannot_skip_to_published() {
    assert!(!BlobStatus::Pending.can_transition_to(BlobStatus::Published));
}

#[test]
fn pending_cannot_transition_to_itself() {
    assert!(!BlobStatus::Pending.can_transition_to(BlobStatus::Pending));
}

#[test]
fn durably_stored_can_transition_to_published() {
    assert!(BlobStatus::DurablyStored.can_transition_to(BlobStatus::Published));
}

#[test]
fn durably_stored_cannot_revert_to_pending() {
    assert!(!BlobStatus::DurablyStored.can_transition_to(BlobStatus::Pending));
}

#[test]
fn durably_stored_cannot_transition_to_itself() {
    assert!(!BlobStatus::DurablyStored.can_transition_to(BlobStatus::DurablyStored));
}

#[test]
fn durably_stored_cannot_transition_to_failed() {
    assert!(!BlobStatus::DurablyStored.can_transition_to(BlobStatus::Failed));
}

#[test]
fn published_is_terminal_state() {
    let variants = BlobStatus::all_variants();
    for &target in variants {
        assert!(
            !BlobStatus::Published.can_transition_to(target),
            "Published should not transition to {:?}",
            target
        );
    }
}

#[test]
fn failed_is_terminal_state() {
    let variants = BlobStatus::all_variants();
    for &target in variants {
        assert!(
            !BlobStatus::Failed.can_transition_to(target),
            "Failed should not transition to {:?}",
            target
        );
    }
}

// =========================================================================
// BlobStatus — all_variants
// =========================================================================

#[test]
fn blob_status_all_variants_returns_four_in_declared_order() {
    let variants = BlobStatus::all_variants();
    assert_eq!(variants.len(), 4);
    assert_eq!(
        variants,
        &[
            BlobStatus::Pending,
            BlobStatus::DurablyStored,
            BlobStatus::Published,
            BlobStatus::Failed,
        ]
    );
}

// =========================================================================
// BlobStatus — Serde Roundtrip
// =========================================================================

#[rstest]
#[case(BlobStatus::Pending)]
#[case(BlobStatus::DurablyStored)]
#[case(BlobStatus::Published)]
#[case(BlobStatus::Failed)]
fn blob_status_serde_roundtrips(#[case] status: BlobStatus) {
    let json_str = serde_json::to_string(&status).expect("serialize");
    let recovered: BlobStatus = serde_json::from_str(&json_str).expect("deserialize");
    assert_eq!(status, recovered);
}

// =========================================================================
// OutputRef::inline — Happy Path
// =========================================================================

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

// =========================================================================
// OutputRef::inline — Error Path
// =========================================================================

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

// =========================================================================
// OutputRef::blob_ref — Construction
// =========================================================================

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

// =========================================================================
// OutputRef — Discriminators
// =========================================================================

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

// =========================================================================
// OutputRef — Serde Roundtrip
// =========================================================================

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

// =========================================================================
// INLINED_MAX_BYTES — Constant
// =========================================================================

#[test]
fn inlined_max_bytes_is_4096() {
    assert_eq!(INLINED_MAX_BYTES, 4096);
}

// =========================================================================
// BlobRef — Equality & Clone
// =========================================================================

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

// =========================================================================
// BlobStatus — Equality
// =========================================================================

#[test]
fn blob_status_equality_works() {
    assert_eq!(BlobStatus::Pending, BlobStatus::Pending);
    assert_eq!(BlobStatus::Published, BlobStatus::Published);
    assert_ne!(BlobStatus::Pending, BlobStatus::Published);
}

// =========================================================================
// OutputRef — Equality & Clone
// =========================================================================

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

// =========================================================================
// OutputPolicy — ADR-040 §3 optional-output blob failure rules
// =========================================================================

#[test]
fn output_policy_optional_permits_completion_on_blob_failure() {
    assert!(OutputPolicy::Optional.permits_completion_on_blob_failure());
}

#[test]
fn output_policy_required_blocks_completion_on_blob_failure() {
    assert!(!OutputPolicy::Required.permits_completion_on_blob_failure());
}

#[test]
fn output_policy_required_is_required_for_replay() {
    assert!(OutputPolicy::Required.is_required_for_replay());
}

#[test]
fn output_policy_optional_is_not_required_for_replay() {
    assert!(!OutputPolicy::Optional.is_required_for_replay());
}

#[test]
fn output_policy_serde_roundtrips() {
    let policies = [OutputPolicy::Required, OutputPolicy::Optional];
    for policy in policies {
        let json_str = serde_json::to_string(&policy).expect("serialize");
        let recovered: OutputPolicy = serde_json::from_str(&json_str).expect("deserialize");
        assert_eq!(policy, recovered);
    }
}

// =========================================================================
// BlobFailureAction — ADR-040 §3 failure semantics
// =========================================================================

#[test]
fn required_output_blocks_step_on_blob_failure() {
    let action = OutputPolicy::Required.blob_failure_action(BlobStatus::Failed);
    assert_eq!(action, BlobFailureAction::BlockStep);
}

#[test]
fn optional_output_allows_inline_completion_on_blob_failure() {
    let action = OutputPolicy::Optional.blob_failure_action(BlobStatus::Failed);
    assert_eq!(action, BlobFailureAction::CompleteWithInline);
}

#[test]
fn non_failed_blob_status_blocks_step_regardless_of_policy() {
    let statuses = [
        BlobStatus::Pending,
        BlobStatus::DurablyStored,
        BlobStatus::Published,
    ];
    let policies = [OutputPolicy::Required, OutputPolicy::Optional];
    for status in statuses {
        for policy in policies {
            let action = policy.blob_failure_action(status);
            assert_eq!(
                action,
                BlobFailureAction::BlockStep,
                "Non-failed status {:?} should block step regardless of policy {:?}",
                status,
                policy
            );
        }
    }
}

#[test]
fn blob_failure_action_serde_roundtrips() {
    let actions = [
        BlobFailureAction::BlockStep,
        BlobFailureAction::CompleteWithInline,
    ];
    for action in actions {
        let json_str = serde_json::to_string(&action).expect("serialize");
        let recovered: BlobFailureAction = serde_json::from_str(&json_str).expect("deserialize");
        assert_eq!(action, recovered);
    }
}

// =========================================================================
// ADR-040 §3 Invariant: Replay never requires optional blob
// =========================================================================

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

// =========================================================================
// ADR-040 Invariant: output_ref never published before blob
// =========================================================================

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
