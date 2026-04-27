//! RED-QUEEN coevolutionary adversarial tests for vo-frontend.
//!
//! Attacks coevolve with defensive code. Each test targets an edge case where
//! a prior fix may regress: state corruption via config merges, Workflow name
//! abuse, badge class invariants, parse boundary attacks, and skeleton
//! generation invalid defaults.

use std::str::FromStr;
use vo_frontend::ui::domain_types::{HttpMethod, NodeTemplateId};
use vo_frontend::ui::graph::{node_kind_to_category, Node, NodeId, Workflow};
use vo_types::GuaranteeClass;
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
    let wf = Workflow::new("before\0after".into(), GuaranteeClass::BestEffort);
    let json = serde_json::to_string(&wf).unwrap();
    let recovered: Workflow = serde_json::from_str(&json).unwrap();
    assert!(
        recovered.name.contains('\0'),
        "null byte survives roundtrip — documented"
    );
}

#[test]
fn workflow_name_with_emoji_does_not_panic() {
    let mut wf = Workflow::new("🦀🔥💣".into(), GuaranteeClass::BestEffort);
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
fn rq_skeleton_http_handler_config_is_empty_not_populated() {
    let nodes = vec![SketchNode::new(NodeTemplateId::HttpHandler)];
    let skeleton = generate_skeleton(&nodes);
    assert!(
        skeleton.contains("config: {}"),
        "http-handler config is empty — a real handler needs port/method/path, \
         but skeleton emits a misleading empty default"
    );
}

#[test]
fn rq_skeleton_kafka_handler_config_is_empty_not_populated() {
    let nodes = vec![SketchNode::new(NodeTemplateId::KafkaHandler)];
    let skeleton = generate_skeleton(&nodes);
    assert!(
        skeleton.contains("config: {}"),
        "kafka-handler config is empty — needs topic/group/brokers, \
         skeleton emits misleading empty default"
    );
}

#[test]
fn rq_skeleton_cron_trigger_config_is_empty_not_populated() {
    let nodes = vec![SketchNode::new(NodeTemplateId::CronTrigger)];
    let skeleton = generate_skeleton(&nodes);
    assert!(
        skeleton.contains("config: {}"),
        "cron-trigger config is empty — needs schedule/cron expression, \
         skeleton emits misleading empty default"
    );
}

#[test]
fn rq_skeleton_timer_config_is_empty_not_populated() {
    let nodes = vec![SketchNode::new(NodeTemplateId::Timer)];
    let skeleton = generate_skeleton(&nodes);
    assert!(
        skeleton.contains("config: {}"),
        "timer config is empty — needs duration, skeleton emits misleading empty default"
    );
}

#[test]
fn rq_skeleton_timeout_config_is_empty_not_populated() {
    let nodes = vec![SketchNode::new(NodeTemplateId::Timeout)];
    let skeleton = generate_skeleton(&nodes);
    assert!(
        skeleton.contains("config: {}"),
        "timeout config is empty — needs duration/deadline, \
         skeleton emits misleading empty default"
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

// ── Deep Nesting: Skeleton Generation ──

#[test]
fn deep_nesting_skeleton_100_nodes_produces_valid_linear_chain() {
    let nodes: Vec<SketchNode> = (0..100)
        .map(|_| SketchNode::new(NodeTemplateId::Run))
        .collect();
    let skeleton = generate_skeleton(&nodes);
    for i in 1..=100 {
        let step_id = format!("step-{i}");
        assert!(
            skeleton.contains(&step_id),
            "skeleton with 100 nodes must contain {step_id}"
        );
    }
    assert!(
        skeleton.contains("depends_on: [step-99]"),
        "last step must depend on step-99"
    );
    assert!(
        !skeleton.contains("step-101"),
        "must not produce step beyond input count"
    );
}

#[test]
fn deep_nesting_skeleton_100_nodes_first_step_has_no_depends() {
    let nodes: Vec<SketchNode> = (0..100)
        .map(|_| SketchNode::new(NodeTemplateId::Run))
        .collect();
    let skeleton = generate_skeleton(&nodes);
    let lines: Vec<&str> = skeleton.lines().collect();
    let step1: Vec<&str> = lines
        .iter()
        .skip_while(|l| !l.contains("id: step-1"))
        .take_while(|l| !l.contains("id: step-2"))
        .copied()
        .collect();
    assert!(
        !step1.iter().any(|l| l.contains("depends_on")),
        "step-1 in 100-node chain must have no depends_on"
    );
}

#[test]
fn deep_nesting_skeleton_100_nodes_every_non_first_has_depends() {
    let nodes: Vec<SketchNode> = (0..100)
        .map(|_| SketchNode::new(NodeTemplateId::HttpHandler))
        .collect();
    let skeleton = generate_skeleton(&nodes);
    let lines: Vec<&str> = skeleton.lines().collect();
    for i in 2..=100 {
        let current = format!("id: step-{i}");
        let next = if i < 100 {
            format!("id: step-{}", i + 1)
        } else {
            "END_MARKER".to_string()
        };
        let block: Vec<&str> = lines
            .iter()
            .skip_while(|l| !l.contains(&current))
            .take_while(|l| !l.contains(&next))
            .copied()
            .collect();
        let prev = format!("depends_on: [step-{}]", i - 1);
        assert!(
            block.iter().any(|l| l.contains(&prev)),
            "step-{i} must depend on step-{} in 100-node chain",
            i - 1
        );
    }
}

#[test]
fn deep_nesting_skeleton_1000_nodes_step_ids_remain_sequential() {
    let nodes: Vec<SketchNode> = (0..1000)
        .map(|_| SketchNode::new(NodeTemplateId::Timer))
        .collect();
    let skeleton = generate_skeleton(&nodes);
    assert!(
        skeleton.contains("step-1"),
        "1000-node skeleton must start at step-1"
    );
    assert!(
        skeleton.contains("step-1000"),
        "1000-node skeleton must reach step-1000"
    );
    assert!(
        !skeleton.contains("step-1001"),
        "1000-node skeleton must not produce step-1001"
    );
    assert!(
        skeleton.contains("depends_on: [step-999]"),
        "1000th step must depend on 999th"
    );
}

#[test]
fn deep_nesting_skeleton_alternating_control_flow_nodes() {
    let control_nodes = [
        NodeTemplateId::Condition,
        NodeTemplateId::Parallel,
        NodeTemplateId::Timeout,
        NodeTemplateId::Timer,
    ];
    let nodes: Vec<SketchNode> = (0..50)
        .flat_map(|_| control_nodes.iter().map(|t| SketchNode::new(*t)))
        .collect();
    let skeleton = generate_skeleton(&nodes);
    assert_eq!(
        skeleton.matches("type: condition").count(),
        50,
        "50 condition nodes in alternating pattern"
    );
    assert_eq!(
        skeleton.matches("type: parallel").count(),
        50,
        "50 parallel nodes in alternating pattern"
    );
    assert_eq!(
        skeleton.matches("type: timeout").count(),
        50,
        "50 timeout nodes in alternating pattern"
    );
    assert_eq!(
        skeleton.matches("type: timer").count(),
        50,
        "50 timer nodes in alternating pattern"
    );
    assert!(
        skeleton.contains("depends_on: [step-199]"),
        "200-node alternating chain: last depends on second-to-last"
    );
    assert!(
        !skeleton.contains("step-201"),
        "200-node chain must not produce step-201"
    );
}

#[test]
fn deep_nesting_skeleton_deeply_nested_condition_branches_still_linear() {
    let nodes: Vec<SketchNode> = (0..20)
        .flat_map(|_| {
            vec![
                SketchNode::new(NodeTemplateId::Condition),
                SketchNode::new(NodeTemplateId::Run),
            ]
        })
        .collect();
    let skeleton = generate_skeleton(&nodes);
    assert_eq!(
        skeleton.matches("type: condition").count(),
        20,
        "20 condition nodes"
    );
    assert_eq!(
        skeleton.matches("type: run").count(),
        20,
        "20 run nodes after conditions"
    );
    let lines: Vec<&str> = skeleton.lines().collect();
    for i in 1..=40 {
        let step_id = format!("id: step-{i}");
        assert!(
            lines.iter().any(|l| l.contains(&step_id)),
            "40-node condition/run chain must contain step-{i}"
        );
    }
    assert!(
        skeleton.contains("depends_on: [step-39]"),
        "step-40 depends on step-39 — all linear, no branch structure"
    );
}

#[test]
fn deep_nesting_skeleton_all_14_types_repeated_10_times() {
    let nodes: Vec<SketchNode> = NodeTemplateId::all()
        .iter()
        .flat_map(|id| (0..10).map(move |_| SketchNode::new(*id)))
        .collect();
    assert_eq!(nodes.len(), 140);
    let skeleton = generate_skeleton(&nodes);
    for id in NodeTemplateId::all() {
        let type_str = format!("type: {}", id);
        assert_eq!(
            skeleton.matches(&type_str).count(),
            10,
            "each of 14 template types appears 10 times in 140-node skeleton"
        );
    }
    assert!(
        skeleton.contains("depends_on: [step-139]"),
        "140-node skeleton: step-140 depends on step-139"
    );
}

#[test]
fn deep_nesting_skeleton_label_with_yaml_nested_block_injection() {
    let mut node = SketchNode::new(NodeTemplateId::Run);
    node.label = "legit\nsteps:\n  - id: injected\n    type: evil\n    depends_on: [step-1]\n    config:\n      nested:\n        deeply:\n          value: pwned".to_string();
    let skeleton = generate_skeleton(&[node]);
    assert!(
        !skeleton.contains("injected"),
        "YAML nested block injection via label must not appear"
    );
    assert!(
        !skeleton.contains("pwned"),
        "deeply nested YAML injection must be blocked"
    );
    assert!(
        !skeleton.contains("evil"),
        "type injection via label must be blocked"
    );
    assert!(
        skeleton.lines().filter(|l| l.contains("steps:")).count() == 1,
        "only the header 'steps:' line must exist — no injected sub-lists"
    );
}

#[test]
fn deep_nesting_skeleton_label_with_deeply_indented_yaml_injection() {
    let mut node = SketchNode::new(NodeTemplateId::Timer);
    node.label = "ok\n    config:\n        deeply_nested:\n            injection:\n                - item1\n                - item2".to_string();
    let skeleton = generate_skeleton(&[node]);
    assert!(
        !skeleton.contains("deeply_nested"),
        "deeply indented YAML injection must not appear"
    );
    assert!(
        !skeleton.contains("injection"),
        "nested list injection must not appear"
    );
    assert!(
        !skeleton.contains("item1"),
        "injected list items must not appear"
    );
}

#[test]
fn deep_nesting_skeleton_label_with_yaml_anchor_and_alias() {
    let mut node = SketchNode::new(NodeTemplateId::HttpHandler);
    node.label = "normal\nanchored: &ref\n  key: value\naliased: *ref".to_string();
    let skeleton = generate_skeleton(&[node]);
    assert!(
        !skeleton.contains("&ref"),
        "YAML anchor injection via label must not appear"
    );
    assert!(
        !skeleton.contains("*ref"),
        "YAML alias injection via label must not appear"
    );
    assert!(
        !skeleton.contains("anchored"),
        "anchor key must not appear in output"
    );
}

#[test]
fn deep_nesting_skeleton_label_with_multiline_scalars() {
    let mut node = SketchNode::new(NodeTemplateId::ServiceCall);
    node.label = "text\n|\n  line1\n  line2\n  line3\n...\n---\n%YAML 1.2\n---".to_string();
    let skeleton = generate_skeleton(&[node]);
    assert!(
        !skeleton.contains("---"),
        "YAML document separator injection must not appear"
    );
    assert!(
        !skeleton.contains("%YAML"),
        "YAML directive injection must not appear"
    );
    assert!(
        !skeleton.contains("..."),
        "YAML end marker injection must not appear"
    );
}

#[test]
fn deep_nesting_skeleton_label_with_null_bytes_and_control_chars() {
    let mut node = SketchNode::new(NodeTemplateId::GetState);
    node.label = "legit\x00\x01\x02\x1b[31mevil\x1b[0m".to_string();
    let skeleton = generate_skeleton(&[node]);
    assert!(
        skeleton.contains("id: step-1"),
        "step-1 must exist even with control chars in label"
    );
    assert!(
        skeleton.contains("type: get-state"),
        "type is from enum, must be correct regardless of label"
    );
    let lines: Vec<&str> = skeleton.lines().collect();
    assert_eq!(
        lines.len(),
        5,
        "skeleton must be exactly 5 lines: header, steps, step-1 id, step-1 type, step-1 config"
    );
}

#[test]
fn deep_nesting_skeleton_mixed_realistic_large_workflow() {
    let nodes: Vec<SketchNode> = vec![
        SketchNode::new(NodeTemplateId::HttpHandler),
        SketchNode::new(NodeTemplateId::Run),
        SketchNode::new(NodeTemplateId::Condition),
        SketchNode::new(NodeTemplateId::Run),
        SketchNode::new(NodeTemplateId::Run),
        SketchNode::new(NodeTemplateId::Parallel),
        SketchNode::new(NodeTemplateId::ServiceCall),
        SketchNode::new(NodeTemplateId::Timer),
        SketchNode::new(NodeTemplateId::Run),
        SketchNode::new(NodeTemplateId::GetState),
        SketchNode::new(NodeTemplateId::SetState),
        SketchNode::new(NodeTemplateId::ObjectCall),
        SketchNode::new(NodeTemplateId::SendMessage),
        SketchNode::new(NodeTemplateId::Timeout),
        SketchNode::new(NodeTemplateId::Run),
        SketchNode::new(NodeTemplateId::WorkflowSubmit),
        SketchNode::new(NodeTemplateId::KafkaHandler),
        SketchNode::new(NodeTemplateId::CronTrigger),
        SketchNode::new(NodeTemplateId::Condition),
        SketchNode::new(NodeTemplateId::Parallel),
    ];
    let skeleton = generate_skeleton(&nodes);
    assert!(
        skeleton.contains("depends_on: [step-19]"),
        "step-20 in realistic workflow depends on step-19"
    );
    assert_eq!(
        skeleton.lines().filter(|l| l.starts_with("  - id:")).count(),
        20,
        "exactly 20 step id lines"
    );
    assert_eq!(
        skeleton.matches("config: {}").count(),
        20,
        "exactly 20 config entries"
    );
    assert_eq!(
        skeleton.lines().filter(|l| l.contains("depends_on")).count(),
        19,
        "exactly 19 depends_on entries (step-1 has none)"
    );
}

#[test]
fn deep_nesting_skeleton_parallel_in_middle_of_long_chain() {
    let mut nodes: Vec<SketchNode> = (0..50)
        .map(|_| SketchNode::new(NodeTemplateId::Run))
        .collect();
    nodes.insert(25, SketchNode::new(NodeTemplateId::Parallel));
    let skeleton = generate_skeleton(&nodes);
    assert_eq!(
        skeleton.matches("type: run").count(),
        50,
        "50 run nodes still present after parallel insertion"
    );
    assert_eq!(
        skeleton.matches("type: parallel").count(),
        1,
        "exactly 1 parallel node"
    );
    assert!(
        skeleton.contains("depends_on: [step-50]"),
        "step-51 (last) depends on step-50"
    );
    assert!(
        !skeleton.contains("step-52"),
        "51-node skeleton must not produce step-52"
    );
}

#[test]
fn deep_nesting_skeleton_config_empty_braces_on_all_steps_regardless_of_size() {
    let nodes: Vec<SketchNode> = (0..100)
        .map(|_| SketchNode::new(NodeTemplateId::Condition))
        .collect();
    let skeleton = generate_skeleton(&nodes);
    let config_count = skeleton.matches("config: {}").count();
    assert_eq!(
        config_count, 100,
        "100 condition nodes must all have config: {{}}"
    );
    let lines: Vec<&str> = skeleton.lines().collect();
    assert!(
        !lines.iter().any(|l| l.contains("config:") && !l.contains("config: {}")),
        "no config key must have non-empty value in 100-node skeleton"
    );
}

#[test]
fn deep_nesting_skeleton_no_config_field_on_first_node() {
    let nodes: Vec<SketchNode> = (0..50)
        .map(|_| SketchNode::new(NodeTemplateId::HttpHandler))
        .collect();
    let skeleton = generate_skeleton(&nodes);
    let lines: Vec<&str> = skeleton.lines().collect();
    let step1_block: Vec<&str> = lines
        .iter()
        .skip_while(|l| !l.contains("id: step-1"))
        .take_while(|l| !l.contains("id: step-2"))
        .copied()
        .collect();
    assert!(
        step1_block.iter().any(|l| l.contains("config: {}")),
        "step-1 must still have config: {{}} even though no depends_on"
    );
}

#[test]
fn deep_nesting_skeleton_type_field_always_correct_for_large_chain() {
    let all_types = NodeTemplateId::all();
    let nodes: Vec<SketchNode> = (0..100)
        .map(|i| SketchNode::new(all_types[i % 14]))
        .collect();
    let skeleton = generate_skeleton(&nodes);
    for (i, node) in nodes.iter().enumerate() {
        let step_id = format!("id: step-{}", i + 1);
        let type_str = format!("type: {}", node.node_type);
        let lines: Vec<&str> = skeleton.lines().collect();
        let block: Vec<&str> = lines
            .iter()
            .skip_while(|l| !l.contains(&step_id))
            .take(5)
            .copied()
            .collect();
        assert!(
            block.iter().any(|l| l.contains(&type_str)),
            "step-{} must have correct type {}",
            i + 1,
            node.node_type
        );
    }
}
