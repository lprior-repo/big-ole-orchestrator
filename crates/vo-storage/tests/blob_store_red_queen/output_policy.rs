use vo_types::{BlobFailureAction, BlobStatus, OutputPolicy};

#[test]
fn red_queen_output_policy_required_blocks_on_blob_failure() {
    let action = OutputPolicy::Required.blob_failure_action(BlobStatus::Failed);
    assert_eq!(
        action,
        BlobFailureAction::BlockStep,
        "Required output must block step on blob failure"
    );
}

#[test]
fn red_queen_output_policy_optional_allows_inline_on_blob_failure() {
    let action = OutputPolicy::Optional.blob_failure_action(BlobStatus::Failed);
    assert_eq!(
        action,
        BlobFailureAction::CompleteWithInline,
        "Optional output must allow inline completion on blob failure"
    );
}

#[test]
fn red_queen_output_policy_non_failed_status_blocks_regardless() {
    let statuses = [
        BlobStatus::Pending,
        BlobStatus::DurablyStored,
        BlobStatus::Published,
    ];
    for status in statuses {
        let required_action = OutputPolicy::Required.blob_failure_action(status);
        assert_eq!(
            required_action,
            BlobFailureAction::BlockStep,
            "Required policy must block for non-failed status {:?}",
            status
        );

        let optional_action = OutputPolicy::Optional.blob_failure_action(status);
        assert_eq!(
            optional_action,
            BlobFailureAction::BlockStep,
            "Optional policy must block for non-failed status {:?}",
            status
        );
    }
}

#[test]
fn red_queen_output_policy_required_is_required_for_replay() {
    assert!(
        OutputPolicy::Required.is_required_for_replay(),
        "Required must be required for replay"
    );
}

#[test]
fn red_queen_output_policy_optional_not_required_for_replay() {
    assert!(
        !OutputPolicy::Optional.is_required_for_replay(),
        "Optional must NOT be required for replay"
    );
}

#[test]
fn red_queen_output_policy_optional_permits_completion() {
    assert!(
        OutputPolicy::Optional.permits_completion_on_blob_failure(),
        "Optional must permit completion on blob failure"
    );
}

#[test]
fn red_queen_output_policy_required_denies_completion() {
    assert!(
        !OutputPolicy::Required.permits_completion_on_blob_failure(),
        "Required must deny completion on blob failure"
    );
}