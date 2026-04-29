use crate::{BlobFailureAction, BlobStatus, OutputPolicy};

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
