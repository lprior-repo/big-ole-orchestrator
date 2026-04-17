use fjall::Keyspace;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

const REDACTED_PLACEHOLDER: &str = "[REDACTED]";
const SENSITIVE_FIELD_NAMES: &[&str] = &[
    "password",
    "secret",
    "token",
    "api_key",
    "apikey",
    "credential",
    "auth",
    "private_key",
    "privatekey",
    "session",
];

fn is_sensitive_field(field_name: &str) -> bool {
    let lower = field_name.to_lowercase();
    SENSITIVE_FIELD_NAMES.iter().any(|s| lower.contains(s))
}

fn redact_value(value: &Value) -> Value {
    match value {
        Value::Null => Value::Null,
        Value::Bool(_) => Value::String(REDACTED_PLACEHOLDER.to_string()),
        Value::Number(_) => Value::String(REDACTED_PLACEHOLDER.to_string()),
        Value::String(_) => Value::String(REDACTED_PLACEHOLDER.to_string()),
        Value::Array(arr) => Value::Array(arr.iter().map(redact_value).collect()),
        Value::Object(obj) => {
            let mut redacted = serde_json::Map::new();
            for (k, v) in obj {
                if is_sensitive_field(k) {
                    redacted.insert(k.clone(), Value::String(REDACTED_PLACEHOLDER.to_string()));
                } else {
                    redacted.insert(k.clone(), redact_value(v));
                }
            }
            Value::Object(redacted)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalEventOutput {
    pub instance_id: String,
    pub sequence: u64,
    pub timestamp_ms: u64,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactedEventOutput {
    pub instance_id: String,
    pub sequence: u64,
    pub timestamp_ms: u64,
    pub payload: Value,
    pub redaction_applied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceHistoryOutput {
    pub instance_id: String,
    pub view_type: String,
    pub event_count: usize,
    pub events: Vec<RedactedEventOutput>,
}

#[derive(Debug, thiserror::Error)]
pub enum InstanceHistoryError {
    #[error("instance not found: {0}")]
    InstanceNotFound(String),

    #[error("failed to read events: {reason}")]
    ReadFailed { reason: String },

    #[error("invalid event data: {reason}")]
    InvalidEvent { reason: String },
}

pub fn get_instance_history(
    keyspace: &Keyspace,
    instance_id: &str,
    canonical: bool,
) -> Result<String, InstanceHistoryError> {
    if instance_id.is_empty() {
        return Err(InstanceHistoryError::InstanceNotFound(
            "instance_id cannot be empty".to_string(),
        ));
    }

    let events_p = keyspace
        .open_partition("events", fjall::PartitionCreateOptions::default())
        .map_err(|e| InstanceHistoryError::ReadFailed {
            reason: e.to_string(),
        })?;

    let instance_id_bytes = instance_id.as_bytes();
    let mut events: Vec<CanonicalEventOutput> = Vec::new();

    for item in events_p.prefix(instance_id_bytes) {
        let (_key, value) = item.map_err(|e| InstanceHistoryError::ReadFailed {
            reason: e.to_string(),
        })?;

        let envelope = vo_types::EventEnvelope::from_bytes(&value).map_err(|e| {
            InstanceHistoryError::InvalidEvent {
                reason: e.to_string(),
            }
        })?;

        if envelope.instance_id == instance_id {
            events.push(CanonicalEventOutput {
                instance_id: envelope.instance_id,
                sequence: envelope.sequence,
                timestamp_ms: envelope.timestamp_ms,
                payload: envelope.payload,
            });
        }
    }

    events.sort_by_key(|e| e.sequence);

    if canonical {
        let output = InstanceHistoryOutput {
            instance_id: instance_id.to_string(),
            view_type: "canonical".to_string(),
            event_count: events.len(),
            events: events
                .into_iter()
                .map(|e| RedactedEventOutput {
                    instance_id: e.instance_id,
                    sequence: e.sequence,
                    timestamp_ms: e.timestamp_ms,
                    payload: e.payload,
                    redaction_applied: false,
                })
                .collect(),
        };
        serde_json::to_string_pretty(&output).map_err(|e| InstanceHistoryError::ReadFailed {
            reason: e.to_string(),
        })
    } else {
        let output = InstanceHistoryOutput {
            instance_id: instance_id.to_string(),
            view_type: "operator_projection".to_string(),
            event_count: events.len(),
            events: events
                .into_iter()
                .map(|e| {
                    let redacted_payload = redact_value(&e.payload);
                    let redaction_applied = redacted_payload != e.payload;
                    RedactedEventOutput {
                        instance_id: e.instance_id,
                        sequence: e.sequence,
                        timestamp_ms: e.timestamp_ms,
                        payload: redacted_payload,
                        redaction_applied,
                    }
                })
                .collect(),
        };
        serde_json::to_string_pretty(&output).map_err(|e| InstanceHistoryError::ReadFailed {
            reason: e.to_string(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntryOutput {
    pub command_id: String,
    pub kind: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryOutput {
    pub can_undo: bool,
    pub can_redo: bool,
    pub undo_stack_depth: usize,
    pub redo_stack_depth: usize,
    pub entries: Vec<HistoryEntryOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoResult {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedoResult {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    #[error("history file not found: {path}")]
    HistoryFileNotFound { path: PathBuf },

    #[error("failed to read history: {reason}")]
    ReadFailed { reason: String },

    #[error("failed to write history: {reason}")]
    WriteFailed { reason: String },

    #[error("invalid history format: {reason}")]
    InvalidFormat { reason: String },
}

pub struct HistoryConfig {
    pub history_path: PathBuf,
    pub workflow_name: String,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            history_path: PathBuf::from(".vo/command_history.json"),
            workflow_name: "default".to_string(),
        }
    }
}

pub fn load_history(
    path: &PathBuf,
) -> Result<vo_types::command_history::CommandHistory, HistoryError> {
    if !path.exists() {
        return Ok(vo_types::command_history::CommandHistory::new());
    }

    let content = std::fs::read_to_string(path).map_err(|e| HistoryError::ReadFailed {
        reason: e.to_string(),
    })?;

    serde_json::from_str(&content).map_err(|e| HistoryError::InvalidFormat {
        reason: e.to_string(),
    })
}

pub fn save_history(
    path: &PathBuf,
    history: &vo_types::command_history::CommandHistory,
) -> Result<(), HistoryError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| HistoryError::WriteFailed {
            reason: format!("failed to create directory: {}", e),
        })?;
    }

    let content = serde_json::to_string_pretty(history).map_err(|e| HistoryError::WriteFailed {
        reason: e.to_string(),
    })?;

    std::fs::write(path, content).map_err(|e| HistoryError::WriteFailed {
        reason: e.to_string(),
    })
}

pub fn get_history(history: &vo_types::command_history::CommandHistory) -> HistoryOutput {
    let entries: Vec<HistoryEntryOutput> = history
        .entries()
        .iter()
        .map(|e| HistoryEntryOutput {
            command_id: e.envelope.metadata.command_id.as_str().to_string(),
            kind: format!("{:?}", e.kind),
            status: format!("{}", e.status),
        })
        .collect();

    HistoryOutput {
        can_undo: history.can_undo(),
        can_redo: history.can_redo(),
        undo_stack_depth: history.undo_stack().len(),
        redo_stack_depth: history.redo_stack().len(),
        entries,
    }
}

pub fn undo_command(history: &mut vo_types::command_history::CommandHistory) -> UndoResult {
    match history.undo() {
        Ok(true) => UndoResult {
            success: true,
            message: "Undo successful".to_string(),
        },
        Ok(false) => UndoResult {
            success: false,
            message: "Nothing to undo".to_string(),
        },
        Err(e) => UndoResult {
            success: false,
            message: format!("Undo failed: {}", e),
        },
    }
}

pub fn redo_command(history: &mut vo_types::command_history::CommandHistory) -> RedoResult {
    match history.redo() {
        Ok(true) => RedoResult {
            success: true,
            message: "Redo successful".to_string(),
        },
        Ok(false) => RedoResult {
            success: false,
            message: "Nothing to redo".to_string(),
        },
        Err(e) => RedoResult {
            success: false,
            message: format!("Redo failed: {}", e),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vo_types::command_history::{CommandHistory, CommandKind};
    use vo_types::{DagNode, NodeName, RetryPolicy, WorkflowSnapshot};

    fn test_snapshot() -> WorkflowSnapshot {
        WorkflowSnapshot::new(
            "test-workflow".into(),
            vec![DagNode {
                node_name: NodeName::parse("test-node").unwrap(),
                retry_policy: RetryPolicy::new(3, 1000, 2.0).unwrap(),
            }],
            vec![],
        )
    }

    #[test]
    fn test_history_output_structure() {
        let history = CommandHistory::new();
        let output = get_history(&history);

        assert!(!output.can_undo);
        assert!(!output.can_redo);
        assert_eq!(output.undo_stack_depth, 0);
        assert_eq!(output.redo_stack_depth, 0);
        assert!(output.entries.is_empty());
    }

    #[test]
    fn test_undo_with_empty_history() {
        let mut history = CommandHistory::new();
        let result = undo_command(&mut history);

        assert!(!result.success);
        assert_eq!(result.message, "Nothing to undo");
    }

    #[test]
    fn test_redo_with_empty_history() {
        let mut history = CommandHistory::new();
        let result = redo_command(&mut history);

        assert!(!result.success);
        assert_eq!(result.message, "Nothing to redo");
    }

    #[test]
    fn test_save_and_load_history() {
        let mut history = CommandHistory::new();
        history
            .save_undo_point(CommandKind::NodeCreate, test_snapshot())
            .unwrap();

        let path = PathBuf::from("/tmp/test_history.json");
        save_history(&path, &history).unwrap();

        let loaded = load_history(&path).unwrap();
        assert_eq!(loaded.entries().len(), 1);

        std::fs::remove_file(&path).ok();
    }
}
