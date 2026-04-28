//! Comprehensive tests for workflow sink validation.
//!
//! Validates publish-time rejection of unsupported sinks,
//! ensuring no published workflow can contain a managed
//! effect targeting an unknown sink.

use super::workflow::sink_validator::*;

#[test]
fn known_sinks_default_contains_blob_http_sql() {
    let sinks = KnownSinks::default_sinks();
    assert!(sinks.contains("blob"));
    assert!(sinks.contains("http"));
    assert!(sinks.contains("sql"));
    assert_eq!(sinks.len(), 3);
}

#[test]
fn known_sinks_does_not_contain_unknown() {
    let sinks = KnownSinks::default_sinks();
    assert!(!sinks.contains("unknown-sink"));
    assert!(!sinks.contains(""));
    assert!(!sinks.contains("kafka"));
    assert!(!sinks.contains("grpc"));
}

#[test]
fn known_sinks_default_trait_matches_default_sinks() {
    let explicit = KnownSinks::default_sinks();
    let via_trait = KnownSinks::default();
    assert_eq!(explicit, via_trait);
}

#[test]
fn known_sinks_with_custom_sinks() {
    let sinks = KnownSinks::new(["custom1", "custom2"]);
    assert!(sinks.contains("custom1"));
    assert!(sinks.contains("custom2"));
    assert!(!sinks.contains("blob"));
    assert!(!sinks.contains("http"));
    assert!(!sinks.contains("sql"));
    assert_eq!(sinks.len(), 2);
}

#[test]
fn known_sinks_with_single_custom_sink() {
    let sinks = KnownSinks::new(["only-one"]);
    assert!(sinks.contains("only-one"));
    assert_eq!(sinks.len(), 1);
    assert!(!sinks.is_empty());
}

#[test]
fn known_sinks_empty_set() {
    let sinks = KnownSinks::new(Vec::<&str>::new());
    assert!(sinks.is_empty());
    assert_eq!(sinks.len(), 0);
    assert!(!sinks.contains("anything"));
}

#[test]
fn known_sinks_equality_same_contents() {
    let a = KnownSinks::new(["x", "y"]);
    let b = KnownSinks::new(["y", "x"]);
    assert_eq!(a, b);
}

#[test]
fn known_sinks_equality_different_contents() {
    let a = KnownSinks::new(["x"]);
    let b = KnownSinks::new(["y"]);
    assert_ne!(a, b);
}

#[test]
fn known_sinks_display_shows_all_sinks() {
    let sinks = KnownSinks::default_sinks();
    let display = format!("{}", sinks);
    assert!(display.contains("blob"));
    assert!(display.contains("http"));
    assert!(display.contains("sql"));
    assert!(display.starts_with('['));
    assert!(display.ends_with(']'));
}

#[test]
fn known_sinks_display_custom_set() {
    let sinks = KnownSinks::new(["alpha"]);
    let display = format!("{}", sinks);
    assert!(display.contains("alpha"));
}

#[test]
fn validator_accepts_all_known_sinks() {
    let validator = WorkflowSinkValidator::new();
    assert!(validator.validate_sink("blob").is_ok());
    assert!(validator.validate_sink("http").is_ok());
    assert!(validator.validate_sink("sql").is_ok());
}

#[test]
fn validator_rejects_unknown_sink() {
    let validator = WorkflowSinkValidator::new();
    let result = validator.validate_sink("unknown-sink");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, UnsupportedSinkError::UnknownSink { .. }));
    assert_eq!(err.error_code(), "unsupported_sink");
    assert_eq!(err.sink_identifier(), Some("unknown-sink"));
}

#[test]
fn validator_rejects_empty_sink() {
    let validator = WorkflowSinkValidator::new();
    let result = validator.validate_sink("");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, UnsupportedSinkError::EmptySink));
    assert_eq!(err.error_code(), "empty_sink");
    assert_eq!(err.sink_identifier(), None);
}

#[test]
fn validator_error_message_contains_sink_and_known_sinks() {
    let validator = WorkflowSinkValidator::new();
    let result = validator.validate_sink("unknown-sink");
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("unknown-sink"));
    assert!(msg.contains("blob"));
    assert!(msg.contains("http"));
    assert!(msg.contains("sql"));
}

#[test]
fn validator_with_custom_sinks_accepts_custom() {
    let custom_sinks = KnownSinks::new(["custom-sink"]);
    let validator = WorkflowSinkValidator::with_sinks(custom_sinks);
    assert!(validator.validate_sink("custom-sink").is_ok());
}

#[test]
fn validator_with_custom_sinks_rejects_default_sinks() {
    let custom_sinks = KnownSinks::new(["custom-sink"]);
    let validator = WorkflowSinkValidator::with_sinks(custom_sinks);
    assert!(validator.validate_sink("blob").is_err());
    assert!(validator.validate_sink("http").is_err());
    assert!(validator.validate_sink("sql").is_err());
}

#[test]
fn validator_default_matches_new() {
    let via_new = WorkflowSinkValidator::new();
    let via_default = WorkflowSinkValidator::default();
    assert_eq!(via_new.known_sinks(), via_default.known_sinks());
}

#[test]
fn validator_known_sinks_accessor() {
    let validator = WorkflowSinkValidator::new();
    let sinks = validator.known_sinks();
    assert!(sinks.contains("blob"));
    assert!(sinks.contains("http"));
    assert!(sinks.contains("sql"));
}

#[test]
fn validate_sinks_multiple_known_succeeds() {
    let validator = WorkflowSinkValidator::new();
    let result = validator.validate_sinks(["blob", "http", "sql"]);
    assert!(result.is_ok());
}

#[test]
fn validate_sinks_returns_first_error() {
    let validator = WorkflowSinkValidator::new();
    let result = validator.validate_sinks(["blob", "unknown", "sql"]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, UnsupportedSinkError::UnknownSink { .. }));
}

#[test]
fn validate_sinks_empty_iterable_succeeds() {
    let validator = WorkflowSinkValidator::new();
    let result = validator.validate_sinks(Vec::<&str>::new());
    assert!(result.is_ok());
}

#[test]
fn validate_sinks_rejects_empty_sink_in_middle() {
    let validator = WorkflowSinkValidator::new();
    let result = validator.validate_sinks(["blob", "", "sql"]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, UnsupportedSinkError::EmptySink));
}

#[test]
fn validate_sinks_rejects_empty_sink_first() {
    let validator = WorkflowSinkValidator::new();
    let result = validator.validate_sinks(["", "blob"]);
    assert!(result.is_err());
}

#[test]
fn validate_sinks_single_known_succeeds() {
    let validator = WorkflowSinkValidator::new();
    assert!(validator.validate_sinks(["blob"]).is_ok());
    assert!(validator.validate_sinks(["http"]).is_ok());
    assert!(validator.validate_sinks(["sql"]).is_ok());
}

#[test]
fn validate_sinks_all_unknown_rejected() {
    let validator = WorkflowSinkValidator::new();
    let result = validator.validate_sinks(["kafka", "grpc", "amqp"]);
    assert!(result.is_err());
}

#[test]
fn validate_workflow_sinks_convenience_accepts_known() {
    assert!(validate_workflow_sinks(["blob", "sql"]).is_ok());
    assert!(validate_workflow_sinks(["http"]).is_ok());
    assert!(validate_workflow_sinks(["blob", "http", "sql"]).is_ok());
}

#[test]
fn validate_workflow_sinks_convenience_rejects_unknown() {
    assert!(validate_workflow_sinks(["unknown"]).is_err());
}

#[test]
fn validate_workflow_sinks_convenience_rejects_empty() {
    assert!(validate_workflow_sinks(["blob", ""]).is_err());
    assert!(validate_workflow_sinks([""]).is_err());
}

#[test]
fn validate_workflow_sinks_convenience_empty_iterable_succeeds() {
    assert!(validate_workflow_sinks(Vec::<&str>::new()).is_ok());
}

#[test]
fn unsupported_sink_error_unknown_sink_fields() {
    let err = UnsupportedSinkError::UnknownSink {
        sink: "test-sink".to_string(),
        known_sinks: "blob, http, sql".to_string(),
    };
    assert_eq!(err.sink_identifier(), Some("test-sink"));
    assert_eq!(err.error_code(), "unsupported_sink");
}

#[test]
fn unsupported_sink_error_empty_sink_fields() {
    let err = UnsupportedSinkError::EmptySink;
    assert_eq!(err.sink_identifier(), None);
    assert_eq!(err.error_code(), "empty_sink");
}

#[test]
fn unsupported_sink_error_equality() {
    let a = UnsupportedSinkError::EmptySink;
    let b = UnsupportedSinkError::EmptySink;
    assert_eq!(a, b);

    let c = UnsupportedSinkError::UnknownSink {
        sink: "x".to_string(),
        known_sinks: "blob".to_string(),
    };
    let d = UnsupportedSinkError::UnknownSink {
        sink: "x".to_string(),
        known_sinks: "blob".to_string(),
    };
    assert_eq!(c, d);
}

#[test]
fn unsupported_sink_error_inequality() {
    let a = UnsupportedSinkError::EmptySink;
    let b = UnsupportedSinkError::UnknownSink {
        sink: "x".to_string(),
        known_sinks: "blob".to_string(),
    };
    assert_ne!(a, b);
}

#[test]
fn validate_effect_kinds_all_known_succeeds() {
    use vo_types::EffectKind;
    let result = validate_effect_kinds([
        EffectKind::HttpCall,
        EffectKind::SqlQuery,
        EffectKind::BlobWrite,
    ]);
    assert!(result.is_ok());
}

#[test]
fn validate_effect_kinds_http_call_ok() {
    use vo_types::EffectKind;
    assert!(validate_effect_kinds([EffectKind::HttpCall]).is_ok());
}

#[test]
fn validate_effect_kinds_sql_query_ok() {
    use vo_types::EffectKind;
    assert!(validate_effect_kinds([EffectKind::SqlQuery]).is_ok());
}

#[test]
fn validate_effect_kinds_blob_write_ok() {
    use vo_types::EffectKind;
    assert!(validate_effect_kinds([EffectKind::BlobWrite]).is_ok());
}

#[test]
fn validate_effect_kinds_empty_iterable_ok() {
    let result = validate_effect_kinds(Vec::<vo_types::EffectKind>::new());
    assert!(result.is_ok());
}

#[test]
fn validate_workflow_effects_all_known_succeeds() {
    use vo_types::EffectKind;
    let kinds = [
        EffectKind::HttpCall,
        EffectKind::SqlQuery,
        EffectKind::BlobWrite,
    ];
    assert!(validate_workflow_effects(kinds).is_ok());
}

#[test]
fn validate_workflow_effects_empty_ok() {
    let result = validate_workflow_effects(Vec::<vo_types::EffectKind>::new());
    assert!(result.is_ok());
}

#[test]
fn validate_workflow_effects_single_http_ok() {
    use vo_types::EffectKind;
    assert!(validate_workflow_effects([EffectKind::HttpCall]).is_ok());
}

#[test]
fn validate_workflow_effects_single_sql_ok() {
    use vo_types::EffectKind;
    assert!(validate_workflow_effects([EffectKind::SqlQuery]).is_ok());
}

#[test]
fn validate_workflow_effects_single_blob_ok() {
    use vo_types::EffectKind;
    assert!(validate_workflow_effects([EffectKind::BlobWrite]).is_ok());
}
