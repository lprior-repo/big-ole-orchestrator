//! Guarantee-class validation for publish-time rejection of unsafe nodes.
//!
//! Validates that workflow node kinds comply with the guarantee class
//! constraints. For example, exact-once workflows must not contain
//! Unsafe nodes (ADR-003, ADR-031).
//!
//! # Architecture
//!
//! - Data: `NodeDescriptor` (lightweight node summary)
//! - Calc: `validate_exact_workflow_node_kinds` pure function
//! - Error: `UnsafeNodeError` for rejection reporting

use thiserror::Error;
use vo_types::{GuaranteeClass, NodeKind};

/// Error returned when a workflow violates guarantee-class node constraints.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum UnsafeNodeError {
    #[error(
        "exact-once workflow contains unsafe node '{node_name}' at index {node_index}; \
         exact-once guarantee class does not permit unsafe nodes"
    )]
    UnsafeNodeInExactWorkflow {
        node_name: String,
        node_index: usize,
    },
}

impl UnsafeNodeError {
    #[must_use]
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::UnsafeNodeInExactWorkflow { .. } => "unsafe_node_in_exact_workflow",
        }
    }
}

/// Lightweight descriptor for a workflow node used in validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeDescriptor {
    pub name: String,
    pub kind: NodeKind,
}

/// Validate that a workflow's node kinds comply with its guarantee class.
///
/// Guarantee classes that do not permit unsafe nodes (ExactOnce, AtLeastOnce)
/// will reject any node with `NodeKind::Unsafe`.
///
/// # Errors
///
/// Returns `UnsafeNodeError::UnsafeNodeInExactWorkflow` if an unsafe node
/// is found in a guarantee class that forbids them.
pub fn validate_exact_workflow_node_kinds(
    guarantee_class: GuaranteeClass,
    nodes: &[NodeDescriptor],
) -> Result<(), UnsafeNodeError> {
    if guarantee_class.permits_unsafe_nodes() {
        return Ok(());
    }
    for (idx, node) in nodes.iter().enumerate() {
        if node.kind == NodeKind::Unsafe {
            return Err(UnsafeNodeError::UnsafeNodeInExactWorkflow {
                node_name: node.name.clone(),
                node_index: idx,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_exact_workflow_with_unsafe_node_when_published_then_validation_rejects() {
        use vo_types::{GuaranteeClass, NodeKind};

        let nodes = vec![
            NodeDescriptor {
                name: "safe_step".to_string(),
                kind: NodeKind::Pure,
            },
            NodeDescriptor {
                name: "dangerous_step".to_string(),
                kind: NodeKind::Unsafe,
            },
        ];

        let result =
            validate_exact_workflow_node_kinds(GuaranteeClass::ExactOnce, &nodes);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(
                &err,
                UnsafeNodeError::UnsafeNodeInExactWorkflow { node_name, node_index }
                if node_name == "dangerous_step" && *node_index == 1
            ),
            "expected UnsafeNodeInExactWorkflow with node_name='dangerous_step' index=1, got {:?}",
            err
        );
        assert_eq!(err.error_code(), "unsafe_node_in_exact_workflow");
    }

    #[test]
    fn exact_workflow_without_unsafe_nodes_passes() {
        use vo_types::{GuaranteeClass, NodeKind};

        let nodes = vec![
            NodeDescriptor {
                name: "step_a".to_string(),
                kind: NodeKind::Pure,
            },
            NodeDescriptor {
                name: "step_b".to_string(),
                kind: NodeKind::ManagedEffect,
            },
        ];

        let result = validate_exact_workflow_node_kinds(GuaranteeClass::ExactOnce, &nodes);
        assert!(result.is_ok());
    }

    #[test]
    fn at_least_once_workflow_with_unsafe_node_rejects() {
        use vo_types::{GuaranteeClass, NodeKind};

        let nodes = vec![NodeDescriptor {
            name: "fire_and_forget".to_string(),
            kind: NodeKind::Unsafe,
        }];

        let result =
            validate_exact_workflow_node_kinds(GuaranteeClass::AtLeastOnce, &nodes);
        assert!(result.is_err());
    }

    #[test]
    fn best_effort_workflow_with_unsafe_node_passes() {
        use vo_types::{GuaranteeClass, NodeKind};

        let nodes = vec![NodeDescriptor {
            name: "allowed_unsafe".to_string(),
            kind: NodeKind::Unsafe,
        }];

        let result =
            validate_exact_workflow_node_kinds(GuaranteeClass::BestEffort, &nodes);
        assert!(result.is_ok());
    }

    #[test]
    fn error_code_returns_correct_value() {
        let err = UnsafeNodeError::UnsafeNodeInExactWorkflow {
            node_name: "x".to_string(),
            node_index: 0,
        };
        assert_eq!(err.error_code(), "unsafe_node_in_exact_workflow");
    }

    #[test]
    fn empty_node_list_passes() {
        use vo_types::GuaranteeClass;

        let result = validate_exact_workflow_node_kinds(GuaranteeClass::ExactOnce, &[]);
        assert!(result.is_ok());
    }
}
