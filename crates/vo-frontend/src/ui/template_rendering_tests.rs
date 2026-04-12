//! TDD Red-phase tests for the Template Rendering Engine.
//!
//! Contract: docs/contracts/template-rendering-engine.md

#![cfg(test)]

use std::collections::HashSet;

use super::domain_types::{
    NodeTemplateId, RenderContext, SerializationReason, TemplateCategory, TemplateError,
    ValidationViolation,
};
use super::prototype_palette::{generate_skeleton, SketchNode};

#[cfg(test)]
use super::command_palette::filtered_templates;

fn node(node_type: NodeTemplateId) -> SketchNode {
    SketchNode {
        node_type,
        label: node_type.label().to_string(),
    }
}

#[test]
fn inv_001_all_returns_exactly_14_variants() {
    assert_eq!(NodeTemplateId::all().len(), 14);
}

#[test]
fn inv_002_all_as_str_values_are_unique() {
    let strs: Vec<&str> = NodeTemplateId::all().iter().map(|id| id.as_str()).collect();
    let unique: HashSet<&str> = strs.iter().copied().collect();
    assert_eq!(strs.len(), unique.len(), "as_str values must be unique");
}

#[test]
fn inv_003_from_str_roundtrips_for_all_variants() {
    for id in NodeTemplateId::all() {
        let s = id.as_str();
        let recovered = NodeTemplateId::parse(s)
            .unwrap_or_else(|| panic!("parse({s:?}) returned None for {id:?}"));
        assert_eq!(recovered, id, "parse(as_str({id:?})) != {id:?}");
    }
}

#[test]
fn inv_003_from_str_rejects_invalid_strings() {
    assert_eq!(NodeTemplateId::parse("nonexistent"), None);
    assert_eq!(NodeTemplateId::parse(""), None);
    assert_eq!(NodeTemplateId::parse("HTTP-HANDLER"), None);
}

#[test]
fn inv_004_all_labels_are_non_empty() {
    for id in NodeTemplateId::all() {
        assert!(
            !id.label().is_empty(),
            "label() for {id:?} must not be empty"
        );
    }
}

#[test]
fn inv_004_all_hints_are_non_empty() {
    for id in NodeTemplateId::all() {
        assert!(!id.hint().is_empty(), "hint() for {id:?} must not be empty");
    }
}

#[test]
fn inv_005_sketch_node_new_defaults_label_to_template_label() {
    for id in NodeTemplateId::all() {
        let sketch = SketchNode::new(id);
        assert_eq!(
            sketch.label,
            id.label(),
            "SketchNode::new({id:?}) label should default"
        );
        assert_eq!(sketch.node_type, id);
    }
}

#[test]
fn inv_007_first_node_has_no_depends_on() {
    let nodes = vec![node(NodeTemplateId::HttpHandler), node(NodeTemplateId::Run)];
    let skeleton = generate_skeleton(&nodes);
    let lines: Vec<&str> = skeleton.lines().collect();
    let step1_block: Vec<&str> = lines
        .iter()
        .skip_while(|l| !l.contains("id: step-1"))
        .take_while(|l| !l.contains("id: step-2"))
        .copied()
        .collect();
    assert!(
        !step1_block.iter().any(|l| l.contains("depends_on")),
        "first node must not have depends_on"
    );
}

#[test]
fn inv_006_skeleton_produces_sequential_step_ids() {
    let nodes: Vec<SketchNode> = NodeTemplateId::all().iter().map(|id| node(*id)).collect();
    let skeleton = generate_skeleton(&nodes);
    for i in 0..nodes.len() {
        let expected_id = format!("step-{}", i + 1);
        assert!(
            skeleton.contains(&expected_id),
            "skeleton must contain {expected_id}"
        );
    }
}

#[test]
fn inv_007_later_nodes_have_depends_on() {
    let nodes = vec![
        node(NodeTemplateId::HttpHandler),
        node(NodeTemplateId::Run),
        node(NodeTemplateId::Condition),
    ];
    let skeleton = generate_skeleton(&nodes);
    assert!(skeleton.contains("depends_on: [step-1]"));
    let lines: Vec<&str> = skeleton.lines().collect();
    let step3_block: Vec<&str> = lines
        .iter()
        .skip_while(|l| !l.contains("id: step-3"))
        .copied()
        .collect();
    assert!(
        step3_block
            .iter()
            .any(|l| l.contains("depends_on: [step-2]")),
        "step-3 must depend on step-2"
    );
}

#[test]
fn inv_007_single_node_has_no_depends_on() {
    let nodes = vec![node(NodeTemplateId::Timer)];
    let skeleton = generate_skeleton(&nodes);
    assert!(
        !skeleton.contains("depends_on"),
        "single node must not have depends_on"
    );
}

#[test]
fn inv_006_skeleton_empty_input_produces_header_only() {
    let skeleton = generate_skeleton(&[]);
    assert!(skeleton.contains("name: \"prototype-workflow\""));
    assert!(skeleton.contains("steps:"));
    assert!(!skeleton.contains("step-1"));
}

#[test]
fn inv_006_skeleton_config_is_always_empty_braces() {
    let nodes = vec![node(NodeTemplateId::HttpHandler), node(NodeTemplateId::Run)];
    let skeleton = generate_skeleton(&nodes);
    let config_count = skeleton.matches("config: {}").count();
    assert_eq!(config_count, 2, "each step must have config: {{}}");
}

#[test]
fn inv_008_all_template_types_can_create_sketch_nodes() {
    for id in NodeTemplateId::all() {
        let _sketch = SketchNode::new(id);
    }
}

#[test]
fn inv_009_filter_matches_label_case_insensitively() {
    let results = filtered_templates("HTTP");
    assert!(results
        .iter()
        .any(|t| t.node_type == NodeTemplateId::HttpHandler));
    let results_lower = filtered_templates("http");
    assert!(results_lower
        .iter()
        .any(|t| t.node_type == NodeTemplateId::HttpHandler));
}

#[test]
fn inv_009_filter_matches_hint_case_insensitively() {
    let results = filtered_templates("DURABLY");
    assert!(results.iter().any(|t| t.node_type == NodeTemplateId::Timer));
}

#[test]
fn inv_009_filter_matches_as_str_case_insensitively() {
    let results = filtered_templates("KAFKA-HANDLER");
    assert!(results
        .iter()
        .any(|t| t.node_type == NodeTemplateId::KafkaHandler));
    let results_lower = filtered_templates("kafka-handler");
    assert!(results_lower
        .iter()
        .any(|t| t.node_type == NodeTemplateId::KafkaHandler));
}

#[test]
fn inv_010_empty_query_returns_all_templates() {
    let results = filtered_templates("");
    assert_eq!(results.len(), 14);
}

#[test]
fn inv_010_whitespace_only_query_returns_all_templates() {
    let results = filtered_templates("   ");
    assert_eq!(results.len(), 14);
}

#[test]
fn inv_010_non_matching_query_returns_empty() {
    let results = filtered_templates("zz-no-match-zz");
    assert!(results.is_empty());
}

#[test]
fn command_template_exposes_label_and_hint_from_node_type() {
    use super::command_palette::CommandTemplate;
    for id in NodeTemplateId::all() {
        let cmd = CommandTemplate::from(id);
        assert_eq!(
            cmd.label,
            id.label(),
            "CommandTemplate label must match template"
        );
        assert_eq!(
            cmd.hint,
            id.hint(),
            "CommandTemplate hint must match template"
        );
    }
}

#[test]
fn descriptor_fields_match_node_template_for_all_variants() {
    for id in NodeTemplateId::all() {
        let desc = id.descriptor();
        assert_eq!(desc.id, id);
        assert_eq!(desc.as_str, id.as_str());
        assert_eq!(desc.label, id.label());
        assert_eq!(desc.hint, id.hint());
    }
}

#[test]
fn descriptor_as_str_is_url_safe() {
    for id in NodeTemplateId::all() {
        let desc = id.descriptor();
        assert!(
            desc.as_str
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-'),
            "descriptor as_str must be URL-safe: {:?}",
            desc.as_str
        );
    }
}

#[test]
fn category_ingress_contains_http_kafka_cron_only() {
    let members = TemplateCategory::Ingress.members();
    assert!(members.contains(&NodeTemplateId::HttpHandler));
    assert!(members.contains(&NodeTemplateId::KafkaHandler));
    assert!(members.contains(&NodeTemplateId::CronTrigger));
    assert_eq!(members.len(), 3);
}

#[test]
fn category_execution_contains_run_service_object_send_only() {
    let members = TemplateCategory::Execution.members();
    assert!(members.contains(&NodeTemplateId::Run));
    assert!(members.contains(&NodeTemplateId::ServiceCall));
    assert!(members.contains(&NodeTemplateId::ObjectCall));
    assert!(members.contains(&NodeTemplateId::SendMessage));
    assert_eq!(members.len(), 4);
}

#[test]
fn category_state_contains_get_and_set_state_only() {
    let members = TemplateCategory::State.members();
    assert!(members.contains(&NodeTemplateId::GetState));
    assert!(members.contains(&NodeTemplateId::SetState));
    assert_eq!(members.len(), 2);
}

#[test]
fn category_control_contains_condition_parallel_timer_timeout_only() {
    let members = TemplateCategory::Control.members();
    assert!(members.contains(&NodeTemplateId::Condition));
    assert!(members.contains(&NodeTemplateId::Parallel));
    assert!(members.contains(&NodeTemplateId::Timer));
    assert!(members.contains(&NodeTemplateId::Timeout));
    assert_eq!(members.len(), 4);
}

#[test]
fn category_workflow_contains_workflow_submit_only() {
    let members = TemplateCategory::Workflow.members();
    assert!(members.contains(&NodeTemplateId::WorkflowSubmit));
    assert_eq!(members.len(), 1);
}

#[test]
fn all_categories_union_covers_exactly_14_templates_no_overlap() {
    let mut seen = HashSet::new();
    for cat in TemplateCategory::all() {
        for id in cat.members() {
            assert!(seen.insert(id), "{id:?} appears in multiple categories");
        }
    }
    assert_eq!(seen.len(), 14);
}

#[test]
fn category_for_returns_correct_category_for_each_variant() {
    assert_eq!(
        NodeTemplateId::HttpHandler.category(),
        TemplateCategory::Ingress
    );
    assert_eq!(NodeTemplateId::Run.category(), TemplateCategory::Execution);
    assert_eq!(NodeTemplateId::GetState.category(), TemplateCategory::State);
    assert_eq!(
        NodeTemplateId::Condition.category(),
        TemplateCategory::Control
    );
    assert_eq!(
        NodeTemplateId::WorkflowSubmit.category(),
        TemplateCategory::Workflow
    );
}

#[test]
fn parse_error_displays_input_and_expected() {
    let err = TemplateError::ParseError {
        input: "bad-input".to_string(),
        expected: "valid template id",
    };
    let msg = format!("{err}");
    assert!(msg.contains("bad-input"));
    assert!(msg.contains("valid template id"));
}

#[test]
fn validation_error_missing_required_field_displays_details() {
    let err = TemplateError::ValidationError {
        template_id: NodeTemplateId::HttpHandler,
        violation: ValidationViolation::MissingRequiredField("port".to_string()),
    };
    let msg = format!("{err}");
    assert!(msg.contains("http-handler"));
    assert!(msg.contains("port"));
}

#[test]
fn validation_error_circular_dependency_is_displayed() {
    let err = TemplateError::ValidationError {
        template_id: NodeTemplateId::Condition,
        violation: ValidationViolation::CircularDependency,
    };
    let msg = format!("{err}");
    assert!(msg.contains("condition"));
    assert!(msg.contains("circular") || msg.contains("Circular"));
}

#[test]
fn validation_error_invalid_combination_lists_templates() {
    let err = TemplateError::ValidationError {
        template_id: NodeTemplateId::Parallel,
        violation: ValidationViolation::InvalidTemplateCombination(vec![
            NodeTemplateId::Timer,
            NodeTemplateId::Timeout,
        ]),
    };
    let msg = format!("{err}");
    assert!(msg.contains("parallel"));
}

#[test]
fn render_error_displays_context() {
    let err = TemplateError::RenderError {
        template_id: NodeTemplateId::Run,
        context: RenderContext::Palette,
    };
    let msg = format!("{err}");
    assert!(msg.contains("run"));
    assert!(msg.contains("Palette") || msg.contains("palette"));
}

#[test]
fn render_error_all_contexts_are_displayable() {
    for context in [
        RenderContext::Palette,
        RenderContext::CommandPalette,
        RenderContext::Canvas,
        RenderContext::Inspector,
    ] {
        let err = TemplateError::RenderError {
            template_id: NodeTemplateId::HttpHandler,
            context,
        };
        let msg = format!("{err}");
        assert!(
            !msg.is_empty(),
            "RenderError with {context:?} must produce non-empty display"
        );
    }
}

#[test]
fn serialization_error_empty_sketch_is_displayed() {
    let err = TemplateError::SerializationError {
        reason: SerializationReason::EmptySketch,
    };
    let msg = format!("{err}");
    assert!(
        msg.contains("sketch")
            || msg.contains("Sketch")
            || msg.contains("empty")
            || msg.contains("Empty")
    );
}

#[test]
fn serialization_error_yaml_encode_is_displayed() {
    let err = TemplateError::SerializationError {
        reason: SerializationReason::YamlEncodeError("test failure".to_string()),
    };
    let msg = format!("{err}");
    assert!(msg.contains("test failure") || msg.contains("yaml") || msg.contains("Yaml"));
}

#[test]
fn serialization_error_json_encode_is_displayed() {
    let err = TemplateError::SerializationError {
        reason: SerializationReason::JsonEncodeError("test failure".to_string()),
    };
    let msg = format!("{err}");
    assert!(msg.contains("test failure") || msg.contains("json") || msg.contains("Json"));
}

#[test]
fn skeleton_output_contains_correct_type_for_each_node() {
    let nodes = vec![
        node(NodeTemplateId::HttpHandler),
        node(NodeTemplateId::KafkaHandler),
        node(NodeTemplateId::CronTrigger),
    ];
    let skeleton = generate_skeleton(&nodes);
    assert!(skeleton.contains("type: http-handler"));
    assert!(skeleton.contains("type: kafka-handler"));
    assert!(skeleton.contains("type: cron-trigger"));
}

#[test]
fn skeleton_with_all_14_templates_produces_valid_output() {
    let nodes: Vec<SketchNode> = NodeTemplateId::all().iter().map(|id| node(*id)).collect();
    let skeleton = generate_skeleton(&nodes);
    assert!(skeleton.contains("name: \"prototype-workflow\""));
    assert!(skeleton.contains("steps:"));
    for i in 1..=14 {
        assert!(skeleton.contains(&format!("id: step-{i}")));
    }
    let lines: Vec<&str> = skeleton.lines().collect();
    let step1: Vec<&str> = lines
        .iter()
        .skip_while(|l| !l.contains("id: step-1"))
        .take_while(|l| !l.contains("id: step-2"))
        .copied()
        .collect();
    assert!(!step1.iter().any(|l| l.contains("depends_on")));
    let step14: Vec<&str> = lines
        .iter()
        .skip_while(|l| !l.contains("id: step-14"))
        .copied()
        .collect();
    assert!(step14.iter().any(|l| l.contains("depends_on: [step-13]")));
}

#[test]
fn filter_multi_word_query_matches_multiple_fields() {
    let results = filtered_templates("handler");
    assert!(results
        .iter()
        .any(|t| t.node_type == NodeTemplateId::HttpHandler));
    assert!(results
        .iter()
        .any(|t| t.node_type == NodeTemplateId::KafkaHandler));
}

#[test]
fn filter_query_trimmed_before_matching() {
    let results = filtered_templates("  timer  ");
    assert!(results.iter().any(|t| t.node_type == NodeTemplateId::Timer));
}
