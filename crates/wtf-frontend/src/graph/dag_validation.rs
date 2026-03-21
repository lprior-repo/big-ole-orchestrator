#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

use crate::graph::node_type::NodeType;
use crate::graph::GraphValidationError;
use crate::graph::GraphValidationResult;

pub fn validate_split_join_structure(
    incoming_count: usize,
    outgoing_count: usize,
    node_type: NodeType,
) -> GraphValidationResult<()> {
    match node_type {
        NodeType::DagSplit => {
            if incoming_count != 1 {
                return Err(GraphValidationError::InvalidStateTransition);
            }
            if outgoing_count < 2 {
                return Err(GraphValidationError::InvalidStateTransition);
            }
            Ok(())
        }
        NodeType::DagJoin => {
            if incoming_count < 2 {
                return Err(GraphValidationError::InvalidStateTransition);
            }
            if outgoing_count != 1 {
                return Err(GraphValidationError::InvalidStateTransition);
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::GraphValidationError;

    mod dag_validation {
        use super::*;

        #[test]
        fn split_requires_single_incoming() {
            let result = validate_split_join_structure(1, 3, NodeType::DagSplit);
            assert!(result.is_ok());
        }

        #[test]
        fn split_rejects_multiple_incoming() {
            let result = validate_split_join_structure(2, 3, NodeType::DagSplit);
            assert!(result.is_err());
        }

        #[test]
        fn split_requires_multiple_outgoing() {
            let result = validate_split_join_structure(1, 1, NodeType::DagSplit);
            assert!(result.is_err());
        }

        #[test]
        fn join_requires_multiple_incoming() {
            let result = validate_split_join_structure(3, 1, NodeType::DagJoin);
            assert!(result.is_ok());
        }

        #[test]
        fn join_rejects_single_incoming() {
            let result = validate_split_join_structure(1, 1, NodeType::DagJoin);
            assert!(result.is_err());
        }

        #[test]
        fn join_requires_single_outgoing() {
            let result = validate_split_join_structure(3, 2, NodeType::DagJoin);
            assert!(result.is_err());
        }

        #[test]
        fn non_split_join_nodes_always_valid() {
            let result = validate_split_join_structure(0, 0, NodeType::DagTask);
            assert!(result.is_ok());
        }
    }
}
