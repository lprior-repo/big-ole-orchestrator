#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

//! types.rs - HTTP API request/response types
//!
//! Per ADR-012, these types define the HTTP API contract.
//! Implements Data→Calc→Actions pattern with compile-time validation.

use std::num::NonZeroU64;

use chrono::{DateTime, Utc};
use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

// ============================================================================
// NEW TYPES (Data Layer)
// ============================================================================

/// Compile-time enforcement for workflow_name pattern `[a-z][a-z0-9_]*`
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WorkflowName(String);

impl WorkflowName {
    pub fn new(s: impl AsRef<str>) -> Result<Self, ParseError> {
        let s = s.as_ref();
        if s.is_empty() {
            return Err(ParseError::EmptyWorkflowName);
        }
        if !workflow_name_regex().is_match(s) {
            return Err(ParseError::InvalidWorkflowNameFormat);
        }
        Ok(Self(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for WorkflowName {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for WorkflowName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::new(&s).map_err(|e| D::Error::custom(e.to_string()))
    }
}

// Each regex helper uses #[allow] because the literals are compile-time constants
// that are always valid — the unwrap can never fail.
#[allow(clippy::unwrap_used)]
fn workflow_name_regex() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"^[a-z][a-z0-9_]*$").unwrap())
}

/// Compile-time enforcement for signal_name pattern `[a-z][a-z0-9_]+`
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SignalName(String);

impl SignalName {
    pub fn new(s: impl AsRef<str>) -> Result<Self, ParseError> {
        let s = s.as_ref();
        if s.is_empty() {
            return Err(ParseError::EmptySignalName);
        }
        if !signal_name_regex().is_match(s) {
            return Err(ParseError::InvalidSignalNameFormat);
        }
        Ok(Self(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for SignalName {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for SignalName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::new(&s).map_err(|e| D::Error::custom(e.to_string()))
    }
}

#[allow(clippy::unwrap_used)]
fn signal_name_regex() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"^[a-z][a-z0-9_]+$").unwrap())
}

/// Compile-time enforcement for ULID format (26 chars, Crockford base32)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvocationId(String);

impl InvocationId {
    pub fn from_str(s: impl AsRef<str>) -> Result<Self, ParseError> {
        let s = s.as_ref();
        if s.len() != 26 {
            return Err(ParseError::InvalidUlidFormat);
        }
        if !ulid_regex().is_match(s) {
            return Err(ParseError::InvalidUlidFormat);
        }
        Ok(Self(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for InvocationId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for InvocationId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::from_str(&s).map_err(|e| D::Error::custom(e.to_string()))
    }
}

#[allow(clippy::unwrap_used)]
fn ulid_regex() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"^[0-9A-HJKMNP-TV-Z]{26}$").unwrap())
}

/// Compile-time enforcement for retry_after_seconds > 0
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetryAfterSeconds(NonZeroU64);

impl RetryAfterSeconds {
    pub fn new(seconds: u64) -> Result<Self, ValidationError> {
        NonZeroU64::new(seconds)
            .map(Self)
            .ok_or(ValidationError::InvalidRetryAfterSeconds)
    }

    pub fn get(&self) -> u64 {
        self.0.get()
    }
}

impl Serialize for RetryAfterSeconds {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(self.0.get())
    }
}

impl<'de> Deserialize<'de> for RetryAfterSeconds {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = u64::deserialize(deserializer)?;
        Self::new(s).map_err(|e| D::Error::custom(e.to_string()))
    }
}

/// RFC3339 timestamp wrapper
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Timestamp(String);

impl Timestamp {
    pub fn new(s: impl AsRef<str>) -> Result<Self, ParseError> {
        let s = s.as_ref();
        DateTime::parse_from_rfc3339(s).map_err(|_| ParseError::InvalidTimestampFormat)?;
        Ok(Self(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the parsed `DateTime<Utc>`, or `None` if the stored string is not valid RFC3339.
    ///
    /// In practice this is always `Some` because `Timestamp::new` validates the string,
    /// but we return `Option` to avoid an infallible-looking `expect`.
    #[must_use]
    pub fn as_datetime(&self) -> Option<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(&self.0)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::new(&s).map_err(|e| D::Error::custom(e.to_string()))
    }
}

// ============================================================================
// ERROR TYPES (Calculations Layer)
// ============================================================================

/// Parse errors for invalid input format
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParseError {
    #[error("workflow_name is empty string")]
    EmptyWorkflowName,
    #[error("workflow_name does not match pattern [a-z][a-z0-9_]*")]
    InvalidWorkflowNameFormat,
    #[error("signal_name is empty string")]
    EmptySignalName,
    #[error("signal_name does not match pattern [a-z][a-z0-9_]+")]
    InvalidSignalNameFormat,
    #[error("invocation_id is not valid 26-char Crockford base32")]
    InvalidUlidFormat,
    #[error("timestamp is not valid RFC3339")]
    InvalidTimestampFormat,
    #[error("unknown status variant")]
    UnknownStatusVariant,
}

/// Validation errors for business rule violations
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ValidationError {
    #[error("retry_after_seconds must be > 0")]
    InvalidRetryAfterSeconds,
    #[error("invalid status transition")]
    InvalidStatusTransition,
    #[error("current_step is inconsistent with status")]
    InvalidCurrentStep,
}

/// Invariant violations for postcondition failures
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InvariantViolation {
    #[error("updated_at timestamp precedes started_at")]
    UpdatedBeforeStarted,
    #[error("journal entries not in ascending seq order")]
    EntriesNotSorted,
    #[error("retry_after_seconds set for non-retryable error")]
    InvalidRetryForErrorType,
    #[error("invocation_id is immutable")]
    InvocationIdModified,
    #[error("status must be 'running' for StartWorkflowResponse")]
    InvalidStatusForResponse,
}

// ============================================================================
// REQUEST TYPES (Data Layer)
// ============================================================================

/// Request to start a new workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartWorkflowRequest {
    pub workflow_name: WorkflowName,
    pub input: serde_json::Value,
}

/// Request to send a signal to a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalRequest {
    pub signal_name: SignalName,
    pub payload: serde_json::Value,
}

// ============================================================================
// RESPONSE TYPES (Data Layer)
// ============================================================================

/// Workflow status value enum
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowStatusValue {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Response after starting a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartWorkflowResponse {
    pub invocation_id: InvocationId,
    pub workflow_name: String,
    pub status: WorkflowStatusValue,
    pub started_at: Timestamp,
}

impl StartWorkflowResponse {
    pub fn validate(&self) -> Result<(), InvariantViolation> {
        if self.status != WorkflowStatusValue::Running {
            return Err(InvariantViolation::InvalidStatusForResponse);
        }
        Ok(())
    }
}

/// Detailed workflow status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStatus {
    pub invocation_id: InvocationId,
    pub workflow_name: String,
    pub status: WorkflowStatusValue,
    pub current_step: u32,
    pub started_at: Timestamp,
    pub updated_at: Timestamp,
}

impl WorkflowStatus {
    pub fn validate(&self) -> Result<(), InvariantViolation> {
        let chronologically_invalid =
            match (self.updated_at.as_datetime(), self.started_at.as_datetime()) {
                (Some(updated), Some(started)) => updated < started,
                _ => true,
            };
        if chronologically_invalid {
            return Err(InvariantViolation::UpdatedBeforeStarted);
        }
        Ok(())
    }
}

/// Response to a signal request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalResponse {
    pub acknowledged: bool,
}

/// Journal entry type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum JournalEntryType {
    Run,
    Wait,
}

/// Journal entry for workflow history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub seq: u32,
    #[serde(flatten)]
    pub entry_type: JournalEntryType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fire_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// Response containing workflow journal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalResponse {
    pub invocation_id: InvocationId,
    pub entries: Vec<JournalEntry>,
}

impl JournalResponse {
    pub fn validate(&self) -> Result<(), InvariantViolation> {
        let seqs = self.entries.iter().map(|e| e.seq);
        if !is_sorted(seqs) {
            return Err(InvariantViolation::EntriesNotSorted);
        }
        Ok(())
    }
}

/// Response containing list of running workflows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListWorkflowsResponse {
    pub workflows: Vec<WorkflowStatus>,
}

// ============================================================================
// DEFINITION INGESTION TYPES (bead wtf-qyxl)
// ============================================================================

/// POST /api/v1/definitions/<type> request body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefinitionRequest {
    pub source: String,
}

/// Diagnostic from the linter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticDto {
    pub code: String,
    pub severity: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<(usize, usize)>,
}

/// Response to POST /api/v1/definitions/<type>.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefinitionResponse {
    pub valid: bool,
    pub diagnostics: Vec<DiagnosticDto>,
}

// ============================================================================
// V3 API REQUEST/RESPONSE TYPES (bead wtf-bjn0)
// ============================================================================

/// POST /api/v1/workflows request body.
///
/// Starts a new workflow instance. If `instance_id` is `None`, the engine
/// generates a ULID automatically.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V3StartRequest {
    /// Namespace the instance should run in (e.g. `"payments"`).
    pub namespace: String,
    /// Workflow type name (selects the execution logic).
    pub workflow_type: String,
    /// Execution paradigm: `"fsm"`, `"dag"`, or `"procedural"`.
    pub paradigm: String,
    /// JSON-encoded input passed to the workflow on first start.
    pub input: serde_json::Value,
    /// Optional stable ID. If omitted, a ULID is generated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
}

/// Response to POST /api/v1/workflows on success (HTTP 201).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V3StartResponse {
    pub instance_id: String,
    pub namespace: String,
    pub workflow_type: String,
}

/// Response to GET /api/v1/workflows/:id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V3StatusResponse {
    pub instance_id: String,
    pub namespace: String,
    pub workflow_type: String,
    /// `"fsm"`, `"dag"`, or `"procedural"`.
    pub paradigm: String,
    /// `"replay"` or `"live"`.
    pub phase: String,
    pub events_applied: u64,
}

/// POST /api/v1/workflows/:id/signals request body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V3SignalRequest {
    pub signal_name: String,
    pub payload: serde_json::Value,
}

/// Generic API error response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub error: String,
    pub message: String,
}

impl ApiError {
    #[must_use]
    pub fn new(error: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            message: message.into(),
        }
    }
}

/// API error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_seconds: Option<RetryAfterSeconds>,
}

impl ErrorResponse {
    pub fn new(
        error: impl Into<String>,
        message: impl Into<String>,
        retry_after: Option<RetryAfterSeconds>,
    ) -> Result<Self, InvariantViolation> {
        let error_str = error.into();
        let is_retryable = is_retryable_error(&error_str);
        let has_retry = retry_after.is_some();
        if is_retryable && !has_retry {
            return Err(InvariantViolation::InvalidRetryForErrorType);
        }
        if !is_retryable && has_retry {
            return Err(InvariantViolation::InvalidRetryForErrorType);
        }
        Ok(Self {
            error: error_str,
            message: message.into(),
            retry_after_seconds: retry_after,
        })
    }
}

// ============================================================================
// HELPER FUNCTIONS (Calculations Layer)
// ============================================================================

fn is_retryable_error(error: &str) -> bool {
    matches!(error, "at_capacity")
}

fn is_sorted<T: PartialOrd + Clone>(mut iter: impl Iterator<Item = T>) -> bool {
    let mut prev = match iter.next() {
        Some(v) => v,
        None => return true,
    };
    iter.all(|curr| {
        let result = prev <= curr;
        prev = curr;
        result
    })
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_name_valid() {
        let cases = ["a", "checkout", "order_2_process", "abc123"];
        for name in cases {
            assert!(
                WorkflowName::new(name).is_ok(),
                "Expected {name} to be valid"
            );
        }
    }

    #[test]
    fn test_workflow_name_invalid() {
        let cases = ["", "Invalid", "1order", "order-name"];
        for name in cases {
            let result = WorkflowName::new(name);
            assert!(result.is_err(), "Expected {name} to be invalid");
        }
    }

    #[test]
    fn test_signal_name_valid() {
        let cases = ["payment_approved", "cancel", "signal_2"];
        for name in cases {
            assert!(SignalName::new(name).is_ok(), "Expected {name} to be valid");
        }
    }

    #[test]
    fn test_signal_name_invalid() {
        let cases = ["", "a", "Invalid", "signal-name"];
        for name in cases {
            let result = SignalName::new(name);
            assert!(result.is_err(), "Expected {name} to be invalid");
        }
    }

    #[test]
    fn test_invocation_id_valid() {
        let result = InvocationId::from_str("01ARZ3NDEKTSV4RRFFQ69G5FAV");
        assert!(result.is_ok(), "Valid ULID should pass");
    }

    #[test]
    fn test_invocation_id_invalid() {
        let cases = [
            "",
            "x",
            "01ARZ3NDEKTSV4RRFFQ69G5FA",
            "01ARZ3NDEKTSV4RRFFQ69G5FAVX",
            "INVALID123",
        ];
        for id in cases {
            let result = InvocationId::from_str(id);
            assert!(result.is_err(), "Expected {id} to be invalid");
        }
    }

    #[test]
    fn test_retry_after_seconds_valid() {
        let result = RetryAfterSeconds::new(5);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().get(), 5);
    }

    #[test]
    fn test_retry_after_seconds_zero_invalid() {
        let result = RetryAfterSeconds::new(0);
        assert!(result.is_err());
    }

    #[test]
    fn test_timestamp_valid() {
        let cases = ["2024-01-15T10:30:00Z", "2024-01-15T10:30:00+05:00"];
        for ts in cases {
            let result = Timestamp::new(ts);
            assert!(result.is_ok(), "Expected {ts} to be valid");
        }
    }

    #[test]
    fn test_timestamp_invalid() {
        let cases = ["invalid", "2024-13-45T99:99:99Z"];
        for ts in cases {
            let result = Timestamp::new(ts);
            assert!(result.is_err(), "Expected {ts} to be invalid");
        }
    }

    #[test]
    fn test_workflow_status_validate_timestamps() {
        let started = Timestamp::new("2024-01-15T10:31:00Z").unwrap();
        let updated_before = Timestamp::new("2024-01-15T10:30:00Z").unwrap();
        let updated_after = Timestamp::new("2024-01-15T10:32:00Z").unwrap();

        let status_before = WorkflowStatus {
            invocation_id: InvocationId::from_str("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap(),
            workflow_name: "test".to_string(),
            status: WorkflowStatusValue::Running,
            current_step: 0,
            started_at: started.clone(),
            updated_at: updated_before,
        };
        assert!(status_before.validate().is_err());

        let status_after = WorkflowStatus {
            invocation_id: InvocationId::from_str("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap(),
            workflow_name: "test".to_string(),
            status: WorkflowStatusValue::Running,
            current_step: 0,
            started_at: started,
            updated_at: updated_after,
        };
        assert!(status_after.validate().is_ok());
    }

    #[test]
    fn test_journal_response_validate_sorted() {
        let invocation_id = InvocationId::from_str("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();

        let unsorted = JournalResponse {
            invocation_id: invocation_id.clone(),
            entries: vec![
                JournalEntry {
                    seq: 1,
                    entry_type: JournalEntryType::Run,
                    name: Some("first".to_string()),
                    input: None,
                    output: None,
                    timestamp: None,
                    duration_ms: None,
                    fire_at: None,
                    status: None,
                },
                JournalEntry {
                    seq: 0,
                    entry_type: JournalEntryType::Run,
                    name: Some("second".to_string()),
                    input: None,
                    output: None,
                    timestamp: None,
                    duration_ms: None,
                    fire_at: None,
                    status: None,
                },
            ],
        };
        assert!(unsorted.validate().is_err());

        let sorted = JournalResponse {
            invocation_id,
            entries: vec![
                JournalEntry {
                    seq: 0,
                    entry_type: JournalEntryType::Run,
                    name: Some("first".to_string()),
                    input: None,
                    output: None,
                    timestamp: None,
                    duration_ms: None,
                    fire_at: None,
                    status: None,
                },
                JournalEntry {
                    seq: 1,
                    entry_type: JournalEntryType::Run,
                    name: Some("second".to_string()),
                    input: None,
                    output: None,
                    timestamp: None,
                    duration_ms: None,
                    fire_at: None,
                    status: None,
                },
            ],
        };
        assert!(sorted.validate().is_ok());
    }

    #[test]
    fn test_error_response_retryable_validation() {
        let retry = RetryAfterSeconds::new(5).unwrap();

        let err = ErrorResponse::new("at_capacity", "Capacity reached", Some(retry.clone()));
        assert!(err.is_ok(), "at_capacity with retry should be ok");

        let err = ErrorResponse::new("at_capacity", "Capacity reached", None);
        assert!(err.is_err(), "at_capacity without retry should fail");

        let err = ErrorResponse::new("not_found", "Not found", None);
        assert!(err.is_ok(), "not_found without retry should be ok");

        let err = ErrorResponse::new("not_found", "Not found", Some(retry));
        assert!(err.is_err(), "not_found with retry should fail");
    }

    #[test]
    fn test_start_workflow_response_validate() {
        let resp = StartWorkflowResponse {
            invocation_id: InvocationId::from_str("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap(),
            workflow_name: "checkout".to_string(),
            status: WorkflowStatusValue::Running,
            started_at: Timestamp::new("2024-01-15T10:30:00Z").unwrap(),
        };
        assert!(resp.validate().is_ok());

        let resp = StartWorkflowResponse {
            invocation_id: InvocationId::from_str("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap(),
            workflow_name: "checkout".to_string(),
            status: WorkflowStatusValue::Completed,
            started_at: Timestamp::new("2024-01-15T10:30:00Z").unwrap(),
        };
        assert!(resp.validate().is_err());
    }

    #[test]
    fn test_serde_roundtrip_start_workflow_request() {
        let request = StartWorkflowRequest {
            workflow_name: WorkflowName::new("checkout").unwrap(),
            input: serde_json::json!({ "order_id": "ord_123" }),
        };
        let json = serde_json::to_string(&request).unwrap();
        let deserialized: StartWorkflowRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(
            request.workflow_name.as_str(),
            deserialized.workflow_name.as_str()
        );
    }

    #[test]
    fn test_serde_roundtrip_signal_request() {
        let request = SignalRequest {
            signal_name: SignalName::new("payment_approved").unwrap(),
            payload: serde_json::json!({ "approved": true }),
        };
        let json = serde_json::to_string(&request).unwrap();
        let deserialized: SignalRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(
            request.signal_name.as_str(),
            deserialized.signal_name.as_str()
        );
    }

    #[test]
    fn test_serde_deserialize_invalid_workflow_name() {
        let json = r#"{"workflow_name": "Invalid-Name", "input": {}}"#;
        let result: Result<StartWorkflowRequest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_serde_deserialize_invalid_signal_name() {
        let json = r#"{"signal_name": "a", "payload": {}}"#;
        let result: Result<SignalRequest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_serde_deserialize_invalid_invocation_id() {
        let json = r#"{"invocation_id": "x", "workflow_name": "test", "status": "running", "started_at": "2024-01-15T10:30:00Z"}"#;
        let result: Result<StartWorkflowResponse, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }
}
