use serde::{Deserialize, Serialize};

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
    /// Optional workflow binary/version hash supplied by live clients at the
    /// top level. Legacy clients may still send this inside `input`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_binary_hash: Option<String>,
    /// Optional stable ID. If omitted, a ULID is generated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
    /// Stable dedupe key for exactly-once ingress (ADR-028).
    /// Required for exact workflow ingress.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dedupe_key: Option<String>,
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
///
/// All API errors use this consistent structure with error code, message, and
/// optional details fields. The error code is a machine-readable identifier
/// that maps to specific HTTP status codes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    /// Machine-readable error code (e.g., "not_found", "invalid_input").
    pub error: String,
    /// Human-readable error message describing what went wrong.
    pub message: String,
    /// Optional additional context or structured details about the error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ApiError {
    #[must_use]
    pub fn new(error: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            message: message.into(),
            details: None,
        }
    }

    #[must_use]
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    #[must_use]
    pub fn status_code(&self) -> u16 {
        match self.error.as_str() {
            "not_found" | "workflow_not_found" | "instance_not_found" => 404,
            "already_exists" | "duplicate_ingress" | "conflict" => 409,
            "invalid_namespace" | "invalid_instance_id" | "invalid_workflow_type"
            | "invalid_input" | "invalid_dedupe_key" | "invalid_paradigm"
            | "missing_dedupe_key" | "unknown_status_variant" => 400,
            "workflow_quarantined" | "workflow_deactivated" | "forbidden" => 403,
            "budget_exhausted" | "writer_pressure_shed" | "workflow_cap_exceeded"
            | "too_many_requests" => 429,
            "global_concurrency_limit" | "at_capacity" | "service_unavailable" => 503,
            "internal_error" | "event_persist_failed" | "actor_unavailable"
            | "actor_timeout" | "actor_error" | "spawn_failed" | "event_replay_failed"
            | "dedupe_storage_error" | "terminate_failed" | "compensation_failed"
            | "ghost_instance" | "invalid_config" => 500,
            _ => 500,
        }
    }
}

/// Single entry in the timeline for a workflow instance (ADR-007).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub sequence: u64,
    pub timestamp_ms: u64,
    pub event_type: String,
    pub payload: serde_json::Value,
}

/// Response to GET /api/v1/workflows/:id/timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineResponse {
    pub instance_id: String,
    pub entries: Vec<TimelineEntry>,
    pub total_replayed: usize,
}

/// Single entry in the execution history for a step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub sequence: u64,
    pub timestamp_ms: u64,
    pub event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,
}

/// Pagination parameters for list endpoints.
#[derive(Debug, Clone, Deserialize)]
pub struct PaginationParams {
    #[serde(default)]
    pub offset: Option<usize>,
    #[serde(default)]
    pub limit: Option<usize>,
}

/// Response to GET /api/v1/workflows/:id/history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryResponse {
    pub instance_id: String,
    pub entries: Vec<HistoryEntry>,
    pub total_count: usize,
    pub offset: usize,
    pub limit: usize,
}

/// Canonical history entry with full event data for forensic inspection (ADR-008).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalHistoryEntry {
    pub sequence: u64,
    pub timestamp_ms: u64,
    pub schema_version: u8,
    pub event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,
    /// Full raw payload including secrets and encrypted fields (privileged access).
    pub raw_payload: serde_json::Value,
}

/// Canonical history response for deep forensic inspection (ADR-008).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalHistoryResponse {
    pub instance_id: String,
    pub entries: Vec<CanonicalHistoryEntry>,
    pub total_replayed: usize,
    pub warning: String,
}

/// Semantic guarantee class for an effect.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EffectSemantics {
    Exact,
    Unsafe,
}

/// Single entry in the effect journal (ADR-007).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectJournalEntry {
    pub sequence: u64,
    pub timestamp_ms: u64,
    pub event_type: String,
    pub semantics: EffectSemantics,
    pub payload: serde_json::Value,
}

/// Response to GET /api/v1/workflows/:id/effect-journal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectJournalResponse {
    pub instance_id: String,
    pub entries: Vec<EffectJournalEntry>,
}

/// Response to GET /api/v1/workflows/:id/version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowVersionResponse {
    pub instance_id: String,
    pub schema_version: u8,
    pub event_count: u64,
    pub last_sequence: Option<u64>,
    pub last_timestamp_ms: Option<u64>,
}

/// Search request body for full-text search across workspaces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub limit: Option<usize>,
}

/// Single search result entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultEntry {
    pub workspace_id: String,
    pub score: f64,
    pub matched_terms: Vec<String>,
}

/// Response to GET /api/v1/search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub query: String,
    pub results: Vec<SearchResultEntry>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn v3_start_request_skip_none_fields() {
        let req = V3StartRequest {
            namespace: "ns".to_string(),
            workflow_type: "wf".to_string(),
            paradigm: "fsm".to_string(),
            input: serde_json::json!({}),
            workflow_binary_hash: None,
            instance_id: None,
            dedupe_key: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("instance_id"));
        assert!(!json.contains("dedupe_key"));
    }

    #[test]
    fn v3_start_request_with_optional_fields() {
        let req = V3StartRequest {
            namespace: "ns".to_string(),
            workflow_type: "wf".to_string(),
            paradigm: "dag".to_string(),
            input: serde_json::json!({"key": "val"}),
            workflow_binary_hash: None,
            instance_id: Some("custom-id".to_string()),
            dedupe_key: Some("dedupe-1".to_string()),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("custom-id"));
        assert!(json.contains("dedupe-1"));
    }

    #[test]
    fn v3_start_request_roundtrip() {
        let req = V3StartRequest {
            namespace: "payments".to_string(),
            workflow_type: "charge".to_string(),
            paradigm: "procedural".to_string(),
            input: serde_json::json!({"amount": 100}),
            workflow_binary_hash: None,
            instance_id: None,
            dedupe_key: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: V3StartRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.namespace, "payments");
        assert_eq!(back.workflow_type, "charge");
    }

    #[test]
    fn v3_start_response_roundtrip() {
        let resp = V3StartResponse {
            instance_id: "abc123".to_string(),
            namespace: "ns".to_string(),
            workflow_type: "wf".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: V3StartResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back.instance_id, "abc123");
    }

    #[test]
    fn v3_status_response_roundtrip() {
        let resp = V3StatusResponse {
            instance_id: "id".to_string(),
            namespace: "ns".to_string(),
            workflow_type: "wf".to_string(),
            paradigm: "fsm".to_string(),
            phase: "live".to_string(),
            events_applied: 42,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: V3StatusResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back.events_applied, 42);
        assert_eq!(back.phase, "live");
    }

    #[test]
    fn v3_signal_request_roundtrip() {
        let req = V3SignalRequest {
            signal_name: "approve".to_string(),
            payload: serde_json::json!({"approved": true}),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: V3SignalRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.signal_name, "approve");
    }

    #[test]
    fn api_error_new() {
        let err = ApiError::new("not_found", "workflow missing");
        assert_eq!(err.error, "not_found");
        assert_eq!(err.message, "workflow missing");
        assert!(err.details.is_none());
    }

    #[test]
    fn api_error_with_details() {
        let err = ApiError::new("validation_error", "input invalid")
            .with_details(serde_json::json!({"field": "name", "reason": "too_short"}));
        assert_eq!(err.error, "validation_error");
        assert!(err.details.is_some());
        let details = err.details.unwrap();
        assert_eq!(details["field"], "name");
        assert_eq!(details["reason"], "too_short");
    }

    #[test]
    fn api_error_details_not_serialized_when_none() {
        let err = ApiError::new("not_found", "workflow missing");
        let json = serde_json::to_string(&err).unwrap();
        assert!(!json.contains("details"));
    }

    #[test]
    fn api_error_details_serialized_when_present() {
        let err = ApiError::new("validation_error", "input invalid")
            .with_details(serde_json::json!({"field": "value"}));
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("details"));
        assert!(json.contains("field"));
    }

    #[test]
    fn api_error_status_code_not_found() {
        let err = ApiError::new("not_found", "workflow missing");
        assert_eq!(err.status_code(), 404);
    }

    #[test]
    fn api_error_status_code_bad_request() {
        let err = ApiError::new("invalid_input", "bad input");
        assert_eq!(err.status_code(), 400);
    }

    #[test]
    fn api_error_status_code_conflict() {
        let err = ApiError::new("already_exists", "already exists");
        assert_eq!(err.status_code(), 409);
    }

    #[test]
    fn api_error_status_code_forbidden() {
        let err = ApiError::new("workflow_quarantined", "workflow is quarantined");
        assert_eq!(err.status_code(), 403);
    }

    #[test]
    fn api_error_status_code_too_many_requests() {
        let err = ApiError::new("budget_exhausted", "budget exceeded");
        assert_eq!(err.status_code(), 429);
    }

    #[test]
    fn api_error_status_code_internal_error() {
        let err = ApiError::new("internal_error", "something went wrong");
        assert_eq!(err.status_code(), 500);
    }

    #[test]
    fn api_error_status_code_service_unavailable() {
        let err = ApiError::new("at_capacity", "system at capacity");
        assert_eq!(err.status_code(), 503);
    }

    #[test]
    fn effect_semantics_serde() {
        let exact = EffectSemantics::Exact;
        assert_eq!(serde_json::to_string(&exact).unwrap(), "\"exact\"");

        let unsafe_sem = EffectSemantics::Unsafe;
        assert_eq!(serde_json::to_string(&unsafe_sem).unwrap(), "\"unsafe\"");

        let back: EffectSemantics = serde_json::from_str("\"exact\"").unwrap();
        assert_eq!(back, EffectSemantics::Exact);
    }

    #[test]
    fn timeline_entry_roundtrip() {
        let entry = TimelineEntry {
            sequence: 1,
            timestamp_ms: 1700000000000,
            event_type: "workflow_started".to_string(),
            payload: serde_json::json!({}),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: TimelineEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.sequence, 1);
        assert_eq!(back.event_type, "workflow_started");
    }

    #[test]
    fn history_entry_skip_none_fields() {
        let entry = HistoryEntry {
            sequence: 1,
            timestamp_ms: 1700000000000,
            event_type: "step_completed".to_string(),
            step_id: None,
            error: None,
            output: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(!json.contains("step_id"));
        assert!(!json.contains("error"));
        assert!(!json.contains("output"));
    }

    #[test]
    fn history_entry_with_all_fields() {
        let entry = HistoryEntry {
            sequence: 1,
            timestamp_ms: 1700000000000,
            event_type: "step_failed".to_string(),
            step_id: Some("step-1".to_string()),
            error: Some("timeout".to_string()),
            output: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("step-1"));
        assert!(json.contains("timeout"));
    }

    #[test]
    fn effect_journal_entry_roundtrip() {
        let entry = EffectJournalEntry {
            sequence: 1,
            timestamp_ms: 1700000000000,
            event_type: "effect_committed".to_string(),
            semantics: EffectSemantics::Exact,
            payload: serde_json::json!({"key": "val"}),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: EffectJournalEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.semantics, EffectSemantics::Exact);
    }

    #[test]
    fn workflow_version_response_null_fields() {
        let resp = WorkflowVersionResponse {
            instance_id: "id".to_string(),
            schema_version: 1,
            event_count: 10,
            last_sequence: None,
            last_timestamp_ms: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("null"));
    }
}
