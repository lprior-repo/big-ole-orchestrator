//! API request DTOs.

use super::newtypes::{SignalName, WorkflowName};
use serde::{Deserialize, Serialize};

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

/// POST /api/v1/definitions/<type> request body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefinitionRequest {
    pub source: String,
    pub workflow_type: String,
}

/// POST /api/v1/workflows request body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V3StartRequest {
    pub namespace: String,
    pub workflow_type: String,
    pub paradigm: String,
    pub input: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
}

/// POST /api/v1/workflows/:id/signals request body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V3SignalRequest {
    pub signal_name: String,
    pub payload: serde_json::Value,
}

/// POST /api/v1/workflows/validate request body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateWorkflowRequest {
    pub source: String,
}
