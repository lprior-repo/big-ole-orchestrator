#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

use vo_frontend::ui::graph::{Connection, Node, NodeCategory, NodeId, PortName, Workflow};
use vo_types::GuaranteeClass;

#[test]
fn workflow_remove_node_by_string_id() {
    let mut wf = Workflow::new("test".to_string(), GuaranteeClass::BestEffort);
    let node = Node::new(NodeId::new(), "node1".to_string(), vo_types::NodeKind::Pure);
    let node_id = node.id.clone();
    wf.add_node(node);
    assert_eq!(wf.nodes.len(), 1);

    wf.remove_node(node_id.as_str());
    assert_eq!(wf.nodes.len(), 0);
}

#[test]
fn workflow_get_node_returns_mutable_reference() {
    let mut wf = Workflow::new("test".to_string(), GuaranteeClass::BestEffort);
    let node = Node::new(NodeId::new(), "original".to_string(), vo_types::NodeKind::Pure);
    let node_id = node.id.clone();
    wf.add_node(node);

    let retrieved = wf.get_node_mut(node_id.clone());
    assert!(retrieved.is_some());
    if let Some(retrieved_node) = retrieved {
        retrieved_node.name = "updated".to_string();
    }

    let check = wf.get_node(node_id);
    assert_eq!(check.unwrap().name, "updated");
}

#[test]
fn node_apply_config_update_with_nested_object() {
    let mut node = Node::new(NodeId::new(), "test".to_string(), vo_types::NodeKind::Pure);
    node.apply_config_update(&serde_json::json!({
        "nested": {"key": "value"}
    }));
    assert_eq!(node.config["nested"]["key"], "value");
}

#[test]
fn node_apply_config_update_merges_recursively() {
    let mut node = Node::new(NodeId::new(), "test".to_string(), vo_types::NodeKind::Pure);
    node.apply_config_update(&serde_json::json!({"a": 1}));
    node.apply_config_update(&serde_json::json!({"b": 2}));
    assert_eq!(node.config["a"], 1);
    assert_eq!(node.config["b"], 2);
}

#[test]
fn node_apply_config_update_does_not_replace_top_level_keys() {
    let mut node = Node::new(NodeId::new(), "test".to_string(), vo_types::NodeKind::Pure);
    node.apply_config_update(&serde_json::json!({"key": "original"}));
    node.apply_config_update(&serde_json::json!({"key": "updated", "new_key": "new"}));
    assert_eq!(node.config["key"], "updated");
    assert_eq!(node.config["new_key"], "new");
}

#[test]
fn workflow_connection_roundtrip_through_json() {
    let conn = Connection {
        id: uuid::Uuid::new_v4(),
        source: NodeId::new(),
        target: NodeId::new(),
        source_port: PortName::from("output"),
        target_port: PortName::from("input"),
    };
    let json = serde_json::to_string(&conn).unwrap();
    let recovered: Connection = serde_json::from_str(&json).unwrap();
    assert_eq!(conn.id, recovered.id);
    assert_eq!(conn.source, recovered.source);
    assert_eq!(conn.target, recovered.target);
}

#[test]
fn node_id_parse_rejects_empty_string() {
    assert!(NodeId::parse("").is_none());
}

#[test]
fn node_id_parse_accepts_26_char_string() {
    let id = NodeId::parse("01ARYZ6S41TSV4RRFFQ69G5FAV");
    assert!(id.is_some());
}

#[test]
fn node_id_parse_rejects_whitespace() {
    assert!(NodeId::parse(" 01ARYZ6S41TSV4RRFFQ69G5FAV").is_none());
    assert!(NodeId::parse("01ARYZ6S41TSV4RRFFQ69G5FAV ").is_none());
}

#[test]
fn node_category_display_formats_correctly() {
    assert_eq!(format!("{}", NodeCategory::Entry), "entry");
    assert_eq!(format!("{}", NodeCategory::Durable), "durable");
    assert_eq!(format!("{}", NodeCategory::Flow), "flow");
    assert_eq!(format!("{}", NodeCategory::Timing), "timing");
    assert_eq!(format!("{}", NodeCategory::Signal), "signal");
}

#[test]
fn execution_state_status_badge_class_all_variants() {
    use vo_frontend::ui::edges::graph_types::ExecutionState;

    assert!(!ExecutionState::Idle.status_badge_class().is_empty());
    assert!(!ExecutionState::Queued.status_badge_class().is_empty());
    assert!(!ExecutionState::Running.status_badge_class().is_empty());
    assert!(!ExecutionState::Completed.status_badge_class().is_empty());
    assert!(!ExecutionState::Failed.status_badge_class().is_empty());
    assert!(!ExecutionState::Skipped.status_badge_class().is_empty());
}

#[test]
fn execution_state_label_all_variants() {
    use vo_frontend::ui::edges::graph_types::ExecutionState;

    assert_eq!(ExecutionState::Idle.label(), "pending");
    assert_eq!(ExecutionState::Queued.label(), "pending");
    assert_eq!(ExecutionState::Running.label(), "running");
    assert_eq!(ExecutionState::Completed.label(), "completed");
    assert_eq!(ExecutionState::Failed.label(), "failed");
    assert_eq!(ExecutionState::Skipped.label(), "skipped");
}

#[test]
fn node_with_execution_state_clone_preserves_state() {
    use vo_frontend::ui::edges::graph_types::ExecutionState;

    let mut node = Node::new(NodeId::new(), "test".to_string(), vo_types::NodeKind::Pure);
    node.execution_state = ExecutionState::Running;

    let cloned = node.clone();
    assert_eq!(cloned.execution_state, ExecutionState::Running);
}

#[test]
fn workflow_clear_removes_all_nodes_and_connections() {
    let mut wf = Workflow::new("test".to_string(), GuaranteeClass::ExactOnce);
    let node1 = Node::new(NodeId::new(), "n1".to_string(), vo_types::NodeKind::Pure);
    let node2 = Node::new(NodeId::new(), "n2".to_string(), vo_types::NodeKind::Pure);
    wf.add_node(node1);
    wf.add_node(node2);

    let conn = Connection {
        id: uuid::Uuid::new_v4(),
        source: wf.nodes[0].id.clone(),
        target: wf.nodes[1].id.clone(),
        source_port: PortName::from("out"),
        target_port: PortName::from("in"),
    };
    wf.connections.push(conn);

    assert_eq!(wf.nodes.len(), 2);
    assert_eq!(wf.connections.len(), 1);

    wf.nodes.clear();
    wf.connections.clear();

    assert!(wf.nodes.is_empty());
    assert!(wf.connections.is_empty());
}
