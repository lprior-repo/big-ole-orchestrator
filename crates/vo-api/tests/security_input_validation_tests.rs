use rstest::rstest;
use vo_api::types::names::*;

#[rstest]
#[case("'; DROP TABLE users; --")]
#[case("\" OR \"1\"=\"1")]
#[case("1; SELECT * FROM secrets")]
#[case("admin'--")]
#[case("<script>alert('xss')</script>")]
#[case("${env.MALICIOUS_VAR}")]
#[case("{{.MaliciousTemplate}}")]
#[case("%00nullbyte")]
fn workflow_name_rejects_sql_injection_patterns(#[case] name: &str) {
    let result = WorkflowName::new(name);
    assert!(
        result.is_err(),
        "WorkflowName should reject SQL injection pattern: {name}"
    );
}

#[rstest]
#[case("'; DROP TABLE users; --")]
#[case("\" OR \"1\"=\"1")]
#[case("1; SELECT * FROM secrets")]
#[case("<script>alert('xss')</script>")]
#[case("${env.MALICIOUS_VAR}")]
#[case("{{.MaliciousTemplate}}")]
#[case("%00nullbyte")]
#[case("name'); WAITFOR DELAY '00:00:05'--")]
fn signal_name_rejects_injection_patterns(#[case] name: &str) {
    let result = SignalName::new(name);
    assert!(
        result.is_err(),
        "SignalName should reject injection pattern: {name}"
    );
}

#[rstest]
#[case("workflow\x00name")]
#[case("workflow\u{200B}name")]
#[case("workflow\u{202E}name")]
#[case("workflow\u{0001}")]
fn workflow_name_rejects_unicode_attack_patterns(#[case] name: &str) {
    let result = WorkflowName::new(name);
    assert!(
        result.is_err(),
        "WorkflowName should reject unicode attack: {name}"
    );
}

#[rstest]
#[case("signal\x00name")]
#[case("signal\u{200B}name")]
#[case("signal\u{202E}name")]
#[case("signal\u{0001}")]
fn signal_name_rejects_unicode_attack_patterns(#[case] name: &str) {
    let result = SignalName::new(name);
    assert!(
        result.is_err(),
        "SignalName should reject unicode attack: {name}"
    );
}

#[rstest]
#[case("workflow\x00name")]
#[case("workflow\nname")]
#[case("workflow\rname")]
#[case("workflow\tname")]
fn invocation_id_rejects_control_characters(#[case] id: &str) {
    use vo_api::types::v1::InvocationId;
    let result = InvocationId::from_str(id);
    assert!(
        result.is_err(),
        "InvocationId should reject control chars: {id:?}"
    );
}

#[test]
fn workflow_name_accepts_underscore_and_numbers() {
    assert!(WorkflowName::new("test_123").is_ok());
    assert!(WorkflowName::new("_private").is_ok());
    assert!(WorkflowName::new("a1").is_ok());
}

#[test]
fn workflow_name_rejects_leading_underscore() {
    assert!(WorkflowName::new("_leading").is_err());
}

#[test]
fn workflow_name_rejects_uppercase() {
    assert!(WorkflowName::new("Test").is_err());
    assert!(WorkflowName::new("TEST").is_err());
    assert!(WorkflowName::new("testName").is_err());
}

#[test]
fn workflow_name_rejects_dash_and_dot() {
    assert!(WorkflowName::new("test-name").is_err());
    assert!(WorkflowName::new("test.name").is_err());
}

#[test]
fn signal_name_accepts_underscore_and_numbers() {
    assert!(SignalName::new("signal_123").is_ok());
    assert!(SignalName::new("sig2").is_ok());
}

#[test]
fn signal_name_rejects_uppercase() {
    assert!(SignalName::new("Signal").is_err());
    assert!(SignalName::new("SIGNAL").is_err());
}

#[test]
fn signal_name_rejects_dash_and_dot() {
    assert!(SignalName::new("signal-name").is_err());
    assert!(SignalName::new("signal.name").is_err());
}

#[test]
fn signal_name_requires_minimum_length() {
    assert!(SignalName::new("a").is_err());
    assert!(SignalName::new("ab").is_ok());
}
