use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use vo_types::IdempotencyKey;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DedupKey(String);

impl DedupKey {
    pub fn new(key: IdempotencyKey) -> Self {
        Self(key.as_str().to_string())
    }

    pub fn parse(input: &str) -> Result<Self, DedupError> {
        if input.is_empty() {
            return Err(DedupError::EmptyKey);
        }
        if input.len() > 1024 {
            return Err(DedupError::KeyExceedsMaxLength {
                max: 1024,
                actual: input.len(),
            });
        }
        Ok(Self(input.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "data", rename_all = "snake_case")]
pub enum IngressAdmissionResponse {
    Admitted {
        instance_id: String,
        dedup_key: DedupKey,
        admitted_at: DateTime<Utc>,
    },
    Deduped {
        instance_id: String,
        dedup_key: DedupKey,
        original_admitted_at: DateTime<Utc>,
        message: String,
    },
    Rejected {
        reason: DedupRejectionReason,
        dedup_key: Option<DedupKey>,
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

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DedupError {
    #[error("dedupe key is empty")]
    EmptyKey,
    #[error("dedupe key exceeds maximum length: max {max}, actual {actual}")]
    KeyExceedsMaxLength { max: usize, actual: usize },
    #[error("invalid dedupe key format")]
    InvalidFormat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DedupRecord {
    pub dedup_key: DedupKey,
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
    pub dedupe_key: Option<DedupKey>,
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

    pub fn validate_for_exact_workflow(&self) -> Result<DedupKey, DedupRejectionReason> {
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
        let key = DedupKey::parse("test-key-123").unwrap();
        assert_eq!(key.as_str(), "test-key-123");
    }

    #[test]
    fn dedup_key_parse_empty_error() {
        let result = DedupKey::parse("");
        assert!(matches!(result, Err(DedupError::EmptyKey)));
    }

    #[test]
    fn dedup_key_parse_exceeds_max_length() {
        let long_key = "a".repeat(1025);
        let result = DedupKey::parse(&long_key);
        assert!(matches!(
            result,
            Err(DedupError::KeyExceedsMaxLength { max: 1024, .. })
        ));
    }

    #[test]
    fn dedup_record_expired() {
        let past = DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let future = DateTime::parse_from_rfc3339("2030-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let record = DedupRecord {
            dedup_key: DedupKey::parse("test").unwrap(),
            instance_id: "instance-1".to_string(),
            workflow_type: "test-workflow".to_string(),
            admitted_at: past,
            expires_at: past,
            retention_window_seconds: 3600,
        };

        assert!(record.is_expired(future));
        assert!(!record.is_expired(past));
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
