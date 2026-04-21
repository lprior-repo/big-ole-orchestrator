//! RED-QUEEN coevolutionary adversarial tests for vo-frontend.
//!
//! Attacks coevolve with defensive code. Each test targets an edge case where
//! a prior fix may regress: state corruption via config merges, Workflow name
//! abuse, badge class invariants, parse boundary attacks, and skeleton
//! generation invalid defaults.

use std::str::FromStr;
use vo_frontend::ui::domain_types::{HttpMethod, NodeTemplateId};
use vo_frontend::ui::graph::{node_kind_to_category, Node, NodeId, Workflow};
use vo_frontend::ui::prototype_palette::{generate_skeleton, SketchNode};

#[test]
fn config_update_overwrites_existing_key() {
    let mut node = Node::new(NodeId::new(), "x".into(), vo_types::NodeKind::Pure);
    node.apply_config_update(&serde_json::json!({"port": 8080}));
    node.apply_config_update(&serde_json::json!({"port": -1}));
    assert_eq!(
        node.config["port"], -1,
        "last write wins — attacker cannot pin stale value"
    );
}

#[test]
fn config_update_with_empty_object_is_noop() {
    let mut node = Node::new(NodeId::new(), "x".into(), vo_types::NodeKind::Pure);
    let before = node.config.clone();
    node.apply_config_update(&serde_json::json!({}));
    assert_eq!(node.config, before, "empty object merge must be identity");
}

#[test]
fn workflow_name_with_null_bytes_roundtrips() {
    let wf = Workflow::new("before\0after".into());
    let json = serde_json::to_string(&wf).unwrap();
    let recovered: Workflow = serde_json::from_str(&json).unwrap();
    assert!(
        recovered.name.contains('\0'),
        "null byte survives roundtrip — documented"
    );
}

#[test]
fn workflow_name_with_emoji_does_not_panic() {
    let mut wf = Workflow::new("🦀🔥💣".into());
    wf.add_node(Node::new(
        NodeId::new(),
        "node".into(),
        vo_types::NodeKind::Pure,
    ));
    let json = serde_json::to_string(&wf).unwrap();
    let _: Workflow = serde_json::from_str(&json).unwrap();
}

#[test]
fn badge_class_invariant_after_kind_flips() {
    let mut node = Node::new(NodeId::new(), "mutant".into(), vo_types::NodeKind::Pure);
    let kinds = [
        vo_types::NodeKind::Pure,
        vo_types::NodeKind::ManagedEffect,
        vo_types::NodeKind::Wait,
        vo_types::NodeKind::Signal,
        vo_types::NodeKind::Unsafe,
    ];
    for kind in kinds {
        node.set_kind(kind);
        let cat = node_kind_to_category(kind);
        assert_eq!(
            node.category, cat,
            "category must match kind after mutation"
        );
        assert!(!node.icon.is_empty(), "icon must never be empty");
        assert!(
            !cat.badge_class().is_empty(),
            "badge class must never be empty"
        );
    }
}

#[test]
fn nodeid_boundary_lengths() {
    assert!(
        NodeId::parse("01ARYZ6S41TSV4RRFFQ69G5FAV").is_some(),
        "26 chars accepted"
    );
    assert!(
        NodeId::parse("01ARYZ6S41TSV4RRFFQ69G5FA").is_none(),
        "25 chars rejected"
    );
    assert!(
        NodeId::parse("01ARYZ6S41TSV4RRFFQ69G5FAVG").is_none(),
        "27 chars rejected"
    );
}

#[test]
fn http_method_from_str_rejects_unknown() {
    assert!(
        HttpMethod::from_str("EVIL").is_err(),
        "unknown method must be rejected"
    );
    assert!(
        HttpMethod::from_str("").is_err(),
        "empty string must be rejected"
    );
    assert!(
        HttpMethod::from_str("get").is_ok(),
        "lowercase get must be accepted"
    );
}

// ── Skeleton Generation: Invalid Defaults ──

#[test]
fn rq_skeleton_parallel_node_still_gets_linear_depends_on() {
    let nodes = vec![
        SketchNode::new(NodeTemplateId::HttpHandler),
        SketchNode::new(NodeTemplateId::Parallel),
        SketchNode::new(NodeTemplateId::Timer),
    ];
    let skeleton = generate_skeleton(&nodes);
    let lines: Vec<&str> = skeleton.lines().collect();
    let parallel_block: Vec<&str> = lines
        .iter()
        .skip_while(|l| !l.contains("id: step-2"))
        .take_while(|l| !l.contains("id: step-3"))
        .copied()
        .collect();
    assert!(
        parallel_block.iter().any(|l| l.contains("depends_on: [step-1]")),
        "parallel node receives linear depends_on — this is a known invalid default: \
         parallel should branch, not chain"
    );
}

#[test]
fn rq_skeleton_condition_node_still_gets_linear_depends_on() {
    let nodes = vec![
        SketchNode::new(NodeTemplateId::Condition),
        SketchNode::new(NodeTemplateId::Run),
        SketchNode::new(NodeTemplateId::Timer),
    ];
    let skeleton = generate_skeleton(&nodes);
    let lines: Vec<&str> = skeleton.lines().collect();
    let run_block: Vec<&str> = lines
        .iter()
        .skip_while(|l| !l.contains("id: step-2"))
        .take_while(|l| !l.contains("id: step-3"))
        .copied()
        .collect();
    assert!(
        run_block.iter().any(|l| l.contains("depends_on: [step-1]")),
        "run after condition gets linear depends_on — known invalid default: \
         condition branches need separate true/false paths"
    );
}

#[test]
fn rq_skeleton_http_handler_config_populated_with_defaults() {
    let nodes = vec![SketchNode::new(NodeTemplateId::HttpHandler)];
    let skeleton = generate_skeleton(&nodes);
    assert!(
        !skeleton.contains("config: {}"),
        "http-handler config must not be empty"
    );
    assert!(
        skeleton.contains("port"),
        "http-handler config must include port default"
    );
    assert!(
        skeleton.contains("method"),
        "http-handler config must include method default"
    );
    assert!(
        skeleton.contains("path"),
        "http-handler config must include path default"
    );
}

#[test]
fn rq_skeleton_kafka_handler_config_populated_with_defaults() {
    let nodes = vec![SketchNode::new(NodeTemplateId::KafkaHandler)];
    let skeleton = generate_skeleton(&nodes);
    assert!(
        !skeleton.contains("config: {}"),
        "kafka-handler config must not be empty"
    );
    assert!(
        skeleton.contains("topic"),
        "kafka-handler config must include topic default"
    );
    assert!(
        skeleton.contains("group_id"),
        "kafka-handler config must include group_id default"
    );
    assert!(
        skeleton.contains("brokers"),
        "kafka-handler config must include brokers default"
    );
}

#[test]
fn rq_skeleton_cron_trigger_config_populated_with_defaults() {
    let nodes = vec![SketchNode::new(NodeTemplateId::CronTrigger)];
    let skeleton = generate_skeleton(&nodes);
    assert!(
        !skeleton.contains("config: {}"),
        "cron-trigger config must not be empty"
    );
    assert!(
        skeleton.contains("schedule"),
        "cron-trigger config must include schedule default"
    );
}

#[test]
fn rq_skeleton_timer_config_populated_with_defaults() {
    let nodes = vec![SketchNode::new(NodeTemplateId::Timer)];
    let skeleton = generate_skeleton(&nodes);
    assert!(
        !skeleton.contains("config: {}"),
        "timer config must not be empty"
    );
    assert!(
        skeleton.contains("duration_ms"),
        "timer config must include duration_ms default"
    );
}

#[test]
fn rq_skeleton_timeout_config_populated_with_defaults() {
    let nodes = vec![SketchNode::new(NodeTemplateId::Timeout)];
    let skeleton = generate_skeleton(&nodes);
    assert!(
        !skeleton.contains("config: {}"),
        "timeout config must not be empty"
    );
    assert!(
        skeleton.contains("duration_ms"),
        "timeout config must include duration_ms default"
    );
}

#[test]
fn rq_skeleton_workflow_name_is_always_hardcoded() {
    let nodes_a = vec![SketchNode::new(NodeTemplateId::HttpHandler)];
    let nodes_b = vec![
        SketchNode::new(NodeTemplateId::KafkaHandler),
        SketchNode::new(NodeTemplateId::Run),
    ];
    let skeleton_a = generate_skeleton(&nodes_a);
    let skeleton_b = generate_skeleton(&nodes_b);
    assert_eq!(
        skeleton_a.lines().next(),
        skeleton_b.lines().next(),
        "both skeletons have identical hardcoded name — \
         no way to distinguish workflows by name in skeleton output"
    );
}

#[test]
fn rq_skeleton_duplicate_node_types_produce_same_step_structure() {
    let nodes = vec![
        SketchNode::new(NodeTemplateId::Run),
        SketchNode::new(NodeTemplateId::Run),
        SketchNode::new(NodeTemplateId::Run),
    ];
    let skeleton = generate_skeleton(&nodes);
    assert_eq!(
        skeleton.matches("type: run").count(),
        3,
        "three identical run nodes produce identical step blocks — \
         no differentiation possible in skeleton output"
    );
    assert!(
        skeleton.contains("depends_on: [step-1]"),
        "step-2 linearly depends on step-1"
    );
    assert!(
        skeleton.contains("depends_on: [step-2]"),
        "step-3 linearly depends on step-2"
    );
    assert!(
        !skeleton.contains("depends_on: [step-1, step-2]"),
        "step-3 does NOT fan-in from both predecessors — \
         only linear chain deps are supported (known limitation)"
    );
}

#[test]
fn rq_skeleton_label_is_completely_ignored_in_output() {
    let mut node = SketchNode::new(NodeTemplateId::HttpHandler);
    node.label = "CRITICAL-PAYLOAD-INJECTION-HERE".to_string();
    let skeleton = generate_skeleton(&[node]);
    assert!(
        !skeleton.contains("CRITICAL-PAYLOAD-INJECTION-HERE"),
        "label field is never emitted — dead data in skeleton output (good for security, \
         bad for usability: user labels vanish without trace)"
    );
}

#[test]
fn rq_skeleton_no_timeout_guard_on_any_step() {
    let nodes = vec![
        SketchNode::new(NodeTemplateId::HttpHandler),
        SketchNode::new(NodeTemplateId::Run),
        SketchNode::new(NodeTemplateId::ServiceCall),
    ];
    let skeleton = generate_skeleton(&nodes);
    assert!(
        !skeleton.contains("timeout"),
        "no step has any timeout default — long-running steps have no \
         deadline protection in generated skeleton"
    );
}

#[test]
fn rq_skeleton_empty_label_sketch_node_still_generates_valid_skeleton() {
    let node = SketchNode {
        node_type: NodeTemplateId::Timer,
        label: String::new(),
    };
    let skeleton = generate_skeleton(&[node]);
    assert!(
        skeleton.contains("id: step-1"),
        "empty-label node still produces valid step"
    );
    assert!(
        skeleton.contains("type: timer"),
        "type is from enum, not label — must still be correct"
    );
    assert!(
        !skeleton.contains("depends_on"),
        "single node must not have depends_on regardless of label"
    );
}

#[test]
fn rq_skeleton_newline_in_label_does_not_corrupt_yaml_structure() {
    let mut node = SketchNode::new(NodeTemplateId::Run);
    node.label = "legit step\n  - id: injected-step\n    type: evil".to_string();
    let skeleton = generate_skeleton(&[node]);
    assert!(
        !skeleton.contains("injected-step"),
        "newline in label must not inject YAML keys"
    );
    assert!(
        !skeleton.contains("evil"),
        "label content must not appear in skeleton output"
    );
}

#[test]
fn rq_skeleton_single_parallel_produces_invalid_workflow_topology() {
    let nodes = vec![
        SketchNode::new(NodeTemplateId::Parallel),
    ];
    let skeleton = generate_skeleton(&nodes);
    assert!(
        skeleton.contains("type: parallel"),
        "parallel node emitted"
    );
    assert!(
        !skeleton.contains("depends_on"),
        "lone parallel has no deps — but parallel with one branch is \
         topologically invalid (needs at least 2 branches)"
    );
    assert!(
        skeleton.contains("config: {}"),
        "parallel config is empty — needs branch definitions"
    );
}

#[test]
fn rq_skeleton_timeout_without_guarded_step_is_semantically_empty() {
    let nodes = vec![
        SketchNode::new(NodeTemplateId::Timeout),
        SketchNode::new(NodeTemplateId::Run),
    ];
    let skeleton = generate_skeleton(&nodes);
    let lines: Vec<&str> = skeleton.lines().collect();
    let timeout_block: Vec<&str> = lines
        .iter()
        .skip_while(|l| !l.contains("id: step-1"))
        .take_while(|l| !l.contains("id: step-2"))
        .copied()
        .collect();
    assert!(
        !timeout_block.iter().any(|l| l.contains("depends_on")),
        "timeout is step-1 with no deps (correct)"
    );
    assert!(
        timeout_block.iter().any(|l| l.contains("config: {}")),
        "timeout config is empty — needs duration to be meaningful, \
         generated skeleton is semantically incomplete"
    );
}
