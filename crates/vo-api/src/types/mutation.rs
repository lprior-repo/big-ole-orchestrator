//! Operator mutation API types per ADR-036 and ADR-042.
//!
//! This module defines request/response types for operator-initiated mutations
//! (cancel, pause, resume, patch, retry, undo) with deduplication and
//! exact-safe error mapping.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use vo_types::BufferPolicy;
use vo_types::CommandEnvelope;
use vo_types::IdempotencyKey;
use vo_types::InstanceId;
use vo_types::SignalAddress;

// ---------------------------------------------------------------------------
// Mutation Type
// ---------------------------------------------------------------------------

/// The kind of operator-initiated mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationType {
    Cancel,
    Pause,
    Resume,
    Patch,
    Retry,
    Undo,
}

// ---------------------------------------------------------------------------
// Mutation Dedup Key
// ---------------------------------------------------------------------------

/// Deduplication key for operator mutations.
///
/// Combines instance_id, mutation_type, and command_id to uniquely identify
/// a mutation attempt. Two mutations with the same key are considered duplicates.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MutationDedupKey {
    instance_id: InstanceId,
    mutation_type: MutationType,
    command_id: IdempotencyKey,
}

impl MutationDedupKey {
    /// Create a new `MutationDedupKey`.
    #[must_use]
    pub fn new(
        instance_id: InstanceId,
        mutation_type: MutationType,
        command_id: IdempotencyKey,
    ) -> Self {
        Self {
            instance_id,
            mutation_type,
            command_id,
        }
    }

    #[must_use]
    pub fn instance_id(&self) -> &InstanceId {
        &self.instance_id
    }

    #[must_use]
    pub fn mutation_type(&self) -> MutationType {
        self.mutation_type
    }

    #[must_use]
    pub fn command_id(&self) -> &IdempotencyKey {
        &self.command_id
    }
}

// ---------------------------------------------------------------------------
// Mutation Request
// ---------------------------------------------------------------------------

/// Request body for an operator mutation (POST /api/v1/workflows/:id/mutations).
///
/// Carries a `CommandEnvelope` for identity, correlation, and causation tracking
/// per ADR-036. Includes a `SignalAddress` for routing and an optional `BufferPolicy`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorMutationRequest {
    pub envelope: CommandEnvelope,
    pub mutation_type: MutationType,
    pub target: SignalAddress,
    #[serde(default)]
    pub buffer_policy: BufferPolicy,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

impl OperatorMutationRequest {
    /// Extract the deduplication key for this request.
    #[must_use]
    pub fn dedup_key(&self) -> MutationDedupKey {
        MutationDedupKey::new(
            self.target.instance_id().clone(),
            self.mutation_type,
            self.envelope.metadata.command_id.clone(),
        )
    }

    /// Returns the correlation ID from the envelope metadata.
    #[must_use]
    pub fn correlation_id(&self) -> &IdempotencyKey {
        &self.envelope.metadata.correlation_id
    }

    /// Returns the causation ID from the envelope metadata.
    #[must_use]
    pub fn causation_id(&self) -> &IdempotencyKey {
        &self.envelope.metadata.causation_id
    }
}

// ---------------------------------------------------------------------------
// Mutation Rejection Reason
// ---------------------------------------------------------------------------

/// Why a mutation was rejected.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MutationRejectionReason {
    #[error("instance not found: {0}")]
    InstanceNotFound(String),
    #[error("lineage is tombstoned: {0}")]
    LineageTombstoned(String),
    #[error("invalid mutation for current state: {0}")]
    InvalidMutationForState(String),
    #[error("mutation payload validation failed: {0}")]
    PayloadValidationFailed(String),
    #[error("command envelope version unsupported")]
    UnsupportedEnvelopeVersion,
}

// ---------------------------------------------------------------------------
// Mutation Response
// ---------------------------------------------------------------------------

/// Response to an operator mutation request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum OperatorMutationResponse {
    /// Mutation was accepted and will be processed.
    Accepted {
        correlation_id: String,
        causation_id: String,
    },
    /// Mutation was detected as a duplicate (same dedup key).
    Duplicate {
        correlation_id: String,
        causation_id: String,
        original_command_id: String,
    },
    /// Mutation was rejected.
    Rejected { reason: String },
}

// ---------------------------------------------------------------------------
// Mutation Error
// ---------------------------------------------------------------------------

/// Errors that can occur during mutation processing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MutationError {
    #[error("envelope validation failed: {0}")]
    EnvelopeValidation(String),
    #[error("target address validation failed: {0}")]
    TargetValidation(String),
    #[error("{0}")]
    Rejected(MutationRejectionReason),
}

impl From<MutationRejectionReason> for MutationError {
    fn from(reason: MutationRejectionReason) -> Self {
        Self::Rejected(reason)
    }
}

// ---------------------------------------------------------------------------
// Error mapping
// ---------------------------------------------------------------------------

/// Map a `MutationError` to an HTTP status code.
#[must_use]
pub fn mutation_error_status_code(err: &MutationError) -> u16 {
    match err {
        MutationError::EnvelopeValidation(_) => 400,
        MutationError::TargetValidation(_) => 400,
        MutationError::Rejected(reason) => match reason {
            MutationRejectionReason::InstanceNotFound(_) => 404,
            MutationRejectionReason::LineageTombstoned(_) => 410,
            MutationRejectionReason::InvalidMutationForState(_) => 409,
            MutationRejectionReason::PayloadValidationFailed(_) => 422,
            MutationRejectionReason::UnsupportedEnvelopeVersion => 400,
        },
    }
}

/// Map a `MutationRejectionReason` to a human-readable error code string.
#[must_use]
pub fn rejection_error_code(reason: &MutationRejectionReason) -> &'static str {
    match reason {
        MutationRejectionReason::InstanceNotFound(_) => "instance_not_found",
        MutationRejectionReason::LineageTombstoned(_) => "lineage_tombstoned",
        MutationRejectionReason::InvalidMutationForState(_) => "invalid_mutation_for_state",
        MutationRejectionReason::PayloadValidationFailed(_) => "payload_validation_failed",
        MutationRejectionReason::UnsupportedEnvelopeVersion => "unsupported_envelope_version",
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use vo_types::{CommandMetadata, Issuer, TimestampMs};

    fn test_envelope() -> CommandEnvelope {
        CommandEnvelope {
            schema_version: 1,
            metadata: CommandMetadata {
                command_id: IdempotencyKey::parse("cmd-mut-001").expect("valid command_id"),
                correlation_id: IdempotencyKey::parse("corr-mut-001")
                    .expect("valid correlation_id"),
                causation_id: IdempotencyKey::parse("cause-mut-001").expect("valid causation_id"),
                issuer: Issuer::Operator,
                issued_at: TimestampMs::try_from(1_700_000_000u64).expect("valid timestamp"),
            },
        }
    }

    fn test_instance_id() -> InstanceId {
        InstanceId::parse("01HQXK5R5TJRP3J4W5G6W7Y8Z9").expect("valid ulid")
    }

    // --- MutationType serialization ---

    #[test]
    fn mutation_type_cancel_serializes_as_snake_case() {
        assert_eq!(
            serde_json::to_string(&MutationType::Cancel).unwrap(),
            r#""cancel""#
        );
    }

    #[test]
    fn mutation_type_pause_serializes_as_snake_case() {
        assert_eq!(
            serde_json::to_string(&MutationType::Pause).unwrap(),
            r#""pause""#
        );
    }

    #[test]
    fn mutation_type_resume_serializes_as_snake_case() {
        assert_eq!(
            serde_json::to_string(&MutationType::Resume).unwrap(),
            r#""resume""#
        );
    }

    #[test]
    fn mutation_type_patch_serializes_as_snake_case() {
        assert_eq!(
            serde_json::to_string(&MutationType::Patch).unwrap(),
            r#""patch""#
        );
    }

    #[test]
    fn mutation_type_retry_serializes_as_snake_case() {
        assert_eq!(
            serde_json::to_string(&MutationType::Retry).unwrap(),
            r#""retry""#
        );
    }

    #[test]
    fn mutation_type_undo_serializes_as_snake_case() {
        assert_eq!(
            serde_json::to_string(&MutationType::Undo).unwrap(),
            r#""undo""#
        );
    }

    #[test]
    fn mutation_type_roundtrips_through_serde() {
        for variant in [
            MutationType::Cancel,
            MutationType::Pause,
            MutationType::Resume,
            MutationType::Patch,
            MutationType::Retry,
            MutationType::Undo,
        ] {
            let json = serde_json::to_string(&variant).expect("serialize");
            let back: MutationType = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, variant);
        }
    }

    // --- MutationDedupKey ---

    #[test]
    fn mutation_dedup_key_equality_same_values() {
        let id = test_instance_id();
        let cmd = IdempotencyKey::parse("cmd-1").expect("valid");
        let a = MutationDedupKey::new(id.clone(), MutationType::Cancel, cmd.clone());
        let b = MutationDedupKey::new(id.clone(), MutationType::Cancel, cmd);
        assert_eq!(a, b);
    }

    #[test]
    fn mutation_dedup_key_inequality_different_type() {
        let id = test_instance_id();
        let cmd = IdempotencyKey::parse("cmd-1").expect("valid");
        let a = MutationDedupKey::new(id.clone(), MutationType::Cancel, cmd.clone());
        let b = MutationDedupKey::new(id, MutationType::Pause, cmd);
        assert_ne!(a, b);
    }

    #[test]
    fn mutation_dedup_key_inequality_different_command() {
        let id = test_instance_id();
        let a = MutationDedupKey::new(
            id.clone(),
            MutationType::Cancel,
            IdempotencyKey::parse("cmd-a").expect("valid"),
        );
        let b = MutationDedupKey::new(
            id,
            MutationType::Cancel,
            IdempotencyKey::parse("cmd-b").expect("valid"),
        );
        assert_ne!(a, b);
    }

    #[test]
    fn mutation_dedup_key_serde_roundtrip() {
        let key = MutationDedupKey::new(
            test_instance_id(),
            MutationType::Retry,
            IdempotencyKey::parse("cmd-serde").expect("valid"),
        );
        let json = serde_json::to_string(&key).expect("serialize");
        let back: MutationDedupKey = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, key);
    }

    // --- OperatorMutationRequest ---

    #[test]
    fn mutation_request_dedup_key_matches_envelope() {
        let envelope = test_envelope();
        let instance_id = test_instance_id();
        let target = SignalAddress::lineage_wide(
            instance_id.clone(),
            instance_id.clone(),
            vo_types::WaitKey::parse("approve").expect("valid"),
        );
        let req = OperatorMutationRequest {
            envelope,
            mutation_type: MutationType::Cancel,
            target,
            buffer_policy: BufferPolicy::Reject,
            payload: None,
        };
        let key = req.dedup_key();
        assert_eq!(key.mutation_type(), MutationType::Cancel);
        assert_eq!(key.command_id().as_str(), "cmd-mut-001");
    }

    #[test]
    fn mutation_request_extracts_correlation_and_causation_ids() {
        let req = OperatorMutationRequest {
            envelope: test_envelope(),
            mutation_type: MutationType::Resume,
            target: SignalAddress::lineage_wide(
                test_instance_id(),
                test_instance_id(),
                vo_types::WaitKey::parse("resume").expect("valid"),
            ),
            buffer_policy: BufferPolicy::Reject,
            payload: None,
        };
        assert_eq!(req.correlation_id().as_str(), "corr-mut-001");
        assert_eq!(req.causation_id().as_str(), "cause-mut-001");
    }

    #[test]
    fn mutation_request_serde_roundtrip() {
        let req = OperatorMutationRequest {
            envelope: test_envelope(),
            mutation_type: MutationType::Patch,
            target: SignalAddress::lineage_wide(
                test_instance_id(),
                test_instance_id(),
                vo_types::WaitKey::parse("patch-step").expect("valid"),
            ),
            buffer_policy: BufferPolicy::BufferOne,
            payload: Some(serde_json::json!({"timeout_ms": 5000})),
        };
        let json = serde_json::to_string(&req).expect("serialize");
        let back: OperatorMutationRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.mutation_type, MutationType::Patch);
        assert_eq!(back.buffer_policy, BufferPolicy::BufferOne);
        assert!(back.payload.is_some());
    }

    // --- OperatorMutationResponse ---

    #[test]
    fn mutation_response_accepted_serializes_correctly() {
        let resp = OperatorMutationResponse::Accepted {
            correlation_id: "corr-1".to_string(),
            causation_id: "cause-1".to_string(),
        };
        let json = serde_json::to_string(&resp).expect("serialize");
        assert!(json.contains(r#""status":"accepted""#));
        assert!(json.contains(r#""correlation_id":"corr-1""#));
    }

    #[test]
    fn mutation_response_duplicate_serializes_correctly() {
        let resp = OperatorMutationResponse::Duplicate {
            correlation_id: "corr-1".to_string(),
            causation_id: "cause-1".to_string(),
            original_command_id: "cmd-orig".to_string(),
        };
        let json = serde_json::to_string(&resp).expect("serialize");
        assert!(json.contains(r#""status":"duplicate""#));
        assert!(json.contains(r#""original_command_id":"cmd-orig""#));
    }

    #[test]
    fn mutation_response_rejected_serializes_correctly() {
        let resp = OperatorMutationResponse::Rejected {
            reason: "instance not found".to_string(),
        };
        let json = serde_json::to_string(&resp).expect("serialize");
        assert!(json.contains(r#""status":"rejected""#));
        assert!(json.contains(r#""reason":"instance not found""#));
    }

    #[test]
    fn mutation_response_roundtrips_through_serde() {
        for resp in [
            OperatorMutationResponse::Accepted {
                correlation_id: "c".to_string(),
                causation_id: "k".to_string(),
            },
            OperatorMutationResponse::Duplicate {
                correlation_id: "c".to_string(),
                causation_id: "k".to_string(),
                original_command_id: "o".to_string(),
            },
            OperatorMutationResponse::Rejected {
                reason: "r".to_string(),
            },
        ] {
            let json = serde_json::to_string(&resp).expect("serialize");
            let back: OperatorMutationResponse = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, resp);
        }
    }

    // --- MutationRejectionReason ---

    #[test]
    fn rejection_reason_instance_not_found() {
        let reason = MutationRejectionReason::InstanceNotFound("inst-1".to_string());
        assert!(reason.to_string().contains("instance not found"));
        assert_eq!(rejection_error_code(&reason), "instance_not_found");
    }

    #[test]
    fn rejection_reason_lineage_tombstoned() {
        let reason = MutationRejectionReason::LineageTombstoned("lineage-1".to_string());
        assert!(reason.to_string().contains("lineage is tombstoned"));
        assert_eq!(rejection_error_code(&reason), "lineage_tombstoned");
    }

    #[test]
    fn rejection_reason_invalid_mutation_for_state() {
        let reason =
            MutationRejectionReason::InvalidMutationForState("cannot pause completed".to_string());
        assert!(reason
            .to_string()
            .contains("invalid mutation for current state"));
        assert_eq!(rejection_error_code(&reason), "invalid_mutation_for_state");
    }

    #[test]
    fn rejection_reason_payload_validation_failed() {
        let reason = MutationRejectionReason::PayloadValidationFailed("missing field".to_string());
        assert!(reason.to_string().contains("payload validation failed"));
        assert_eq!(rejection_error_code(&reason), "payload_validation_failed");
    }

    #[test]
    fn rejection_reason_unsupported_envelope_version() {
        let reason = MutationRejectionReason::UnsupportedEnvelopeVersion;
        assert!(reason.to_string().contains("unsupported"));
        assert_eq!(
            rejection_error_code(&reason),
            "unsupported_envelope_version"
        );
    }

    // --- MutationError and status code mapping ---

    #[test]
    fn mutation_error_envelope_validation_maps_to_400() {
        let err = MutationError::EnvelopeValidation("bad json".to_string());
        assert_eq!(mutation_error_status_code(&err), 400);
    }

    #[test]
    fn mutation_error_target_validation_maps_to_400() {
        let err = MutationError::TargetValidation("bad address".to_string());
        assert_eq!(mutation_error_status_code(&err), 400);
    }

    #[test]
    fn mutation_error_instance_not_found_maps_to_404() {
        let err =
            MutationError::Rejected(MutationRejectionReason::InstanceNotFound("x".to_string()));
        assert_eq!(mutation_error_status_code(&err), 404);
    }

    #[test]
    fn mutation_error_lineage_tombstoned_maps_to_410() {
        let err =
            MutationError::Rejected(MutationRejectionReason::LineageTombstoned("x".to_string()));
        assert_eq!(mutation_error_status_code(&err), 410);
    }

    #[test]
    fn mutation_error_invalid_mutation_for_state_maps_to_409() {
        let err = MutationError::Rejected(MutationRejectionReason::InvalidMutationForState(
            "x".to_string(),
        ));
        assert_eq!(mutation_error_status_code(&err), 409);
    }

    #[test]
    fn mutation_error_payload_validation_maps_to_422() {
        let err = MutationError::Rejected(MutationRejectionReason::PayloadValidationFailed(
            "x".to_string(),
        ));
        assert_eq!(mutation_error_status_code(&err), 422);
    }

    #[test]
    fn mutation_error_unsupported_version_maps_to_400() {
        let err = MutationError::Rejected(MutationRejectionReason::UnsupportedEnvelopeVersion);
        assert_eq!(mutation_error_status_code(&err), 400);
    }

    #[test]
    fn mutation_rejection_converts_to_mutation_error() {
        let reason = MutationRejectionReason::InstanceNotFound("inst".to_string());
        let err: MutationError = reason.clone().into();
        assert_eq!(err, MutationError::Rejected(reason));
    }
}
