use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use vo_types::signal::{BufferPolicy, SignalAddress};
use vo_types::{CommandEnvelope, IdempotencyKey};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationType {
    Cancel,
    Pause,
    Resume,
    Patch,
    Retry,
    Undo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "data")]
pub enum OperatorMutationResponse {
    Accepted {
        instance_id: String,
        mutation_type: MutationType,
        command_id: IdempotencyKey,
        accepted_at: DateTime<Utc>,
    },
    Duplicate {
        instance_id: String,
        mutation_type: MutationType,
        command_id: IdempotencyKey,
        original_accepted_at: DateTime<Utc>,
        message: String,
    },
    Rejected {
        reason: MutationRejectionReason,
        command_id: Option<IdempotencyKey>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationRejectionReason {
    InstanceNotFound,
    InstanceNotSuspended,
    InstanceNotRunning,
    InvalidMutationForState,
    LineageTombstoned,
    EpochNoLongerActive,
    CommandIdExhausted,
    InternalError(String),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MutationError {
    #[error("instance not found: {0}")]
    InstanceNotFound(String),
    #[error("instance not in suspended state")]
    InstanceNotSuspended,
    #[error("instance not in running state")]
    InstanceNotRunning,
    #[error("invalid mutation {mutation_type:?} for current state")]
    InvalidMutationForState { mutation_type: MutationType },
    #[error("lineage has been tombstoned")]
    LineageTombstoned,
    #[error("targeted epoch is no longer active")]
    EpochNoLongerActive,
    #[error("command ID already used: {0}")]
    CommandIdExhausted(String),
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperatorMutationRequest {
    pub instance_id: String,
    pub lineage_id: String,
    pub mutation_type: MutationType,
    pub payload: serde_json::Value,
    pub command_envelope: CommandEnvelope,
    pub signal_address: Option<SignalAddress>,
    pub buffer_policy: BufferPolicy,
}

impl OperatorMutationRequest {
    pub fn command_id(&self) -> &IdempotencyKey {
        &self.command_envelope.metadata.command_id
    }

    pub fn correlation_id(&self) -> &IdempotencyKey {
        &self.command_envelope.metadata.correlation_id
    }

    pub fn causation_id(&self) -> &IdempotencyKey {
        &self.command_envelope.metadata.causation_id
    }

    pub fn issuer(&self) -> vo_types::Issuer {
        self.command_envelope.metadata.issuer
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationDedupKey {
    pub instance_id: String,
    pub lineage_id: String,
    pub command_id: IdempotencyKey,
    pub mutation_type: MutationType,
}

impl MutationDedupKey {
    pub fn from_request(request: &OperatorMutationRequest) -> Self {
        Self {
            instance_id: request.instance_id.clone(),
            lineage_id: request.lineage_id.clone(),
            command_id: request.command_envelope.metadata.command_id.clone(),
            mutation_type: request.mutation_type.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vo_types::{CommandMetadata, Issuer, TimestampMs};

    fn make_command_envelope() -> CommandEnvelope {
        let metadata = CommandMetadata {
            command_id: IdempotencyKey::parse("cmd-mutation-001").unwrap(),
            correlation_id: IdempotencyKey::parse("corr-batch-001").unwrap(),
            causation_id: IdempotencyKey::parse("cause-operator-001").unwrap(),
            issuer: Issuer::Operator,
            issued_at: TimestampMs::try_from(1_700_000_000u64).unwrap(),
        };
        CommandEnvelope {
            schema_version: 1,
            metadata,
        }
    }

    #[test]
    fn mutation_request_extracts_command_identity() {
        let envelope = make_command_envelope();
        let request = OperatorMutationRequest {
            instance_id: "instance-1".to_string(),
            lineage_id: "lineage-1".to_string(),
            mutation_type: MutationType::Cancel,
            payload: serde_json::json!({}),
            command_envelope: envelope,
            signal_address: None,
            buffer_policy: BufferPolicy::Reject,
        };

        assert_eq!(request.command_id().as_str(), "cmd-mutation-001");
        assert_eq!(request.correlation_id().as_str(), "corr-batch-001");
        assert_eq!(request.causation_id().as_str(), "cause-operator-001");
        assert!(matches!(request.issuer(), Issuer::Operator));
    }

    #[test]
    fn dedup_key_from_request() {
        let envelope = make_command_envelope();
        let request = OperatorMutationRequest {
            instance_id: "instance-1".to_string(),
            lineage_id: "lineage-1".to_string(),
            mutation_type: MutationType::Pause,
            payload: serde_json::json!({}),
            command_envelope: envelope,
            signal_address: None,
            buffer_policy: BufferPolicy::Reject,
        };

        let dedup_key = MutationDedupKey::from_request(&request);
        assert_eq!(dedup_key.instance_id, "instance-1");
        assert_eq!(dedup_key.lineage_id, "lineage-1");
        assert_eq!(dedup_key.mutation_type, MutationType::Pause);
    }

    #[test]
    fn mutation_type_serde() {
        let cancel = MutationType::Cancel;
        let json = serde_json::to_string(&cancel).unwrap();
        assert_eq!(json, "\"cancel\"");

        let back: MutationType = serde_json::from_str(&json).unwrap();
        assert_eq!(back, MutationType::Cancel);
    }

    #[test]
    fn rejection_reason_serde() {
        let reason = MutationRejectionReason::InstanceNotFound;
        let json = serde_json::to_string(&reason).unwrap();
        assert!(json.contains("instance_not_found"));

        let back: MutationRejectionReason = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, MutationRejectionReason::InstanceNotFound));
    }
}
