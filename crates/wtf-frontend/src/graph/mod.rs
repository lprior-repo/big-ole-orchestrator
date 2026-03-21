#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fmt;
use uuid::Uuid;

pub mod calc;
mod connectivity;
mod core;
mod domain_types;
mod execution;
pub mod execution_record;
pub mod execution_state;
mod metadata;
mod view;

pub mod expressions;
pub mod layout;
pub mod restate_types;
pub mod validation;
pub mod workflow_node;

pub use connectivity::{ConnectionError, ConnectionResult};
pub use domain_types::{
    EmptyStringError, NodeIcon, NodeMetadata, NodeUiState, NonEmptyString, PositiveDuration,
    RunOutcome, ServiceName, StateKey, UnknownIconError,
};
pub use execution_record::{
    AttemptNumber, EmptyErrorMessage, ExecutionError, ExecutionOverallStatus, ExecutionRecord,
    ExecutionRecordId, StepCount, StepName, StepOutput, StepRecord, StepType, WorkflowName,
};
pub use execution_state::{
    can_transition, try_transition, CompletedState, ExecutionState, FailedState, IdleState,
    InvalidTransition, QueuedState, RunningState, SkippedState, StateTransition, TerminalState,
};
pub use validation::{validate_workflow, ValidationIssue, ValidationResult, ValidationSeverity};
pub use workflow_node::{
    ConditionResult, HttpMethod, RunConfig, UnknownHttpMethodError, WorkflowNode,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct NodeId(pub Uuid);

impl NodeId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PortName(pub String);

impl<S: Into<String>> From<S> for PortName {
    fn from(s: S) -> Self {
        Self(s.into())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum NodeCategory {
    Entry,
    Durable,
    State,
    Flow,
    Timing,
    Signal,
}

impl fmt::Display for NodeCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Entry => "entry",
            Self::Durable => "durable",
            Self::State => "state",
            Self::Flow => "flow",
            Self::Timing => "timing",
            Self::Signal => "signal",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Node {
    pub id: NodeId,
    pub name: String,
    #[serde(skip)]
    pub node: WorkflowNode,
    pub category: NodeCategory,
    pub icon: String,
    pub x: f32,
    pub y: f32,
    pub last_output: Option<serde_json::Value>,
    #[serde(default)]
    pub selected: bool,
    #[serde(default)]
    pub executing: bool,
    #[serde(default)]
    pub skipped: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip)]
    pub execution_state: ExecutionState,
    #[serde(default, skip)]
    pub metadata: serde_json::Value,
    #[serde(default, skip)]
    pub execution_data: serde_json::Value,
    #[serde(default)]
    pub node_type: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub config: serde_json::Value,
}

impl Node {
    fn alias_target_for_config_key(key: &str) -> Option<&'static str> {
        match key {
            "stateKey" => Some("key"),
            "conditionExpression" => Some("expression"),
            "durableStepName" => Some("durable_step_name"),
            "targetService" | "service_name" => Some("target"),
            "handler_name" => Some("handler"),
            "loopIterator" => Some("iterator"),
            "compensationHandler" => Some("target_step"),
            "cronExpression" => Some("schedule"),
            "workflowKey" => Some("workflow_name"),
            "promiseName" => Some("promise_name"),
            "awakeableId" => Some("awakeable_id"),
            "signalName" => Some("signal_name"),
            "timeoutMs" => Some("timeout_ms"),
            _ => None,
        }
    }

    fn should_apply_alias(config_object: &Map<String, Value>, target: &str) -> bool {
        config_object.get(target).map_or(true, Value::is_null)
    }

    fn normalize_config_aliases(config: &Value) -> Value {
        let Value::Object(config_object) = config else {
            return config.clone();
        };

        let mut normalized = config_object.clone();
        for (key, value) in config_object {
            if let Some(target) = Self::alias_target_for_config_key(key) {
                if Self::should_apply_alias(&normalized, target) {
                    normalized.insert(target.to_string(), value.clone());
                }
            }
        }

        Value::Object(normalized)
    }

    fn merged_node_json(&self, config: &Value) -> Option<Value> {
        let Value::Object(base_object) = serde_json::to_value(&self.node).ok()? else {
            return None;
        };

        let Value::Object(config_object) = config else {
            return None;
        };

        let mut merged = base_object.clone();
        for (key, value) in config_object {
            merged.insert(key.clone(), value.clone());
        }

        if let Some(base_type) = base_object.get("type").cloned() {
            merged.insert("type".to_string(), base_type);
        }

        Some(Value::Object(merged))
    }

    pub fn apply_config_update(&mut self, new_config: &Value) {
        let normalized_config = Self::normalize_config_aliases(new_config);
        self.config = normalized_config.clone();

        if let Some(updated_node) = self
            .merged_node_json(&normalized_config)
            .and_then(|json| serde_json::from_value::<WorkflowNode>(json).ok())
        {
            self.node = updated_node;
            self.node_type = self.node.to_string();
            self.category = self.node.category();
            self.icon = self.node.icon().to_string();
            self.description = self.node.description().to_string();
        }
    }

    #[must_use]
    pub fn from_workflow_node(name: String, node: WorkflowNode, x: f32, y: f32) -> Self {
        let category = node.category();
        let icon = node.icon().to_string();
        let node_type = node.to_string();
        let description = node.description().to_string();
        let config = serde_json::to_value(&node).unwrap_or_default();

        Self {
            id: NodeId::new(),
            name,
            node,
            category,
            icon,
            x,
            y,
            last_output: None,
            selected: false,
            executing: false,
            skipped: false,
            error: None,
            execution_state: ExecutionState::default(),
            metadata: Value::default(),
            execution_data: Value::default(),
            node_type,
            description,
            config,
        }
    }

    pub fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
    }
}

impl Default for Node {
    fn default() -> Self {
        Self::from_workflow_node(String::new(), WorkflowNode::default(), 0.0, 0.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Connection {
    pub id: Uuid,
    pub source: NodeId,
    pub target: NodeId,
    pub source_port: PortName,
    pub target_port: PortName,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunRecord {
    pub id: Uuid,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub results: std::collections::HashMap<NodeId, serde_json::Value>,
    pub success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restate_invocation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Workflow {
    pub nodes: Vec<Node>,
    pub connections: Vec<Connection>,
    pub viewport: Viewport,
    pub execution_queue: Vec<NodeId>,
    pub current_step: usize,
    pub history: Vec<RunRecord>,
    #[serde(default)]
    pub execution_records: Vec<ExecutionRecord>,
}

impl Default for Workflow {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::workflow_node::{SendMessageConfig, SetStateConfig};
    use super::{Node, NodeCategory, NodeId, PortName, WorkflowNode};
    use serde_json::{json, Value};

    #[test]
    fn given_node_id_when_displayed_then_it_matches_inner_uuid() {
        let id = NodeId::new();
        assert_eq!(id.to_string(), id.0.to_string());
    }

    #[test]
    fn given_default_node_id_when_created_then_it_is_not_nil() {
        let id = NodeId::default();
        assert_ne!(id.0, uuid::Uuid::nil());
    }

    #[test]
    fn given_string_when_converted_to_port_name_then_value_is_preserved() {
        let port = PortName::from("main");
        assert_eq!(port.0, "main");
    }

    #[test]
    fn given_node_categories_when_displayed_then_lowercase_labels_are_returned() {
        assert_eq!(NodeCategory::Entry.to_string(), "entry");
        assert_eq!(NodeCategory::Durable.to_string(), "durable");
        assert_eq!(NodeCategory::State.to_string(), "state");
        assert_eq!(NodeCategory::Flow.to_string(), "flow");
        assert_eq!(NodeCategory::Timing.to_string(), "timing");
        assert_eq!(NodeCategory::Signal.to_string(), "signal");
    }

    #[test]
    fn given_config_update_when_applied_then_node_config_is_replaced() {
        let mut node = Node::from_workflow_node(
            "state".to_string(),
            WorkflowNode::SetState(SetStateConfig::default()),
            0.0,
            0.0,
        );

        node.apply_config_update(&json!({
            "type": "set-state",
            "stateKey": "cart"
        }));

        assert_eq!(
            node.config.get("stateKey").and_then(Value::as_str),
            Some("cart")
        );
    }

    #[test]
    fn given_set_state_alias_state_key_when_applied_then_typed_key_is_updated() {
        let mut node = Node::from_workflow_node(
            "state".to_string(),
            WorkflowNode::SetState(SetStateConfig::default()),
            0.0,
            0.0,
        );

        node.apply_config_update(&json!({
            "type": "set-state",
            "stateKey": "session"
        }));

        assert_eq!(
            node.config.get("key").and_then(Value::as_str),
            Some("session")
        );

        assert!(matches!(
            &node.node,
            WorkflowNode::SetState(config)
                if config.key.as_deref() == Some("session")
        ));
    }

    #[test]
    fn given_send_message_alias_target_service_when_applied_then_typed_target_is_updated() {
        let mut node = Node::from_workflow_node(
            "send".to_string(),
            WorkflowNode::SendMessage(SendMessageConfig::default()),
            0.0,
            0.0,
        );

        node.apply_config_update(&json!({
            "type": "send-message",
            "targetService": "notification-service"
        }));

        assert_eq!(
            node.config.get("target").and_then(Value::as_str),
            Some("notification-service")
        );

        assert!(matches!(
            &node.node,
            WorkflowNode::SendMessage(config)
                if config.target.as_deref() == Some("notification-service")
        ));
    }

    #[test]
    fn given_non_object_config_when_applying_then_typed_node_is_preserved() {
        let mut node = Node::from_workflow_node(
            "state".to_string(),
            WorkflowNode::SetState(SetStateConfig {
                key: Some("session".to_string()),
                value: Some("active".to_string()),
            }),
            0.0,
            0.0,
        );
        let original_node = node.node.clone();

        node.apply_config_update(&json!("invalid-shape"));

        assert_eq!(node.config, json!("invalid-shape"));
        assert_eq!(node.node, original_node);
    }
}
