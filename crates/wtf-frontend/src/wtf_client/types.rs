//! wtf_client types - placeholder for full implementation

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceView {
    pub instance_id: String,
    pub workflow_type: String,
    pub status: String,
    pub current_state: Option<String>,
    pub last_event_seq: u64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRecord {
    pub seq: u64,
    pub event_type: String,
    pub data: serde_json::Value,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartResponse {
    pub instance_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WtfClientConfig {
    pub base_url: String,
    pub timeout_ms: u64,
}
