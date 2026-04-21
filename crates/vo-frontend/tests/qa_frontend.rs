//! QA integration tests for vo-frontend components, props, and state management.

use vo_frontend::ui::domain_types::{NodeTemplateId, TemplateCategory};
use vo_frontend::ui::graph::{Node, NodeCategory, NodeId, Workflow};

#[test]
fn node_render_invariants_category_matches_kind() {
    let kinds = [
        (vo_types::NodeKind::Pure, NodeCategory::Flow, "zap"),
        (
            vo_types::NodeKind::ManagedEffect,
            NodeCategory::Durable,
            "database",
        ),
        (vo_types::NodeKind::Wait, NodeCategory::Timing, "clock"),
        (vo_types::NodeKind::Signal, NodeCategory::Signal, "wifi"),
        (vo_types::NodeKind::Unsafe, NodeCategory::Flow, "zap"),
    ];
    for (kind, expected_cat, expected_icon) in kinds {
        let node = Node::new(NodeId::new(), "test".into(), kind).expect("valid name");
        assert_eq!(
            node.category, expected_cat,
            "category mismatch for {kind:?}"
        );
        assert_eq!(node.icon, expected_icon, "icon mismatch for {kind:?}");
    }
}

#[test]
fn workflow_render_empty_produces_valid_nodes_by_id() {
    let wf = Workflow::new("empty".into());
    assert!(wf.nodes_by_id().is_empty());
}

#[test]
fn command_palette_filters_by_substring() {
    let all = vo_frontend::ui::command_palette::filtered_templates("");
    let http = vo_frontend::ui::command_palette::filtered_templates("http");
    assert!(all.len() > http.len());
    assert!(http.iter().all(|t| t.label.to_lowercase().contains("http")));
}

#[test]
fn template_categories_partition_all_templates() {
    let all: std::collections::HashSet<NodeTemplateId> =
        NodeTemplateId::all().into_iter().collect();
    let categorized: std::collections::HashSet<NodeTemplateId> = TemplateCategory::all()
        .iter()
        .flat_map(|cat| cat.members().iter().copied())
        .collect();
    assert_eq!(
        all, categorized,
        "every template must belong to exactly one category"
    );
}

#[test]
fn escape_key_detection() {
    use vo_frontend::ui::command_palette::is_escape_key;
    assert!(is_escape_key("Escape"));
    assert!(is_escape_key("esc"));
    assert!(is_escape_key("ESC"));
    assert!(!is_escape_key("Enter"));
}

#[test]
fn workflow_mutation_add_remove_get_consistent() {
    let mut wf = Workflow::new("test".into());
    let id = NodeId::new();
    wf.add_node(Node::new(id.clone(), "a".into(), vo_types::NodeKind::Pure).expect("valid name"));
    assert!(wf.get_node(id.clone()).is_some());
    wf.remove_node(id.clone());
    assert!(wf.get_node(id).is_none());
    assert!(wf.nodes.is_empty());
}

#[test]
fn skeleton_generation_empty_and_populated() {
    use vo_frontend::ui::prototype_palette::{generate_skeleton, SketchNode};
    let empty = generate_skeleton(&[]);
    assert!(empty.contains("steps:"));
    assert!(!empty.contains("step-"));
    let nodes = vec![SketchNode::new(NodeTemplateId::HttpHandler)];
    let skel = generate_skeleton(&nodes);
    assert!(skel.contains("http-handler"));
    assert!(skel.contains("step-1"));
}

#[test]
fn command_palette_empty_query_returns_all_templates() {
    let results = vo_frontend::ui::command_palette::filtered_templates("");
    assert_eq!(results.len(), NodeTemplateId::all().len());
}

#[test]
fn node_set_kind_updates_category_and_icon() {
    let mut node = Node::new(NodeId::new(), "test".into(), vo_types::NodeKind::Pure).expect("valid name");
    assert_eq!(node.category, NodeCategory::Flow);
    assert_eq!(node.icon, "zap");
    node.set_kind(vo_types::NodeKind::ManagedEffect);
    assert_eq!(node.category, NodeCategory::Durable);
    assert_eq!(node.icon, "database");
}

#[test]
fn node_config_update_merges_fields() {
    let mut node = Node::new(NodeId::new(), "cfg".into(), vo_types::NodeKind::Pure).expect("valid name");
    node.apply_config_update(&serde_json::json!({"url": "http://localhost"}));
    assert_eq!(node.config["url"], "http://localhost");
    node.apply_config_update(&serde_json::json!({"method": "POST"}));
    assert_eq!(node.config["url"], "http://localhost");
    assert_eq!(node.config["method"], "POST");
}

#[test]
fn node_id_generates_unique_26_char_ulid() {
    let a = NodeId::new();
    let b = NodeId::new();
    assert_eq!(a.0.len(), 26);
    assert_eq!(b.0.len(), 26);
    assert_ne!(a, b);
}

#[test]
fn command_palette_case_insensitive_filter() {
    let results = vo_frontend::ui::command_palette::filtered_templates("HTTP");
    assert!(!results.is_empty());
    assert!(results
        .iter()
        .all(|t| t.label.to_lowercase().contains("http")));
}

#[test]
fn workflow_nodes_by_id_lookup_consistent() {
    let mut wf = Workflow::new("lookup".into());
    let id = NodeId::new();
    wf.add_node(Node::new(
        id.clone(),
        "x".into(),
        vo_types::NodeKind::Signal,
    ).expect("valid name"));
    let map = wf.nodes_by_id();
    assert_eq!(map.len(), 1);
    assert_eq!(map[&id.0].name, "x");
}

#[test]
fn skeleton_chain_generates_depends_on() {
    use vo_frontend::ui::prototype_palette::{generate_skeleton, SketchNode};
    let nodes = vec![
        SketchNode::new(NodeTemplateId::HttpHandler),
        SketchNode::new(NodeTemplateId::Run),
    ];
    let skel = generate_skeleton(&nodes);
    assert!(skel.contains("depends_on: [step-1]"));
    assert!(!skel.contains("depends_on: [step-2]"));
}
