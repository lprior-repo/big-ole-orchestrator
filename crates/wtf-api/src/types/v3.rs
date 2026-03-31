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
