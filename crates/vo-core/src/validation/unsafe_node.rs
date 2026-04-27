//! Publish-time validation: reject Unsafe nodes in exact-once workflows (ADR-003, ADR-031).
//!
//! # Architecture
//!
//! - Data: `GuaranteeClass` + `NodeKind` (from vo-types, no I/O)
//! - Calc: `validate_no_unsafe_in_exact_workflow` pure function
//! - Error: `UnsafeNodeInExactWorkflow` for rejection reporting
//!
//! # Validation Contract
//!
//! Per ADR-003 / ADR-031:
//! - WHEN a workflow with guarantee class `ExactOnce` or `AtLeastOnce` is published
//!   and contains a node of kind `Unsafe`
//!   THE SYSTEM SHALL reject the publication synchronously
//! - Only `BestEffort` workflows may contain `Unsafe` nodes

use thiserror::Error;
use vo_types::{GuaranteeClass, NodeKind};

/// Error returned when an exact workflow contains an Unsafe node.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("exact-once workflow contains unsafe node: {node_name}")]
pub struct UnsafeNodeInExactWorkflow {
    pub node_name: String,
}

impl UnsafeNodeInExactWorkflow {
    #[must_use]
    pub fn error_code() -> &'static str {
        "unsafe_node_in_exact_workflow"
    }
}

/// Validate that a workflow with the given guarantee class does not contain
/// Unsafe nodes when the guarantee class does not permit them.
///
/// Only `BestEffort` workflows may contain `Unsafe` nodes.
/// Both `ExactOnce` and `AtLeastOnce` workflows reject `Unsafe` nodes.
///
/// # Errors
///
/// Returns `UnsafeNodeInExactWorkflow` if the guarantee class does not permit
/// unsafe nodes and an `Unsafe` node kind is found in the provided node kinds.
pub fn validate_no_unsafe_in_exact_workflow(
    guarantee_class: GuaranteeClass,
    node_kinds: &[(NodeKind, &str)],
) -> Result<(), UnsafeNodeInExactWorkflow> {
    if guarantee_class.permits_unsafe_nodes() {
        return Ok(());
    }
    for (kind, name) in node_kinds {
        if *kind == NodeKind::Unsafe {
            return Err(UnsafeNodeInExactWorkflow {
                node_name: (*name).to_string(),
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
        // Given an exact workflow spec contains an Unsafe node
        let guarantee_class = GuaranteeClass::ExactOnce;
        let node_kinds = [
            (NodeKind::Pure, "step_a"),
            (NodeKind::ManagedEffect, "step_b"),
            (NodeKind::Unsafe, "dangerous_step"),
        ];

        // When publish validation runs
        let result = validate_no_unsafe_in_exact_workflow(guarantee_class, &node_kinds);

        // Then publish fails and no workflow version is activated
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.node_name, "dangerous_step");
        assert!(err.to_string().contains("dangerous_step"));
    }

    #[test]
    fn exact_workflow_without_unsafe_nodes_passes() {
        let result = validate_no_unsafe_in_exact_workflow(
            GuaranteeClass::ExactOnce,
            &[
                (NodeKind::Pure, "step_a"),
                (NodeKind::ManagedEffect, "step_b"),
                (NodeKind::Signal, "step_c"),
            ],
        );
        assert!(result.is_ok());
    }

    #[test]
    fn at_least_once_workflow_with_unsafe_node_rejects() {
        let result = validate_no_unsafe_in_exact_workflow(
            GuaranteeClass::AtLeastOnce,
            &[(NodeKind::Unsafe, "fire_and_forget")],
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().node_name, "fire_and_forget");
    }

    #[test]
    fn best_effort_workflow_with_unsafe_node_passes() {
        let result = validate_no_unsafe_in_exact_workflow(
            GuaranteeClass::BestEffort,
            &[(NodeKind::Unsafe, "allowed_unsafe")],
        );
        assert!(result.is_ok());
    }

    #[test]
    fn best_effort_workflow_with_multiple_unsafe_nodes_passes() {
        let result = validate_no_unsafe_in_exact_workflow(
            GuaranteeClass::BestEffort,
            &[
                (NodeKind::Unsafe, "unsafe_a"),
                (NodeKind::Unsafe, "unsafe_b"),
                (NodeKind::Pure, "pure_c"),
            ],
        );
        assert!(result.is_ok());
    }

    #[test]
    fn error_code_is_correct() {
        assert_eq!(
            UnsafeNodeInExactWorkflow::error_code(),
            "unsafe_node_in_exact_workflow"
        );
    }

    #[test]
    fn empty_node_list_passes_for_exact_once() {
        let result = validate_no_unsafe_in_exact_workflow(GuaranteeClass::ExactOnce, &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn returns_first_unsafe_node_found() {
        let result = validate_no_unsafe_in_exact_workflow(
            GuaranteeClass::ExactOnce,
            &[
                (NodeKind::Pure, "step_a"),
                (NodeKind::Unsafe, "first_unsafe"),
                (NodeKind::Unsafe, "second_unsafe"),
            ],
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().node_name, "first_unsafe");
    }
}
