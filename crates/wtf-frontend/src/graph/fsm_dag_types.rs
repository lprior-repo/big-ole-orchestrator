#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

pub mod dag_validation;
pub mod fsm_validation;
pub mod node_type;

pub use dag_validation::validate_split_join_structure;
pub use fsm_validation::{
    is_terminal_node, is_valid_entry_node, is_valid_exit_node, validate_transition,
};
pub use node_type::{NodeType, ParseNodeTypeError};
pub use GraphValidationError;
pub use GraphValidationResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphValidationError {
    CycleDetected,
    InvalidStateTransition,
    NodeNotFound,
    DuplicateNodeId,
    DisconnectedGraph,
    InvalidPortName,
    EmptyNodeName,
}

impl std::fmt::Display for GraphValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CycleDetected => write!(f, "Cycle detected in graph"),
            Self::InvalidStateTransition => write!(f, "Invalid FSM state transition"),
            Self::NodeNotFound => write!(f, "Node not found"),
            Self::DuplicateNodeId => write!(f, "Duplicate node ID"),
            Self::DisconnectedGraph => write!(f, "Graph is disconnected"),
            Self::InvalidPortName => write!(f, "Invalid port name"),
            Self::EmptyNodeName => write!(f, "Node name cannot be empty"),
        }
    }
}

impl std::error::Error for GraphValidationError {}

pub type GraphValidationResult<T> = Result<T, GraphValidationError>;

pub mod fsm {
    pub use super::is_terminal_node;
    pub use super::is_valid_entry_node;
    pub use super::is_valid_exit_node;
    pub use super::validate_transition;
}

pub mod dag {
    pub use super::validate_split_join_structure;
}

#[cfg(test)]
mod tests {
    use super::*;

    mod graph_validation_error {
        use super::*;

        #[test]
        fn cycle_detected_display() {
            assert_eq!(
                format!("{}", GraphValidationError::CycleDetected),
                "Cycle detected in graph"
            );
        }

        #[test]
        fn invalid_state_transition_display() {
            assert_eq!(
                format!("{}", GraphValidationError::InvalidStateTransition),
                "Invalid FSM state transition"
            );
        }

        #[test]
        fn node_not_found_display() {
            assert_eq!(
                format!("{}", GraphValidationError::NodeNotFound),
                "Node not found"
            );
        }
    }
}
