//! BLACK-HAT adversarial XSS injection tests.
//!
//! Attack vectors: script injection in props, CSS injection, event handler abuse,
//! SVG-based XSS, and HTML entity smuggling through node names and descriptions.

use std::str::FromStr;
use vo_frontend::ui::domain_types::HttpMethod;
use vo_frontend::ui::graph::{Node, NodeCategory, NodeId, Workflow};
use vo_types::NodeKind;

fn mk_node(name: &str, desc: &str) -> Node {
    let mut node = Node::new(NodeId::new(), name.into(), NodeKind::Pure);
    node.description = desc.into();
    node.category = NodeCategory::Flow;
    node
}

#[test]
fn xss_payloads_do_not_panic_node_creation() {
    let payloads = [
        "<script>alert('xss')</script>",
        "<img src=x onerror=alert(1)>",
        "javascript:alert(1)",
        "<svg onload=alert(1)>",
        "<body onload=alert(1)>",
        "';alert(String.fromCharCode(88,83,83))//",
        "\"><script>alert(document.cookie)</script>",
        "<iframe src=\"javascript:alert(1)\">",
        "<details open ontoggle=alert(1)>",
        "<marquee onstart=alert(1)>",
        "<input onfocus=alert(1) autofocus>",
        "<a href=\"javascript:void(0)\" onclick=alert(1)>",
        "<div onmouseover=alert(1)>hover me</div>",
        "<math><mtext><table><mglyph><style><!--</style><img title=\"--><img src=1 onerror=alert(1)>\">",
        "expression(alert(1))",
        "url(javascript:alert(1))",
        "@import url(evil.css)",
        "background:url(javascript:alert(1))",
        "<style>body{background:url(javascript:alert(1))}</style>",
    ];
    for payload in &payloads {
        let _node = mk_node(payload, payload);
    }
}

#[test]
fn xss_in_node_name_survives_round_trip() {
    let node = mk_node("<script>alert('xss')</script>", "safe");
    assert_eq!(&node.name, "<script>alert('xss')</script>");
}

#[test]
fn xss_in_node_description_survives_round_trip() {
    let node = mk_node("safe", "<img src=x onerror=alert(1)>");
    assert_eq!(&node.description, "<img src=x onerror=alert(1)>");
}

#[test]
fn svg_onload_in_name_does_not_panic() {
    let node = mk_node("<svg onload=alert(1)>", "normal");
    assert!(node.name.contains("svg"));
}

#[test]
fn javascript_uri_in_name_survives_round_trip() {
    let node = mk_node("javascript:alert(1)", "desc");
    assert_eq!(&node.name, "javascript:alert(1)");
}

#[test]
fn html_entity_smuggling_in_description() {
    let node = mk_node("safe", "&lt;script&gt;alert(1)&lt;/script&gt;");
    assert_eq!(&node.description, "&lt;script&gt;alert(1)&lt;/script&gt;");
}

#[test]
fn null_byte_injection_in_name() {
    let node = mk_node("node\x00<script>alert(1)</script>", "safe");
    assert!(node.name.contains("node"));
}

#[test]
fn css_expression_injection_in_description() {
    let node = mk_node("safe", "expression(alert(1))");
    assert_eq!(&node.description, "expression(alert(1))");
}

#[test]
fn css_import_injection_in_name() {
    let node = mk_node("@import url(evil.css)", "safe");
    assert_eq!(&node.name, "@import url(evil.css)");
}

#[test]
fn css_url_javascript_injection_in_description() {
    let node = mk_node("safe", "url(javascript:alert(1))");
    assert_eq!(&node.description, "url(javascript:alert(1))");
}

#[test]
fn onerror_handler_variants_do_not_panic() {
    for p in &["<img src=x onerror=alert(1)>", "<img/src=x/onerror=alert(1)>"] {
        let _ = mk_node(p, "safe");
    }
}

#[test]
fn onfocus_autofocus_in_description() {
    let node = mk_node("safe", "<input onfocus=alert(1) autofocus>");
    assert!(node.description.contains("onfocus"));
}

#[test]
fn ontoggle_event_in_name() {
    let node = mk_node("<details open ontoggle=alert(1)>", "safe");
    assert!(node.name.contains("ontoggle"));
}

#[test]
fn workflow_with_xss_nodes_does_not_panic() {
    let mut wf = Workflow::new("xss-test".into());
    for p in &["<script>x</script>", "<img src=x onerror=alert(1)>", "<svg onload=alert(1)>"] {
        wf.add_node(mk_node(p, p));
    }
    assert_eq!(wf.nodes_by_id().len(), 3);
}

#[test]
fn workflow_lookup_by_xss_name_returns_correct_node() {
    let mut wf = Workflow::new("lookup-test".into());
    let node = mk_node("<script>evil</script>", "desc");
    wf.add_node(node.clone());
    let by_id = wf.nodes_by_id();
    let found = by_id.get(&node.id.0);
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, "<script>evil</script>");
}

#[test]
fn http_method_from_str_rejects_xss_payloads() {
    for m in &["GET<script>alert(1)</script>", "<script>x</script>", "POST\n<script>"] {
        assert!(HttpMethod::from_str(m).is_err(), "accepted XSS method: {m}");
    }
}

#[test]
fn http_method_from_str_ignore_case_falls_back_on_xss() {
    let result = HttpMethod::from_str_ignore_case("<script>alert(1)</script>");
    assert_eq!(result, HttpMethod::Post, "should safely fallback to default");
}
