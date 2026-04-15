#![allow(clippy::unwrap_used)]

use chrono::{DateTime, Utc};
use serde_json::json;
use vo_api::types::mutation::{
    MutationDedupKey, MutationRejectionReason, MutationType, OperatorMutationRequest,
    OperatorMutationResponse,
};
use vo_types::signal::BufferPolicy;
use vo_types::{CommandEnvelope, CommandMetadata, IdempotencyKey, Issuer, TimestampMs};

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

#[test]
fn test_duplicate_mutation_returns_dup_response() {
    let envelope = make_valid_envelope();
    let request = OperatorMutationRequest {
        instance_id: "instance-1".to_string(),
        lineage_id: "lineage-1".to_string(),
        mutation_type: MutationType::Cancel,
        payload: json!({}),
        command_envelope: envelope,
        signal_address: None,
        buffer_policy: BufferPolicy::Reject,
    };

    let dedup_key = MutationDedupKey::from_request(&request);

    let response = OperatorMutationResponse::Duplicate {
        instance_id: "instance-1".to_string(),
        mutation_type: MutationType::Cancel,
        command_id: dedup_key.command_id,
        original_accepted_at: DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
        message: "Mutation already applied".to_string(),
    };

    match response {
        OperatorMutationResponse::Duplicate { .. } => {}
        _ => panic!("Expected Duplicate response"),
    }
}

#[test]
fn test_mutation_rejected_for_unknown_instance() {
    let envelope = make_valid_envelope();
    let request = OperatorMutationRequest {
        instance_id: "nonexistent-instance".to_string(),
        lineage_id: "lineage-1".to_string(),
        mutation_type: MutationType::Cancel,
        payload: json!({}),
        command_envelope: envelope,
        signal_address: None,
        buffer_policy: BufferPolicy::Reject,
    };

    let response = OperatorMutationResponse::Rejected {
        reason: MutationRejectionReason::InstanceNotFound,
        command_id: Some(request.command_envelope.metadata.command_id.clone()),
    };

    match response {
        OperatorMutationResponse::Rejected {
            reason: MutationRejectionReason::InstanceNotFound,
            ..
        } => {}
        _ => panic!("Expected InstanceNotFound rejection"),
    }
}

#[test]
fn test_mutation_rejected_for_tombstoned_lineage() {
    let envelope = make_valid_envelope();
    let request = OperatorMutationRequest {
        instance_id: "instance-1".to_string(),
        lineage_id: "tombstoned-lineage".to_string(),
        mutation_type: MutationType::Resume,
        payload: json!({}),
        command_envelope: envelope,
        signal_address: None,
        buffer_policy: BufferPolicy::Reject,
    };

    let response = OperatorMutationResponse::Rejected {
        reason: MutationRejectionReason::LineageTombstoned,
        command_id: Some(request.command_envelope.metadata.command_id.clone()),
    };

    match response {
        OperatorMutationResponse::Rejected {
            reason: MutationRejectionReason::LineageTombstoned,
            ..
        } => {}
        _ => panic!("Expected LineageTombstoned rejection"),
    }
}

#[test]
fn test_invalid_mutation_for_running_instance() {
    let envelope = make_valid_envelope();
    let request = OperatorMutationRequest {
        instance_id: "instance-1".to_string(),
        lineage_id: "lineage-1".to_string(),
        mutation_type: MutationType::Resume,
        payload: json!({}),
        command_envelope: envelope,
        signal_address: None,
        buffer_policy: BufferPolicy::Reject,
    };

    let response = OperatorMutationResponse::Rejected {
        reason: MutationRejectionReason::InvalidMutationForState,
        command_id: Some(request.command_envelope.metadata.command_id.clone()),
    };

    match response {
        OperatorMutationResponse::Rejected {
            reason: MutationRejectionReason::InvalidMutationForState,
            ..
        } => {}
        _ => panic!("Expected InvalidMutationForState rejection"),
    }
}

#[test]
fn test_command_id_exhausted_rejection() {
    let envelope = make_valid_envelope();
    let request = OperatorMutationRequest {
        instance_id: "instance-1".to_string(),
        lineage_id: "lineage-1".to_string(),
        mutation_type: MutationType::Undo,
        payload: json!({}),
        command_envelope: envelope,
        signal_address: None,
        buffer_policy: BufferPolicy::Reject,
    };

    let response = OperatorMutationResponse::Rejected {
        reason: MutationRejectionReason::CommandIdExhausted,
        command_id: Some(request.command_envelope.metadata.command_id.clone()),
    };

    match response {
        OperatorMutationResponse::Rejected {
            reason: MutationRejectionReason::CommandIdExhausted,
            ..
        } => {}
        _ => panic!("Expected CommandIdExhausted rejection"),
    }
}

#[test]
fn test_epoch_no_longer_active_rejection() {
    let envelope = make_valid_envelope();
    let request = OperatorMutationRequest {
        instance_id: "instance-1".to_string(),
        lineage_id: "lineage-1".to_string(),
        mutation_type: MutationType::Patch,
        payload: json!({}),
        command_envelope: envelope,
        signal_address: None,
        buffer_policy: BufferPolicy::Reject,
    };

    let response = OperatorMutationResponse::Rejected {
        reason: MutationRejectionReason::EpochNoLongerActive,
        command_id: Some(request.command_envelope.metadata.command_id.clone()),
    };

    match response {
        OperatorMutationResponse::Rejected {
            reason: MutationRejectionReason::EpochNoLongerActive,
            ..
        } => {}
        _ => panic!("Expected EpochNoLongerActive rejection"),
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

    let request1 = OperatorMutationRequest {
        instance_id: "instance-1".to_string(),
        lineage_id: "lineage-1".to_string(),
        mutation_type: MutationType::Cancel,
        payload: json!({}),
        command_envelope: envelope1,
        signal_address: None,
        buffer_policy: BufferPolicy::Reject,
    };

    let request2 = OperatorMutationRequest {
        instance_id: "instance-1".to_string(),
        lineage_id: "lineage-1".to_string(),
        mutation_type: MutationType::Cancel,
        payload: json!({}),
        command_envelope: envelope2,
        signal_address: None,
        buffer_policy: BufferPolicy::Reject,
    };

    let dedup_key1 = MutationDedupKey::from_request(&request1);
    let dedup_key2 = MutationDedupKey::from_request(&request2);

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

    let request1 = OperatorMutationRequest {
        instance_id: "instance-1".to_string(),
        lineage_id: "lineage-1".to_string(),
        mutation_type: MutationType::Cancel,
        payload: json!({}),
        command_envelope: envelope1,
        signal_address: None,
        buffer_policy: BufferPolicy::Reject,
    };

    let request2 = OperatorMutationRequest {
        instance_id: "instance-1".to_string(),
        lineage_id: "lineage-1".to_string(),
        mutation_type: MutationType::Cancel,
        payload: json!({}),
        command_envelope: envelope2,
        signal_address: None,
        buffer_policy: BufferPolicy::Reject,
    };

    let dedup_key1 = MutationDedupKey::from_request(&request1);
    let dedup_key2 = MutationDedupKey::from_request(&request2);

    assert_ne!(dedup_key1, dedup_key2);
}
