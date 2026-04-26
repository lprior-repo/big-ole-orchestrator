use serde::{Deserialize, Serialize};
use vo_types::{apply_redaction, RedactionKind, RedactionRule};

#[derive(Debug, Clone)]
pub struct WorkflowHistoryConfig {
    pub instance_id: String,
    pub engine_url: String,
    pub json: bool,
}

impl Default for WorkflowHistoryConfig {
    fn default() -> Self {
        Self {
            instance_id: String::new(),
            engine_url: "http://localhost:3000".to_string(),
            json: false,
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum WorkflowHistoryError {
    #[error("API unreachable at {url}: {reason}")]
    Unreachable { url: String, reason: String },

    #[error("API returned HTTP {status} for {url}")]
    HttpError { url: String, status: u16 },

    #[error("invalid response from API: {reason}")]
    InvalidResponse { reason: String },

    #[error("not found: workflow {instance_id}")]
    NotFound { instance_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowHistoryEntry {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowHistoryResponse {
    pub instance_id: String,
    pub entries: Vec<WorkflowHistoryEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redacted_fields: Option<Vec<Vec<String>>>,
}

fn default_redaction_rules() -> Vec<RedactionRule> {
    vec![
        RedactionRule::new(
            vec!["secrets".to_string()],
            RedactionKind::Remove,
        ),
        RedactionRule::new(
            vec!["api_key".to_string()],
            RedactionKind::Remove,
        ),
        RedactionRule::new(
            vec!["token".to_string()],
            RedactionKind::Remove,
        ),
        RedactionRule::new(
            vec!["password".to_string()],
            RedactionKind::Remove,
        ),
    ]
}

fn redact_history_entries(
    entries: Vec<WorkflowHistoryEntry>,
) -> (Vec<WorkflowHistoryEntry>, Vec<Vec<String>>) {
    let rules = default_redaction_rules();
    let mut all_redacted = Vec::new();
    let redacted_entries = entries
        .into_iter()
        .map(|entry| {
            let redacted_output = match &entry.output {
                Some(val) => {
                    let (v, p) = apply_redaction(val, &rules);
                    all_redacted.extend(p);
                    Some(v)
                }
                None => None,
            };

            WorkflowHistoryEntry {
                output: redacted_output,
                ..entry
            }
        })
        .collect();

    (redacted_entries, all_redacted)
}

pub async fn fetch_workflow_history(
    engine_url: &str,
    instance_id: &str,
) -> Result<WorkflowHistoryResponse, WorkflowHistoryError> {
    let url = format!(
        "{}/api/v1/workflows/{}/history",
        engine_url, instance_id
    );

    let response = reqwest::get(&url)
        .await
        .map_err(|e| WorkflowHistoryError::Unreachable {
            url: url.clone(),
            reason: e.to_string(),
        })?;

    let status = response.status().as_u16();
    if status == 404 {
        return Err(WorkflowHistoryError::NotFound {
            instance_id: instance_id.to_string(),
        });
    }
    if !response.status().is_success() {
        return Err(WorkflowHistoryError::HttpError { url, status });
    }

    let body = response
        .bytes()
        .await
        .map_err(|e| WorkflowHistoryError::InvalidResponse {
            reason: e.to_string(),
        })?;

    serde_json::from_slice(&body).map_err(|e| WorkflowHistoryError::InvalidResponse {
        reason: e.to_string(),
    })
}

pub async fn run_workflow_history(
    config: &WorkflowHistoryConfig,
) -> Result<WorkflowHistoryResponse, WorkflowHistoryError> {
    let raw = fetch_workflow_history(&config.engine_url, &config.instance_id).await?;

    let (redacted_entries, redacted_fields) = redact_history_entries(raw.entries);

    Ok(WorkflowHistoryResponse {
        instance_id: raw.instance_id,
        entries: redacted_entries,
        redacted_fields: if redacted_fields.is_empty() {
            None
        } else {
            Some(redacted_fields)
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_default_engine_url() {
        let config = WorkflowHistoryConfig::default();
        assert_eq!(config.engine_url, "http://localhost:3000");
    }

    #[test]
    fn error_not_found_display() {
        let err = WorkflowHistoryError::NotFound {
            instance_id: "test-id".to_string(),
        };
        assert_eq!(err.to_string(), "not found: workflow test-id");
    }

    #[test]
    fn redaction_removes_secrets_from_output() {
        let rules = default_redaction_rules();
        let payload = serde_json::json!({
            "secrets": {"api_key": "sk-12345"},
            "public_data": "visible"
        });
        let (redacted, paths) = apply_redaction(&payload, &rules);
        assert!(
            !redacted.as_object().unwrap().contains_key("secrets"),
            "secrets must be removed"
        );
        assert_eq!(redacted["public_data"], "visible");
        assert!(!paths.is_empty());
    }

    #[test]
    fn history_response_serde_roundtrip() {
        let response = WorkflowHistoryResponse {
            instance_id: "ns/01ARZ3NDEK".to_string(),
            entries: vec![WorkflowHistoryEntry {
                sequence: 1,
                timestamp_ms: 1700000000000,
                event_type: "workflow_started".to_string(),
                step_id: Some("step-1".to_string()),
                error: None,
                output: Some(serde_json::json!({"result": "ok"})),
            }],
            redacted_fields: Some(vec![vec!["output".to_string(), "secrets".to_string()]]),
        };
        let json = serde_json::to_string_pretty(&response).unwrap();
        let back: WorkflowHistoryResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back.instance_id, "ns/01ARZ3NDEK");
        assert_eq!(back.entries.len(), 1);
        assert_eq!(back.entries[0].event_type, "workflow_started");
        assert!(back.redacted_fields.is_some());
    }

    #[test]
    fn redact_history_entries_removes_sensitive_output() {
        let entries = vec![WorkflowHistoryEntry {
            sequence: 1,
            timestamp_ms: 1700000000000,
            event_type: "step_completed".to_string(),
            step_id: Some("step-1".to_string()),
            error: None,
            output: Some(serde_json::json!({
                "result": "ok",
                "secrets": {"token": "abc123"}
            })),
        }];

        let (redacted, paths) = redact_history_entries(entries);
        assert_eq!(redacted.len(), 1);
        let output = redacted[0].output.as_ref().unwrap();
        assert!(
            !output.as_object().unwrap().contains_key("secrets"),
            "secrets must be removed from output"
        );
        assert_eq!(output["result"], "ok");
        assert!(!paths.is_empty());
    }

    #[test]
    fn redact_history_entries_preserves_non_sensitive_output() {
        let entries = vec![WorkflowHistoryEntry {
            sequence: 1,
            timestamp_ms: 1700000000000,
            event_type: "step_completed".to_string(),
            step_id: None,
            error: None,
            output: Some(serde_json::json!({"result": "ok"})),
        }];

        let (redacted, paths) = redact_history_entries(entries);
        assert_eq!(redacted[0].output.as_ref().unwrap()["result"], "ok");
        assert!(paths.is_empty());
    }
}
