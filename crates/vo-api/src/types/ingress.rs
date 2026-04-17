use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use vo_types::{DedupeKey, IdempotencyKey};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "data")]
pub enum IngressAdmissionResponse {
    Admitted {
        instance_id: String,
        dedup_key: DedupeKey,
        admitted_at: DateTime<Utc>,
    },
    Deduped {
        instance_id: String,
        dedup_key: DedupeKey,
        original_admitted_at: DateTime<Utc>,
        message: String,
    },
    Rejected {
        reason: DedupRejectionReason,
        dedup_key: Option<DedupeKey>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DedupRejectionReason {
    MissingDedupKey,
    InvalidDedupKeyFormat,
    DedupKeyExceedsMaxLength,
    WorkflowNotExact,
    InternalError(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DedupRecord {
    pub dedup_key: DedupeKey,
    pub instance_id: String,
    pub workflow_type: String,
    pub admitted_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub retention_window_seconds: u64,
}

impl DedupRecord {
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        now > self.expires_at
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngressAdmissionRequest {
    pub dedupe_key: Option<DedupeKey>,
    pub namespace: String,
    pub workflow_type: String,
    pub input: serde_json::Value,
    pub command_id: IdempotencyKey,
    pub correlation_id: IdempotencyKey,
    pub causation_id: IdempotencyKey,
    pub is_exact_workflow: bool,
}

impl IngressAdmissionRequest {
    pub fn requires_dedup(&self) -> bool {
        self.is_exact_workflow
    }

    pub fn validate_for_exact_workflow(&self) -> Result<DedupeKey, DedupRejectionReason> {
        self.dedupe_key
            .clone()
            .ok_or(DedupRejectionReason::MissingDedupKey)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedup_key_parse_valid() {
        let key = DedupeKey::parse("test-key-123").unwrap();
        assert_eq!(key.as_str(), "test-key-123");
    }

    #[test]
    fn dedup_key_parse_empty_error() {
        let result = DedupeKey::parse("");
        assert!(matches!(result, Err(vo_types::ParseError::Empty { .. })));
    }

    #[test]
    fn dedup_key_parse_exceeds_max_length() {
        let long_key = "a".repeat(300);
        let result = DedupeKey::parse(&long_key);
        assert!(matches!(
            result,
            Err(vo_types::ParseError::ExceedsMaxLength { max: 256, .. })
        ));
    }

    #[test]
    fn admission_request_requires_dedup_for_exact() {
        let request = IngressAdmissionRequest {
            dedupe_key: None,
            namespace: "ns".to_string(),
            workflow_type: "wf".to_string(),
            input: serde_json::json!({}),
            command_id: IdempotencyKey::parse("cmd-1").unwrap(),
            correlation_id: IdempotencyKey::parse("corr-1").unwrap(),
            causation_id: IdempotencyKey::parse("cause-1").unwrap(),
            is_exact_workflow: true,
        };
        assert!(request.requires_dedup());
        assert!(request.validate_for_exact_workflow().is_err());
    }

    #[test]
    fn admission_request_no_dedup_for_inexact() {
        let request = IngressAdmissionRequest {
            dedupe_key: None,
            namespace: "ns".to_string(),
            workflow_type: "wf".to_string(),
            input: serde_json::json!({}),
            command_id: IdempotencyKey::parse("cmd-1").unwrap(),
            correlation_id: IdempotencyKey::parse("corr-1").unwrap(),
            causation_id: IdempotencyKey::parse("cause-1").unwrap(),
            is_exact_workflow: false,
        };
        assert!(!request.requires_dedup());
    }
}
