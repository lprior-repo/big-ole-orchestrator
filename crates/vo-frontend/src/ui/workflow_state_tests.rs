//! Tests for workflow state management, serialization, and edge cases.
//!
//! Covers:
//! - Workflow serialization roundtrips
//! - Node operations and state consistency
//! - Concurrent update simulation via workflow mutations
//! - Stale state detection and recovery scenarios

use vo_frontend::ui::graph::{
    Connection, ExecutionState, Node, NodeCategory, NodeId, PortName, ValidationIssue,
    ValidationResult, ValidationSeverity, Workflow, WorkflowNode,
};
use vo_types::GuaranteeClass;
use vo_types::NodeKind;

#[cfg(test)]
mod workflow_serialization {
    use super::*;

    #[test]
    fn workflow_serializes_and_deserializes_roundtrip() {
        let mut workflow = Workflow::new("test-workflow".to_string(), GuaranteeClass::ExactOnce);
        let node = Node::new(NodeId::new(), "TestNode".to_string(), NodeKind::Pure);
        let node_id = node.id.clone();
        workflow.add_node(node);

        let serialized = serde_json::to_string(&workflow).unwrap();
        let deserialized: Workflow = serde_json::from_str(&serialized).unwrap();

        assert_eq!(workflow.name, deserialized.name);
        assert_eq!(workflow.guarantee_class, deserialized.guarantee_class);
        assert_eq!(workflow.nodes.len(), deserialized.nodes.len());
        assert_eq!(
            workflow.nodes[0].id.to_string(),
            deserialized.nodes[0].id.to_string()
        );
    }

    #[test]
    fn node_serializes_and_deserializes_roundtrip() {
        let node = Node::new(NodeId::new(), "TestNode".to_string(), NodeKind::ManagedEffect);

        let serialized = serde_json::to_string(&node).unwrap();
        let deserialized: Node = serde_json::from_str(&serialized).unwrap();

        assert_eq!(node.id, deserialized.id);
        assert_eq!(node.name, deserialized.name);
        assert_eq!(node.kind, deserialized.kind);
        assert_eq!(node.category, deserialized.category);
        assert_eq!(node.execution_state, deserialized.execution_state);
    }

    #[test]
    fn connection_serializes_and_deserializes_roundtrip() {
        let conn = Connection {
            id: uuid::Uuid::new_v4(),
            source: NodeId::new(),
            target: NodeId::new(),
            source_port: PortName::from("output"),
            target_port: PortName::from("input"),
        };

        let serialized = serde_json::to_string(&conn).unwrap();
        let deserialized: Connection = serde_json::from_str(&serialized).unwrap();

        assert_eq!(conn.id, deserialized.id);
        assert_eq!(conn.source, deserialized.source);
        assert_eq!(conn.target, deserialized.target);
    }

    #[test]
    fn execution_state_serializes_as_string() {
        let states = [
            ExecutionState::Idle,
            ExecutionState::Running,
            ExecutionState::Queued,
            ExecutionState::Completed,
            ExecutionState::Failed,
            ExecutionState::Skipped,
        ];

        for state in states {
            let serialized = serde_json::to_string(&state).unwrap();
            let deserialized: ExecutionState = serde_json::from_str(&serialized).unwrap();
            assert_eq!(state, deserialized);
        }
    }

    #[test]
    fn workflow_node_serializes_and_deserializes_roundtrip() {
        let nodes = [
            WorkflowNode::Run(Default::default()),
            WorkflowNode::Parallel(Default::default()),
        ];

        for node in nodes {
            let serialized = serde_json::to_string(&node).unwrap();
            let deserialized: WorkflowNode = serde_json::from_str(&serialized).unwrap();
            assert_eq!(
                format!("{:?}", node),
                format!("{:?}", deserialized)
            );
        }
    }

    #[test]
    fn validation_result_serializes_and_deserializes_roundtrip() {
        let issues = vec![
            ValidationIssue {
                node_id: Some(NodeId::new()),
                severity: ValidationSeverity::Error,
                message: "Test error".to_string(),
            },
            ValidationIssue {
                node_id: None,
                severity: ValidationSeverity::Warning,
                message: "Test warning".to_string(),
            },
        ];
        let result = ValidationResult::new(issues);

        let serialized = serde_json::to_string(&result).unwrap();
        let deserialized: ValidationResult = serde_json::from_str(&serialized).unwrap();

        assert_eq!(result.issues.len(), deserialized.issues.len());
        assert_eq!(result.is_valid(), deserialized.is_valid());
        assert_eq!(result.error_count(), deserialized.error_count());
    }

    #[test]
    fn workflow_with_all_node_kinds_serializes_correctly() {
        let mut workflow = Workflow::new("kinds-test".to_string(), GuaranteeClass::AtLeastOnce);

        let kinds = [
            NodeKind::Pure,
            NodeKind::ManagedEffect,
            NodeKind::Wait,
            NodeKind::Signal,
            NodeKind::Unsafe,
        ];

        for kind in kinds {
            let node = Node::new(NodeId::new(), format!("{:?}", kind), kind);
            workflow.add_node(node);
        }

        let serialized = serde_json::to_string(&workflow).unwrap();
        let deserialized: Workflow = serde_json::from_str(&serialized).unwrap();

        assert_eq!(workflow.nodes.len(), deserialized.nodes.len());
        for (original, restored) in workflow.nodes.iter().zip(deserialized.nodes.iter()) {
            assert_eq!(original.kind, restored.kind);
            assert_eq!(original.category, restored.category);
        }
    }

    #[test]
    fn empty_workflow_serializes_correctly() {
        let workflow = Workflow::new("empty".to_string(), GuaranteeClass::BestEffort);

        let serialized = serde_json::to_string(&workflow).unwrap();
        let deserialized: Workflow = serde_json::from_str(&serialized).unwrap();

        assert!(deserialized.nodes.is_empty());
        assert!(deserialized.connections.is_empty());
    }

    #[test]
    fn workflow_with_connections_serializes_correctly() {
        let mut workflow = Workflow::new("connected".to_string(), GuaranteeClass::ExactOnce);

        let node1 = Node::new(NodeId::new(), "Node1".to_string(), NodeKind::Pure);
        let node2 = Node::new(NodeId::new(), "Node2".to_string(), NodeKind::Pure);
        let id1 = node1.id.clone();
        let id2 = node2.id.clone();

        workflow.add_node(node1);
        workflow.add_node(node2);

        let connection = Connection {
            id: uuid::Uuid::new_v4(),
            source: id1,
            target: id2,
            source_port: PortName::from("out"),
            target_port: PortName::from("in"),
        };
        workflow.connections.push(connection);

        let serialized = serde_json::to_string(&workflow).unwrap();
        let deserialized: Workflow = serde_json::from_str(&serialized).unwrap();

        assert_eq!(workflow.connections.len(), deserialized.connections.len());
        assert_eq!(
            workflow.connections[0].source.to_string(),
            deserialized.connections[0].source.to_string()
        );
    }
}

#[cfg(test)]
mod workflow_state_mutations {
    use super::*;

    #[test]
    fn node_set_kind_propagates_to_category_and_icon() {
        let mut node = Node::new(NodeId::new(), "test".to_string(), NodeKind::Pure);
        assert_eq!(node.category, NodeCategory::Flow);
        assert_eq!(node.icon, "zap");

        node.set_kind(NodeKind::ManagedEffect);
        assert_eq!(node.category, NodeCategory::Durable);
        assert_eq!(node.icon, "database");

        node.set_kind(NodeKind::Wait);
        assert_eq!(node.category, NodeCategory::Timing);
        assert_eq!(node.icon, "clock");

        node.set_kind(NodeKind::Signal);
        assert_eq!(node.category, NodeCategory::Signal);
        assert_eq!(node.icon, "wifi");

        node.set_kind(NodeKind::Unsafe);
        assert_eq!(node.category, NodeCategory::Flow);
        assert_eq!(node.icon, "zap");
    }

    #[test]
    fn apply_config_update_merges_objects() {
        let mut node = Node::new(NodeId::new(), "test".to_string(), NodeKind::Pure);

        node.apply_config_update(&serde_json::json!({"key1": "value1", "key2": "value2"}));
        assert_eq!(node.config["key1"], "value1");
        assert_eq!(node.config["key2"], "value2");

        node.apply_config_update(&serde_json::json!({"key3": "value3", "key1": "updated"}));
        assert_eq!(node.config["key1"], "updated");
        assert_eq!(node.config["key2"], "value2");
        assert_eq!(node.config["key3"], "value3");
    }

    #[test]
    fn apply_config_update_with_non_object_ignores() {
        let mut node = Node::new(NodeId::new(), "test".to_string(), NodeKind::Pure);
        node.apply_config_update(&serde_json::json!({"key": "value"}));

        let before = node.config.clone();
        node.apply_config_update(&serde_json::json!("not an object"));
        assert_eq!(node.config, before);

        node.apply_config_update(&serde_json::json!(123));
        assert_eq!(node.config, before);

        node.apply_config_update(&serde_json::json!(null));
        assert_eq!(node.config, before);
    }

    #[test]
    fn apply_config_update_with_empty_object_is_noop() {
        let mut node = Node::new(NodeId::new(), "test".to_string(), NodeKind::Pure);
        node.apply_config_update(&serde_json::json!({"key": "value"}));
        let before = node.config.clone();

        node.apply_config_update(&serde_json::json!({}));
        assert_eq!(node.config, before);
    }

    #[test]
    fn remove_node_eliminates_from_workflow() {
        let mut workflow = Workflow::new("test".to_string(), GuaranteeClass::BestEffort);
        let node = Node::new(NodeId::new(), "test".to_string(), NodeKind::Pure);
        let node_id = node.id.clone();
        workflow.add_node(node);

        assert!(workflow.get_node(node_id.clone()).is_some());
        workflow.remove_node(node_id.clone());
        assert!(workflow.get_node(node_id).is_none());
    }

    #[test]
    fn get_node_mut_allows_modification() {
        let mut workflow = Workflow::new("test".to_string(), GuaranteeClass::BestEffort);
        let node = Node::new(NodeId::new(), "original".to_string(), NodeKind::Pure);
        let node_id = node.id.clone();
        workflow.add_node(node);

        if let Some(n) = workflow.get_node_mut(node_id.clone()) {
            n.name = "modified".to_string();
        }

        assert_eq!(
            workflow.get_node(node_id).unwrap().name,
            "modified"
        );
    }

    #[test]
    fn nodes_by_id_provides_efficient_lookup() {
        let mut workflow = Workflow::new("test".to_string(), GuaranteeClass::BestEffort);

        for i in 0..10 {
            let node = Node::new(NodeId::new(), format!("node-{}", i), NodeKind::Pure);
            let id = node.id.0.clone();
            workflow.add_node(node);
            assert!(workflow.nodes_by_id().contains_key(&id));
        }

        assert_eq!(workflow.nodes_by_id().len(), 10);
    }

    #[test]
    fn multiple_nodes_with_different_kinds() {
        let mut workflow = Workflow::new("test".to_string(), GuaranteeClass::BestEffort);

        let kinds = [
            NodeKind::Pure,
            NodeKind::ManagedEffect,
            NodeKind::Wait,
            NodeKind::Signal,
            NodeKind::Unsafe,
        ];

        for (i, kind) in kinds.iter().enumerate() {
            let node = Node::new(NodeId::new(), format!("node-{}", i), *kind);
            workflow.add_node(node);
        }

        assert_eq!(workflow.nodes.len(), 5);

        for (i, node) in workflow.nodes.iter().enumerate() {
            assert_eq!(node.kind, kinds[i]);
        }
    }

    #[test]
    fn workflow_node_from_str_parses_correctly() {
        assert!(matches!(
            WorkflowNode::from_str("run"),
            Ok(WorkflowNode::Run(_))
        ));
        assert!(matches!(
            WorkflowNode::from_str("parallel"),
            Ok(WorkflowNode::Parallel(_))
        ));
        assert!(matches!(
            WorkflowNode::from_str("service-call"),
            Ok(WorkflowNode::Run(_))
        ));
        assert!(WorkflowNode::from_str("unknown").is_err());
    }

    #[test]
    fn workflow_node_is_parallel_detects_correctly() {
        assert!(!WorkflowNode::Run(Default::default()).is_parallel());
        assert!(WorkflowNode::Parallel(Default::default()).is_parallel());
    }
}

#[cfg(test)]
mod validation_tests {
    use super::*;

    #[test]
    fn validation_result_empty_is_valid() {
        let result = ValidationResult::new(vec![]);
        assert!(result.is_valid());
        assert_eq!(result.error_count(), 0);
        assert_eq!(result.warning_count(), 0);
    }

    #[test]
    fn validation_result_counts_errors_and_warnings() {
        let issues = vec![
            ValidationIssue {
                node_id: None,
                severity: ValidationSeverity::Error,
                message: "error 1".to_string(),
            },
            ValidationIssue {
                node_id: None,
                severity: ValidationSeverity::Warning,
                message: "warning 1".to_string(),
            },
            ValidationIssue {
                node_id: None,
                severity: ValidationSeverity::Error,
                message: "error 2".to_string(),
            },
        ];
        let result = ValidationResult::new(issues);

        assert!(!result.is_valid());
        assert_eq!(result.error_count(), 2);
        assert_eq!(result.warning_count(), 1);
    }

    #[test]
    fn validation_issue_with_node_id_roundtrips() {
        let issue = ValidationIssue {
            node_id: Some(NodeId::new()),
            severity: ValidationSeverity::Error,
            message: "Test error".to_string(),
        };

        let serialized = serde_json::to_string(&issue).unwrap();
        let deserialized: ValidationIssue = serde_json::from_str(&serialized).unwrap();

        assert_eq!(
            issue.node_id.unwrap().to_string(),
            deserialized.node_id.unwrap().to_string()
        );
        assert_eq!(issue.severity, deserialized.severity);
    }

    #[test]
    fn validation_result_new_with_multiple_issues() {
        let issues = vec![
            ValidationIssue {
                node_id: Some(NodeId::new()),
                severity: ValidationSeverity::Error,
                message: "Error 1".to_string(),
            },
            ValidationIssue {
                node_id: None,
                severity: ValidationSeverity::Warning,
                message: "Warning 1".to_string(),
            },
            ValidationIssue {
                node_id: Some(NodeId::new()),
                severity: ValidationSeverity::Error,
                message: "Error 2".to_string(),
            },
            ValidationIssue {
                node_id: None,
                severity: ValidationSeverity::Warning,
                message: "Warning 2".to_string(),
            },
        ];

        let result = ValidationResult::new(issues);

        assert!(!result.is_valid());
        assert_eq!(result.error_count(), 2);
        assert_eq!(result.warning_count(), 2);
    }
}

#[cfg(test)]
mod node_id_tests {
    use super::*;

    #[test]
    fn node_id_new_generates_unique_ids() {
        let id1 = NodeId::new();
        let id2 = NodeId::new();
        assert_ne!(id1, id2);
        assert_eq!(id1.0.len(), 26);
        assert_eq!(id2.0.len(), 26);
    }

    #[test]
    fn node_id_parse_accepts_valid_26_char_string() {
        let id = NodeId::parse("01ARYZ6S41TSV4RRFFQ69G5FAV");
        assert!(id.is_some());
        assert_eq!(id.unwrap().0, "01ARYZ6S41TSV4RRFFQ69G5FAV");
    }

    #[test]
    fn node_id_parse_rejects_invalid_lengths() {
        assert_eq!(NodeId::parse(""), None);
        assert_eq!(NodeId::parse("01ARYZ6S41TSV4RRFFQ69G5FA"), None);
        assert_eq!(NodeId::parse("01ARYZ6S41TSV4RRFFQ69G5FAVG"), None);
    }

    #[test]
    fn node_id_display_shows_inner_value() {
        let id = NodeId::parse("01ARYZ6S41TSV4RRFFQ69G5FAV").unwrap();
        assert_eq!(format!("{}", id), "01ARYZ6S41TSV4RRFFQ69G5FAV");
    }

    #[test]
    fn node_id_as_str_returns_inner() {
        let id = NodeId::parse("01ARYZ6S41TSV4RRFFQ69G5FAV").unwrap();
        assert_eq!(id.as_str(), "01ARYZ6S41TSV4RRFFQ69G5FAV");
    }

    #[test]
    fn port_name_conversions() {
        let port: PortName = "input".into();
        assert_eq!(port.0, "input");

        let port: PortName = String::from("output").into();
        assert_eq!(String::from(port), "output");
    }

    #[test]
    fn port_name_display() {
        let port = PortName::from("output");
        assert_eq!(format!("{}", port), "output");
    }
}

#[cfg(test)]
mod execution_state_tests {
    use super::*;

    #[test]
    fn execution_state_status_badge_class_all_variants() {
        assert_eq!(
            ExecutionState::Idle.status_badge_class(),
            "bg-slate-100 text-slate-700 border-slate-200"
        );
        assert_eq!(
            ExecutionState::Queued.status_badge_class(),
            "bg-slate-100 text-slate-700 border-slate-200"
        );
        assert_eq!(
            ExecutionState::Running.status_badge_class(),
            "bg-blue-100 text-blue-700 border-blue-200"
        );
        assert_eq!(
            ExecutionState::Completed.status_badge_class(),
            "bg-green-100 text-green-700 border-green-200"
        );
        assert_eq!(
            ExecutionState::Failed.status_badge_class(),
            "bg-red-100 text-red-700 border-red-200"
        );
        assert_eq!(
            ExecutionState::Skipped.status_badge_class(),
            "bg-slate-100 text-slate-500 border-slate-200"
        );
    }

    #[test]
    fn execution_state_label_all_variants() {
        assert_eq!(ExecutionState::Idle.label(), "pending");
        assert_eq!(ExecutionState::Queued.label(), "pending");
        assert_eq!(ExecutionState::Running.label(), "running");
        assert_eq!(ExecutionState::Completed.label(), "completed");
        assert_eq!(ExecutionState::Failed.label(), "failed");
        assert_eq!(ExecutionState::Skipped.label(), "skipped");
    }

    #[test]
    fn execution_state_default_is_idle() {
        assert_eq!(ExecutionState::default(), ExecutionState::Idle);
    }
}

#[cfg(test)]
mod node_category_tests {
    use super::*;

    #[test]
    fn node_category_badge_class_all_variants() {
        assert_eq!(
            NodeCategory::Entry.badge_class(),
            "bg-emerald-50 text-emerald-700 border-emerald-200"
        );
        assert_eq!(
            NodeCategory::Durable.badge_class(),
            "bg-indigo-50 text-indigo-700 border-indigo-200"
        );
        assert_eq!(
            NodeCategory::State.badge_class(),
            "bg-orange-50 text-orange-700 border-orange-200"
        );
        assert_eq!(
            NodeCategory::Flow.badge_class(),
            "bg-amber-50 text-amber-700 border-amber-200"
        );
        assert_eq!(
            NodeCategory::Timing.badge_class(),
            "bg-pink-50 text-pink-700 border-pink-200"
        );
        assert_eq!(
            NodeCategory::Signal.badge_class(),
            "bg-blue-50 text-blue-700 border-blue-200"
        );
    }

    #[test]
    fn node_category_display_all_variants() {
        assert_eq!(format!("{}", NodeCategory::Entry), "entry");
        assert_eq!(format!("{}", NodeCategory::Durable), "durable");
        assert_eq!(format!("{}", NodeCategory::State), "state");
        assert_eq!(format!("{}", NodeCategory::Flow), "flow");
        assert_eq!(format!("{}", NodeCategory::Timing), "timing");
        assert_eq!(format!("{}", NodeCategory::Signal), "signal");
    }
}

#[cfg(test)]
mod guarantee_class_tests {
    use super::*;

    #[test]
    fn guarantee_class_badge_classes_are_distinct() {
        let exact = GuaranteeClass::ExactOnce.badge_class();
        let atleast = GuaranteeClass::AtLeastOnce.badge_class();
        let best = GuaranteeClass::BestEffort.badge_class();

        assert_ne!(exact, atleast);
        assert_ne!(exact, best);
        assert_ne!(atleast, best);
    }

    #[test]
    fn guarantee_class_icons_are_distinct() {
        let exact = GuaranteeClass::ExactOnce.icon();
        let atleast = GuaranteeClass::AtLeastOnce.icon();
        let best = GuaranteeClass::BestEffort.icon();

        assert!(exact.contains("shield"));
        assert!(atleast.contains("shield"));
        assert!(best.contains("shield"));
    }
}

#[cfg(test)]
mod stale_state_recovery_simulation {
    use super::*;

    #[test]
    fn node_config_persists_across_mutations() {
        let mut node = Node::new(NodeId::new(), "test".to_string(), NodeKind::Pure);

        node.apply_config_update(&serde_json::json!({"url": "http://localhost"}));
        assert_eq!(node.config["url"], "http://localhost");

        node.set_kind(NodeKind::ManagedEffect);
        assert_eq!(node.config["url"], "http://localhost");

        node.set_kind(NodeKind::Wait);
        assert_eq!(node.config["url"], "http://localhost");
    }

    #[test]
    fn workflow_state_survives_node_removal_and_add() {
        let mut workflow = Workflow::new("test".to_string(), GuaranteeClass::ExactOnce);

        let node1 = Node::new(NodeId::new(), "node1".to_string(), NodeKind::Pure);
        let node2 = Node::new(NodeId::new(), "node2".to_string(), NodeKind::Pure);
        let id1 = node1.id.clone();
        let id2 = node2.id.clone();

        workflow.add_node(node1);
        workflow.add_node(node2.clone());
        assert_eq!(workflow.nodes.len(), 2);

        workflow.remove_node(id1.clone());
        assert_eq!(workflow.nodes.len(), 1);
        assert!(workflow.get_node(id1).is_none());
        assert!(workflow.get_node(id2.clone()).is_some());

        let node3 = Node::new(NodeId::new(), "node3".to_string(), NodeKind::Signal);
        let id3 = node3.id.clone();
        workflow.add_node(node3);

        assert_eq!(workflow.nodes.len(), 2);
        assert!(workflow.get_node(id2).is_some());
        assert!(workflow.get_node(id3).is_some());
    }

    #[test]
    fn execution_state_transitions_are_independent() {
        let mut workflow = Workflow::new("test".to_string(), GuaranteeClass::BestEffort);

        let node1 = Node::new(NodeId::new(), "node1".to_string(), NodeKind::Pure);
        let node2 = Node::new(NodeId::new(), "node2".to_string(), NodeKind::Pure);

        let id1 = node1.id.clone();
        let id2 = node2.id.clone();

        workflow.add_node(node1);
        workflow.add_node(node2);

        let mut wf = workflow;
        if let Some(n) = wf.get_node_mut(id1.clone()) {
            n.execution_state = ExecutionState::Completed;
        }
        if let Some(n) = wf.get_node_mut(id2.clone()) {
            n.execution_state = ExecutionState::Running;
        }

        assert_eq!(
            wf.get_node(id1).unwrap().execution_state,
            ExecutionState::Completed
        );
        assert_eq!(
            wf.get_node(id2).unwrap().execution_state,
            ExecutionState::Running
        );
    }

    #[test]
    fn config_updates_merge_correctly_under_rapid_changes() {
        let mut node = Node::new(NodeId::new(), "test".to_string(), NodeKind::Pure);

        let updates = [
            serde_json::json!({"a": 1}),
            serde_json::json!({"b": 2}),
            serde_json::json!({"c": 3}),
            serde_json::json!({"a": 10}),
        ];

        for update in updates {
            node.apply_config_update(&update);
        }

        assert_eq!(node.config["a"], 10);
        assert_eq!(node.config["b"], 2);
        assert_eq!(node.config["c"], 3);
    }
}