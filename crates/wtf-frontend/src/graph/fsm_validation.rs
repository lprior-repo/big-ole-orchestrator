#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

use crate::graph::node_type::NodeType;
use crate::graph::GraphValidationError;
use crate::graph::GraphValidationResult;

pub fn validate_transition(from: NodeType, to: NodeType) -> GraphValidationResult<NodeType> {
    match (from, to) {
        (NodeType::FsmEntry, NodeType::FsmState) => Ok(to),
        (NodeType::FsmEntry, NodeType::FsmFinal) => Ok(to),
        (NodeType::FsmState, NodeType::FsmState) => Ok(to),
        (NodeType::FsmState, NodeType::FsmTransition) => Ok(to),
        (NodeType::FsmState, NodeType::FsmFinal) => Ok(to),
        (NodeType::FsmTransition, NodeType::FsmState) => Ok(to),
        (NodeType::FsmTransition, NodeType::FsmFinal) => Ok(to),
        (NodeType::FsmFinal, _) => Err(GraphValidationError::InvalidStateTransition),
        (_, NodeType::FsmEntry) => Err(GraphValidationError::InvalidStateTransition),
        _ => Err(GraphValidationError::InvalidStateTransition),
    }
}

#[must_use]
pub fn is_valid_entry_node(node_type: NodeType) -> bool {
    matches!(node_type, NodeType::FsmEntry)
}

#[must_use]
pub fn is_valid_exit_node(node_type: NodeType) -> bool {
    matches!(node_type, NodeType::FsmFinal)
}

#[must_use]
pub fn is_terminal_node(node_type: NodeType) -> bool {
    matches!(node_type, NodeType::FsmFinal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::GraphValidationError;

    mod fsm_validation {
        use super::*;

        #[test]
        fn entry_to_state_is_valid() {
            let result = validate_transition(NodeType::FsmEntry, NodeType::FsmState);
            assert!(result.is_ok());
        }

        #[test]
        fn state_to_state_is_valid() {
            let result = validate_transition(NodeType::FsmState, NodeType::FsmState);
            assert!(result.is_ok());
        }

        #[test]
        fn state_to_final_is_valid() {
            let result = validate_transition(NodeType::FsmState, NodeType::FsmFinal);
            assert!(result.is_ok());
        }

        #[test]
        fn final_to_anything_is_invalid() {
            let result = validate_transition(NodeType::FsmFinal, NodeType::FsmEntry);
            assert!(result.is_err());
            assert!(matches!(
                result.unwrap_err(),
                GraphValidationError::InvalidStateTransition
            ));
        }

        #[test]
        fn anything_to_entry_is_invalid() {
            let result = validate_transition(NodeType::FsmState, NodeType::FsmEntry);
            assert!(result.is_err());
            assert!(matches!(
                result.unwrap_err(),
                GraphValidationError::InvalidStateTransition
            ));
        }

        #[test]
        fn is_valid_entry_node() {
            assert!(is_valid_entry_node(NodeType::FsmEntry));
            assert!(!is_valid_entry_node(NodeType::FsmState));
        }

        #[test]
        fn is_valid_exit_node() {
            assert!(is_valid_exit_node(NodeType::FsmFinal));
            assert!(!is_valid_exit_node(NodeType::FsmState));
        }
    }
}
