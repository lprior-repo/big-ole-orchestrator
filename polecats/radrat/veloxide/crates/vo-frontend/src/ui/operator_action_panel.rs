//! Operator action panel component (stub — implementation pending)

use serde::{Deserialize, Serialize};

/// Types of actions an operator can perform on a workflow node
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionType {
    Retry,
    Cancel,
    Skip,
    ForceComplete,
}

/// Placeholder component for operator action panel
#[derive(Debug, Clone)]
pub struct OperatorActionPanel {
    pub node_id: String,
    pub available_actions: Vec<ActionType>,
}

impl OperatorActionPanel {
    pub fn new(node_id: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            available_actions: vec![
                ActionType::Retry,
                ActionType::Cancel,
                ActionType::Skip,
                ActionType::ForceComplete,
            ],
        }
    }
}
