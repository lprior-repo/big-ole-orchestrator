//! Shared PII test fixtures and constants

use vo_types::{InstanceId, RedactionKind, RedactionRule};

pub const TEST_SSN: &str = "123-45-6789";
pub const TEST_EMAIL: &str = "alice@example.com";
pub const TEST_CREDIT_CARD: &str = "4111-1111-1111-1111";
pub const TEST_PHONE: &str = "+1-555-123-4567";
pub const TEST_SSN_2: &str = "987-65-4321";
pub const TEST_EMAIL_2: &str = "bob@private.org";

pub fn instance_id() -> InstanceId {
    InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID")
}

pub fn standard_pii_redaction_rules() -> Vec<RedactionRule> {
    vec![
        RedactionRule::new(vec!["user".into(), "ssn".into()], RedactionKind::Remove),
        RedactionRule::new(
            vec!["user".into(), "email".into()],
            RedactionKind::ReplaceWith("[EMAIL_REDACTED]".into()),
        ),
        RedactionRule::new(
            vec!["user".into(), "credit_card".into()],
            RedactionKind::ReplaceWith("[CC_REDACTED]".into()),
        ),
        RedactionRule::new(vec!["user".into(), "phone".into()], RedactionKind::Hash),
        RedactionRule::new(
            vec!["user".into(), "password".into()],
            RedactionKind::Remove,
        ),
        RedactionRule::new(
            vec!["secret".into()],
            RedactionKind::ReplaceWith("[SECRET_REDACTED]".into()),
        ),
    ]
}

pub fn multi_user_pii_redaction_rules() -> Vec<RedactionRule> {
    vec![
        RedactionRule::new(vec!["users".into(), "ssn".into()], RedactionKind::Remove),
        RedactionRule::new(vec!["users".into(), "email".into()], RedactionKind::Hash),
    ]
}

pub fn nested_pii_redaction_rules() -> Vec<RedactionRule> {
    vec![
        RedactionRule::new(
            vec!["profile".into(), "credentials".into(), "password".into()],
            RedactionKind::Remove,
        ),
        RedactionRule::new(
            vec!["profile".into(), "credentials".into(), "totp".into()],
            RedactionKind::ReplaceWith("[TOTP_REDACTED]".into()),
        ),
    ]
}
