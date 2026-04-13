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

/// Response to GET /api/v1/workflows/:id/history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryResponse {
    pub instance_id: String,
    pub entries: Vec<HistoryEntry>,
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
