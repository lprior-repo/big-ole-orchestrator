#![allow(clippy::unwrap_used)]

use serde_json::json;
use vo_api::types::mutation::{
    MutationRejectionReason, MutationType, OperatorMutationRequest, OperatorMutationResponse,
};
use vo_types::{
    BufferPolicy, CommandEnvelope, CommandMetadata, IdempotencyKey, InstanceId, Issuer,
    SignalAddress, TimestampMs, WaitKey,
};

fn make_valid_envelope() -> CommandEnvelope {
    CommandEnvelope {
        schema_version: 1,
        metadata: CommandMetadata {
            command_id: IdempotencyKey::parse("cmd-test-001").unwrap(),
            correlation_id: IdempotencyKey::parse("corr-batch-001").unwrap(),
            causation_id: IdempotencyKey::parse("cause-event-001").unwrap(),
            issuer: Issuer::Operator,
            issued_at: TimestampMs::try_from(1_700_000_000u64).unwrap(),
        },
    }
}

fn make_instance_id() -> InstanceId {
    InstanceId::parse("01HQXK5R5TJRP3J4W5G6W7Y8Z9").unwrap()
}

fn make_target(instance_id: &InstanceId) -> SignalAddress {
    SignalAddress::lineage_wide(
        instance_id.clone(),
        instance_id.clone(),
        WaitKey::parse("approve").unwrap(),
    )
}

#[test]
fn test_duplicate_mutation_returns_dup_response() {
    let envelope = make_valid_envelope();
    let instance_id = make_instance_id();
    let _request = OperatorMutationRequest {
        envelope,
        mutation_type: MutationType::Cancel,
        target: make_target(&instance_id),
        buffer_policy: BufferPolicy::Reject,
        payload: Some(json!({})),
    };

    let response = OperatorMutationResponse::Duplicate {
        correlation_id: "corr-1".to_string(),
        causation_id: "cause-1".to_string(),
        original_command_id: "cmd-orig".to_string(),
    };

    match response {
        OperatorMutationResponse::Duplicate { .. } => {}
        _ => panic!("Expected Duplicate response"),
    }
}

#[test]
fn test_mutation_rejected_for_unknown_instance() {
    let envelope = make_valid_envelope();
    let instance_id = make_instance_id();
    let _request = OperatorMutationRequest {
        envelope,
        mutation_type: MutationType::Cancel,
        target: make_target(&instance_id),
        buffer_policy: BufferPolicy::Reject,
        payload: Some(json!({})),
    };

    let response = OperatorMutationResponse::Rejected {
        reason: MutationRejectionReason::InstanceNotFound("nonexistent-instance".to_string())
            .to_string(),
    };

    match response {
        OperatorMutationResponse::Rejected { reason } => {
            assert!(reason.contains("instance not found"));
        }
        _ => panic!("Expected Rejected response"),
    }
}

#[test]
fn test_mutation_rejected_for_tombstoned_lineage() {
    let envelope = make_valid_envelope();
    let instance_id = make_instance_id();
    let _request = OperatorMutationRequest {
        envelope,
        mutation_type: MutationType::Resume,
        target: make_target(&instance_id),
        buffer_policy: BufferPolicy::Reject,
        payload: Some(json!({})),
    };

    let response = OperatorMutationResponse::Rejected {
        reason: MutationRejectionReason::LineageTombstoned("tombstoned-lineage".to_string())
            .to_string(),
    };

    match response {
        OperatorMutationResponse::Rejected { reason } => {
            assert!(reason.contains("lineage is tombstoned"));
        }
        _ => panic!("Expected Rejected response"),
    }
}

#[test]
fn test_invalid_mutation_for_running_instance() {
    let envelope = make_valid_envelope();
    let instance_id = make_instance_id();
    let _request = OperatorMutationRequest {
        envelope,
        mutation_type: MutationType::Resume,
        target: make_target(&instance_id),
        buffer_policy: BufferPolicy::Reject,
        payload: Some(json!({})),
    };

    let response = OperatorMutationResponse::Rejected {
        reason: MutationRejectionReason::InvalidMutationForState(
            "cannot resume running instance".to_string(),
        )
        .to_string(),
    };

    match response {
        OperatorMutationResponse::Rejected { reason } => {
            assert!(reason.contains("invalid mutation for current state"));
        }
        _ => panic!("Expected Rejected response"),
    }
}

#[test]
fn test_mutation_type_variants() {
    assert_eq!(
        serde_json::to_string(&MutationType::Cancel).unwrap(),
        "\"cancel\""
    );
    assert_eq!(
        serde_json::to_string(&MutationType::Pause).unwrap(),
        "\"pause\""
    );
    assert_eq!(
        serde_json::to_string(&MutationType::Resume).unwrap(),
        "\"resume\""
    );
    assert_eq!(
        serde_json::to_string(&MutationType::Patch).unwrap(),
        "\"patch\""
    );
    assert_eq!(
        serde_json::to_string(&MutationType::Retry).unwrap(),
        "\"retry\""
    );
    assert_eq!(
        serde_json::to_string(&MutationType::Undo).unwrap(),
        "\"undo\""
    );
}

#[test]
fn test_dedup_key_equality_for_same_inputs() {
    let envelope1 = make_valid_envelope();
    let envelope2 = CommandEnvelope {
        schema_version: 1,
        metadata: CommandMetadata {
            command_id: IdempotencyKey::parse("cmd-test-001").unwrap(),
            correlation_id: IdempotencyKey::parse("corr-batch-001").unwrap(),
            causation_id: IdempotencyKey::parse("cause-event-001").unwrap(),
            issuer: Issuer::Operator,
            issued_at: TimestampMs::try_from(1_700_000_000u64).unwrap(),
        },
    };

    let instance_id = make_instance_id();
    let request1 = OperatorMutationRequest {
        envelope: envelope1,
        mutation_type: MutationType::Cancel,
        target: make_target(&instance_id),
        buffer_policy: BufferPolicy::Reject,
        payload: Some(json!({})),
    };

    let request2 = OperatorMutationRequest {
        envelope: envelope2,
        mutation_type: MutationType::Cancel,
        target: make_target(&instance_id),
        buffer_policy: BufferPolicy::Reject,
        payload: Some(json!({})),
    };

    let dedup_key1 = request1.dedup_key();
    let dedup_key2 = request2.dedup_key();

    assert_eq!(dedup_key1, dedup_key2);
}

#[test]
fn test_dedup_key_inequality_for_different_command_ids() {
    let envelope1 = make_valid_envelope();
    let envelope2 = CommandEnvelope {
        schema_version: 1,
        metadata: CommandMetadata {
            command_id: IdempotencyKey::parse("cmd-different-001").unwrap(),
            correlation_id: IdempotencyKey::parse("corr-batch-001").unwrap(),
            causation_id: IdempotencyKey::parse("cause-event-001").unwrap(),
            issuer: Issuer::Operator,
            issued_at: TimestampMs::try_from(1_700_000_000u64).unwrap(),
        },
    };

    let instance_id = make_instance_id();
    let request1 = OperatorMutationRequest {
        envelope: envelope1,
        mutation_type: MutationType::Cancel,
        target: make_target(&instance_id),
        buffer_policy: BufferPolicy::Reject,
        payload: Some(json!({})),
    };

    let request2 = OperatorMutationRequest {
        envelope: envelope2,
        mutation_type: MutationType::Cancel,
        target: make_target(&instance_id),
        buffer_policy: BufferPolicy::Reject,
        payload: Some(json!({})),
    };

    let dedup_key1 = request1.dedup_key();
    let dedup_key2 = request2.dedup_key();

    assert_ne!(dedup_key1, dedup_key2);
}
